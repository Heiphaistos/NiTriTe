use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use tauri::Emitter;

use crate::error::NiTriTeError;

#[derive(Debug, Clone, Serialize)]
pub struct WingetPackage {
    pub name: String,
    pub id: String,
    pub version: String,
    pub available: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallResult {
    pub success: bool,
    pub app_id: String,
    pub message: String,
}

/// Découpe une ligne de table winget en colonnes. winget sépare les colonnes
/// par 2 espaces ou plus ; un nom de paquet peut contenir un espace simple,
/// d'où ce parsing par double-espace plutôt qu'un `split_whitespace` qui
/// éclaterait les noms composés. L'indexation ASCII est sûre : la coupe ne se
/// fait que sur des espaces (frontières de caractères valides).
fn split_winget_columns(line: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let bytes = line.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b' ' && bytes[i + 1] == b' ' {
            let p = line[start..i].trim();
            if !p.is_empty() { parts.push(p); }
            while i < bytes.len() && bytes[i] == b' ' { i += 1; }
            start = i;
        } else { i += 1; }
    }
    let last = line[start..].trim();
    if !last.is_empty() { parts.push(last); }
    parts
}

pub fn check_winget() -> bool {
    Command::new("winget").arg("--version")
        .stdout(Stdio::null()).stderr(Stdio::null())
        .creation_flags(0x08000000)
        .status().is_ok()
}

pub fn list_upgradable() -> Result<Vec<WingetPackage>, NiTriTeError> {
    // Timeout : la 1re requete du jour rafraichit les sources (msstore compris)
    // et peut depasser la minute ; sans borne, un winget bloque gelait l'onglet.
    let out = crate::maintenance::commands::execute_system_command(
        "winget",
        &["upgrade", "--include-unknown", "--accept-source-agreements", "--disable-interactivity"],
        150,
    )?;
    let text = out.stdout;
    let mut packages = Vec::new();
    let mut past_separator = false;

    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() { continue; }
        if !past_separator {
            if t.chars().all(|c| c == '-' || c == ' ') && t.len() > 10 { past_separator = true; }
            continue;
        }
        let lower = t.to_lowercase();
        // Fin de la table = phrase de pied de page winget. On matche la PHRASE
        // (EN « N package(s) have version numbers that cannot be determined »,
        // « X upgrades available » / FR « … ne peuvent pas être déterminés »,
        // « … mises à niveau disponibles ») et NON le mot nu « package », qui
        // apparaît légitimement dans des noms (« NuGet Package Explorer ») et
        // aurait tronqué la liste prématurément.
        if lower.contains("cannot be determined")
            || lower.contains("détermin") || lower.contains("determin")
            || lower.contains("upgrades available")
            || (lower.contains("niveau") && lower.contains("disponible"))
        {
            break;
        }
        let parts = split_winget_columns(t);
        if parts.len() >= 3 {
            packages.push(WingetPackage {
                name: parts[0].to_string(),
                id: parts.get(1).unwrap_or(&"").to_string(),
                version: parts.get(2).unwrap_or(&"").to_string(),
                available: parts.get(3).unwrap_or(&"").to_string(),
                source: "winget".to_string(),
            });
        }
    }

    Ok(packages)
}

pub fn install_package(
    package_id: &str,
    window: &tauri::Window,
) -> Result<InstallResult, NiTriTeError> {
    let mut child = Command::new("winget")
        .args([
            "install", "--id", package_id, "--exact", "--silent",
            "--accept-source-agreements", "--accept-package-agreements",
            "--disable-interactivity",
        ])
        .stdout(Stdio::piped())
        // stderr non lu ici : le piper sans le drainer bloquerait winget (deadlock)
        // dès que son buffer stderr se remplit. On le jette ; le verdict vient du
        // code de sortie et la sortie utile de winget passe par stdout.
        .stderr(Stdio::null())
        .creation_flags(0x08000000)
        .spawn()?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            // Classification EN + FR, insensible à la casse : winget est localisé,
            // donc « erreur »/« Réussi » sur Windows FR — sans ça toutes les lignes
            // tombaient en « info » (aucune coloration rouge/verte).
            let ll = line.to_lowercase();
            let level = if ll.contains("error") || ll.contains("erreur") || ll.contains("échou") || ll.contains("failed") || ll.contains("échec") {
                "error"
            } else if ll.contains("successfully") || ll.contains("réussi") || ll.contains("succès") {
                "success"
            } else {
                "info"
            };
            let _ = window.emit("install-log", serde_json::json!({
                "app_id": package_id,
                "line": line,
                "level": level,
            }));
        }
    }

    let status = wait_with_watchdog(child, INSTALL_TIMEOUT)?;
    let code = status.code().unwrap_or(-1);
    let success = status.success() || OK_EXIT_CODES.contains(&code);

    Ok(InstallResult {
        success,
        app_id: package_id.to_string(),
        message: if status.success() { "Installation reussie".into() } else { describe_exit(code) },
    })
}

/// Duree maximale d'une installation/mise a jour winget. Au-dela, l'installeur
/// attend presque toujours une fenetre invisible (process lance sans console).
const INSTALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Codes de sortie winget qui ne sont PAS des echecs (verifies avec
/// `winget error <code>`, winget 1.29) : deja a jour, deja installe,
/// redemarrage requis pour terminer / redemarrage lance.
pub const OK_EXIT_CODES: [i32; 4] = [
    0x8A15002B_u32 as i32, // UPDATE_NOT_APPLICABLE
    0x8A150061_u32 as i32, // PACKAGE_ALREADY_INSTALLED
    0x8A150109_u32 as i32, // INSTALL_REBOOT_REQUIRED_TO_FINISH
    0x8A15010B_u32 as i32, // INSTALL_REBOOT_INITIATED
];

/// Libelle francais d'un code de sortie winget (codes les plus courants,
/// libelles repris de `winget error <code>`). Code inconnu : valeur hexa,
/// que `winget error` sait expliquer.
pub fn describe_exit(code: i32) -> String {
    let msg = match code as u32 {
        0 => "Reussi",
        0x8A15002B => "Deja a jour (aucune mise a jour applicable)",
        0x8A150061 => "Deja installe",
        0x8A150109 => "Installe, redemarrez le PC pour terminer",
        0x8A15010B => "Installe, le PC va redemarrer pour terminer",
        0x8A150014 => "Paquet introuvable dans les sources winget",
        0x8A15010A => "Redemarrage requis avant l'installation",
        0x8A15010D => "Une autre version de l'application est deja installee",
        0x8A150101 => "Application en cours d'execution : fermez-la puis reessayez",
        0x8A150102 => "Une autre installation est deja en cours",
        0x8A150107 => "Pas de connexion Internet",
        0x8A15010C => "Installation annulee par l'utilisateur",
        0x8A150056 => "L'installeur refuse d'etre lance en administrateur",
        _ => return format!("Echec winget (code 0x{:08X})", code as u32),
    };
    format!("{msg} (code 0x{:08X})", code as u32)
}

/// Attend la fin du process ; s'il depasse `timeout`, tue toute son
/// arborescence (`taskkill /T` : winget lance l'installeur en enfant) et
/// renvoie une erreur au lieu de bloquer indefiniment.
pub(crate) fn wait_with_watchdog(mut child: std::process::Child, timeout: std::time::Duration) -> Result<std::process::ExitStatus, NiTriTeError> {
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let watchdog = std::thread::spawn(move || {
        if rx.recv_timeout(timeout).is_err() {
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .creation_flags(0x08000000)
                .status();
            return true;
        }
        false
    });
    let status = child.wait();
    let _ = tx.send(());
    let killed = watchdog.join().unwrap_or(false);
    if killed {
        return Err(NiTriTeError::Timeout(format!(
            "winget interrompu apres {} min (installeur probablement en attente d'une interaction)",
            timeout.as_secs() / 60
        )));
    }
    Ok(status?)
}

/// Lance une commande winget et streame stdout ligne par ligne via `upgrade-log`.
fn stream_winget_upgrade(args: &[&str], window: &tauri::Window) -> Result<(), NiTriTeError> {
    let mut child = Command::new("winget")
        .args(args)
        .stdout(Stdio::piped())
        // stderr non lu ici : le piper sans le drainer bloquerait winget (deadlock)
        // dès que son buffer stderr se remplit. On le jette ; le verdict vient du
        // code de sortie et la sortie utile de winget passe par stdout.
        .stderr(Stdio::null())
        .creation_flags(0x08000000)
        .spawn()?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = window.emit("upgrade-log", &line);
        }
    }

    // child.wait()? seul ne propage QUE les erreurs d'E/S (process introuvable
    // etc.) — un winget qui echoue reellement (paquet verrouille, telechargement
    // interrompu, confirmation interactive malgre --silent) renvoie un
    // ExitStatus valide avec success()=false, jamais un Err. upgrade_all()
    // renvoyait donc toujours Ok(()) meme quand AUCUNE mise a jour n'avait
    // reellement abouti, et les deux appelants (UpdatesPage.vue, DiagTabUpdates.vue)
    // font confiance a ce succes pour afficher un toast "termine" et vider la
    // liste des mises a jour en attente.
    let status = wait_with_watchdog(child, INSTALL_TIMEOUT)?;
    let code = status.code().unwrap_or(-1);
    if !status.success() && !OK_EXIT_CODES.contains(&code) {
        return Err(NiTriTeError::System(describe_exit(code)));
    }
    Ok(())
}

/// Identifiants (en minuscules) des paquets installes connus de winget.
/// `winget export` plutot que `winget list` : la table de `list` tronque les
/// colonnes (« Microsoft.VisualStudio… ») selon la largeur de console, alors
/// que l'export JSON donne les identifiants exacts.
pub fn installed_ids() -> std::collections::HashSet<String> {
    let path = std::env::temp_dir().join(format!("nitrite-winget-export-{}.json", std::process::id()));
    let path_str = path.to_string_lossy().to_string();
    let ran = crate::maintenance::commands::execute_system_command(
        "winget",
        &["export", "-o", &path_str, "--accept-source-agreements", "--disable-interactivity"],
        120,
    );
    let text = if ran.is_ok() { std::fs::read_to_string(&path).unwrap_or_default() } else { String::new() };
    let _ = std::fs::remove_file(&path);
    parse_export_ids(&text)
}

/// Extrait les `PackageIdentifier` d'un fichier `winget export`.
pub fn parse_export_ids(json: &str) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    // winget ecrit parfois un BOM UTF-8 en tete du fichier.
    let json = json.trim_start_matches('\u{feff}');
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return ids };
    for src in v["Sources"].as_array().into_iter().flatten() {
        for pkg in src["Packages"].as_array().into_iter().flatten() {
            if let Some(id) = pkg["PackageIdentifier"].as_str() {
                ids.insert(id.to_lowercase());
            }
        }
    }
    ids
}

pub fn upgrade_all(excluded_ids: Vec<String>, window: &tauri::Window) -> Result<(), NiTriTeError> {
    // winget upgrade ne supporte PAS d'option d'exclusion : `--all` mettrait à
    // jour TOUS les paquets, y compris ceux que l'utilisateur a exclus. Quand une
    // exclusion existe, on met donc à jour chaque paquet non-exclu individuellement.
    if excluded_ids.is_empty() {
        return stream_winget_upgrade(
            &["upgrade", "--all", "--silent", "--accept-source-agreements", "--accept-package-agreements", "--disable-interactivity"],
            window,
        );
    }

    // Un paquet en echec ne doit pas empecher la mise a jour des suivants :
    // on continue et on rapporte tous les echecs a la fin.
    let pkgs = list_upgradable()?;
    let mut failures: Vec<String> = Vec::new();
    for p in pkgs {
        if p.id.is_empty() || excluded_ids.iter().any(|e| e.eq_ignore_ascii_case(&p.id)) {
            let _ = window.emit("upgrade-log", format!("Exclu : {} ({})", p.name, p.id));
            continue;
        }
        let _ = window.emit("upgrade-log", format!("Mise à jour de {} ({})…", p.name, p.id));
        if let Err(e) = stream_winget_upgrade(
            &["upgrade", "--id", p.id.as_str(), "--exact", "--silent",
              "--accept-source-agreements", "--accept-package-agreements", "--disable-interactivity"],
            window,
        ) {
            let _ = window.emit("upgrade-log", format!("Échec : {} — {e}", p.name));
            failures.push(format!("{} : {e}", p.name));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(NiTriTeError::System(format!("{} mise(s) à jour en échec — {}", failures.len(), failures.join(" | "))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_export_ids_reads_every_source() {
        let json = "\u{feff}{\"Sources\":[{\"Packages\":[{\"PackageIdentifier\":\"7zip.7zip\"},{\"PackageIdentifier\":\"Git.Git\"}]},{\"Packages\":[{\"PackageIdentifier\":\"9NKSQGP7F2NH\"}]}]}";
        let ids = parse_export_ids(json);
        assert_eq!(ids.len(), 3);
        assert!(ids.contains("git.git"));
        assert!(ids.contains("9nksqgp7f2nh"));
    }

    #[test]
    fn known_benign_exit_codes_are_ok() {
        // -1978335189 = deja a jour, observe en vrai (LESSONS cycle 55).
        assert!(OK_EXIT_CODES.contains(&-1978335189));
        assert!(!OK_EXIT_CODES.contains(&(0x8A150014_u32 as i32)));
        assert_eq!(describe_exit(-1978335212), "Paquet introuvable dans les sources winget (code 0x8A150014)");
        assert!(describe_exit(-1).starts_with("Echec winget (code 0xFFFFFFFF"));
    }

    #[test]
    fn parse_export_ids_tolerates_garbage() {
        assert!(parse_export_ids("").is_empty());
        assert!(parse_export_ids("not json").is_empty());
        assert!(parse_export_ids("{\"Sources\":null}").is_empty());
    }
}
