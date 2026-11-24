use serde::Serialize;
use tauri::Emitter;

use crate::maintenance::commands::execute_system_command;

#[derive(Debug, Clone, Serialize)]
pub struct DepsProgress {
    pub checked: u8,
    pub total: u8,
    pub item: Dependency,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dependency {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub installed: bool,
    pub version: String,
    pub winget_id: String,
    pub install_url: String,
}

/// Duree maximale d'une verification : `code --version`, `java -version` ou un
/// PowerShell froid peuvent rester bloques ; sans borne, le scan ne finissait
/// jamais (l'evenement `done` n'etait jamais emis).
const CHECK_TIMEOUT_SECS: u64 = 20;

/// Lance `cmd args` avec timeout (VS Code passe par `cmd /c` : `code` est un
/// `.cmd`, que `Command::new` ne resout pas — il etait toujours vu absent). Installe = code 0 ET sortie non vide.
/// (L'ancien test « code 0 OU stdout non vide » prenait pour une version le
/// message d'erreur de `dotnet --version` sans SDK, et laissait passer les
/// alias d'execution du Store.)
fn run_cmd(cmd: &str, args: &[&str]) -> (bool, String) {
    match execute_system_command(cmd, args, CHECK_TIMEOUT_SECS) {
        Ok(r) => {
            let combined = format!("{}{}", r.stdout, r.stderr);
            let first_line = combined.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string();
            (r.success && !first_line.is_empty(), first_line)
        }
        Err(_) => (false, String::new()),
    }
}

fn check_ps(ps_expr: &str) -> (bool, String) {
    run_cmd("powershell", &["-NoProfile", "-NonInteractive", "-Command", ps_expr])
}

fn dep(id: &str, name: &str, cat: &str, desc: &str, (ok, version): (bool, String), wid: &str, url: &str) -> Dependency {
    Dependency {
        id: id.into(), name: name.into(), category: cat.into(), description: desc.into(),
        installed: ok, version, winget_id: wid.into(), install_url: url.into(),
    }
}

/// Test de presence par chemin (exe hors PATH : 7-Zip, Notepad++, VLC…).
fn exists_any(paths: &[&str]) -> (bool, String) {
    let ok = paths.iter().any(|p| {
        let expanded = p.replace("%ProgramFiles%", &std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into()))
            .replace("%ProgramFiles(x86)%", &std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into()));
        std::path::Path::new(&expanded).exists()
    });
    (ok, String::new())
}

type CheckFn = Box<dyn Fn() -> Dependency + Send + Sync>;

/// Liste unique des verifications (avant : dupliquee entre `check_all` et
/// `check_all_streaming`, les deux copies divergeaient).
/// Versions a jour 2026-11 : Python 3.14 (stable courante), .NET 10 (LTS ;
/// .NET 8 hors support depuis le 10/11/2026), Java = Temurin 25 LTS (l'ancien
/// `Oracle.JavaRuntimeEnvironment` est le JRE 8 sous licence Oracle), OpenSSH
/// `Microsoft.OpenSSH.Preview` (l'id `.Beta` n'existe plus), `cURL.cURL`
/// (l'id `curl.curl` n'existe pas dans winget).
fn checks() -> Vec<CheckFn> {
    vec![
        Box::new(|| dep("git", "Git", "Développement", "Gestionnaire de versions distribué", run_cmd("git", &["--version"]), "Git.Git", "https://git-scm.com/download/win")),
        Box::new(|| dep("nodejs", "Node.js", "Développement", "Runtime JavaScript (LTS)", run_cmd("node", &["--version"]), "OpenJS.NodeJS.LTS", "https://nodejs.org")),
        Box::new(|| dep("python", "Python", "Développement", "Langage de script Python 3", run_cmd("python", &["--version"]), "Python.Python.3.14", "https://www.python.org/downloads/")),
        Box::new(|| dep("rust", "Rust (rustc)", "Développement", "Compilateur Rust via rustup", run_cmd("rustc", &["--version"]), "Rustlang.Rustup", "https://rustup.rs")),
        Box::new(|| dep("vscode", "Visual Studio Code", "Développement", "Éditeur de code Microsoft", run_cmd("cmd", &["/c", "code", "--version"]), "Microsoft.VisualStudioCode", "https://code.visualstudio.com")),
        // --list-runtimes : fonctionne sans SDK (un simple runtime suffit aux apps).
        Box::new(|| dep("dotnet", ".NET Runtime", "Runtimes", "Runtime .NET 10 (LTS)", run_cmd("dotnet", &["--list-runtimes"]), "Microsoft.DotNet.DesktopRuntime.10", "https://dotnet.microsoft.com/download/dotnet/10.0")),
        Box::new(|| dep("winget", "WinGet", "Gestionnaires", "Gestionnaire de paquets Windows officiel", run_cmd("winget", &["--version"]), "", "ms-windows-store://pdp/?productid=9NBLGGH4NNS1")),
        Box::new(|| dep("scoop", "Scoop", "Gestionnaires", "Gestionnaire de paquets sans admin", check_ps("if (Get-Command scoop -EA SilentlyContinue) { 'present' } else { exit 1 }"), "", "https://scoop.sh")),
        Box::new(|| dep("chocolatey", "Chocolatey", "Gestionnaires", "Gestionnaire de paquets Windows", check_ps("if (Get-Command choco -EA SilentlyContinue) { choco --version } else { exit 1 }"), "", "https://chocolatey.org/install")),
        Box::new(|| dep("curl", "curl", "Outils Système", "Outil de transfert HTTP/S en ligne de commande", run_cmd("curl", &["--version"]), "cURL.cURL", "https://curl.se")),
        Box::new(|| dep("7zip", "7-Zip", "Outils Système", "Archiveur haute compression", exists_any(&[r"%ProgramFiles%\7-Zip\7z.exe", r"%ProgramFiles(x86)%\7-Zip\7z.exe"]), "7zip.7zip", "https://7-zip.org")),
        Box::new(|| dep("notepadpp", "Notepad++", "Outils Système", "Éditeur de texte avancé", exists_any(&[r"%ProgramFiles%\Notepad++\notepad++.exe", r"%ProgramFiles(x86)%\Notepad++\notepad++.exe"]), "Notepad++.Notepad++", "https://notepad-plus-plus.org")),
        Box::new(|| dep("openssh", "OpenSSH Client", "Outils Système", "Client SSH intégré Windows", run_cmd("ssh", &["-V"]), "Microsoft.OpenSSH.Preview", "")),
        Box::new(|| dep("vlc", "VLC Media Player", "Médias", "Lecteur multimédia universel", exists_any(&[r"%ProgramFiles%\VideoLAN\VLC\vlc.exe", r"%ProgramFiles(x86)%\VideoLAN\VLC\vlc.exe"]), "VideoLAN.VLC", "https://www.videolan.org")),
        Box::new(|| dep("powertoys", "PowerToys", "Productivité", "Suite d'utilitaires Microsoft pour power users", check_ps("if (Get-ItemProperty HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*,HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\* -EA SilentlyContinue | Where-Object { $_.DisplayName -like '*PowerToys*' }) { 'present' } else { exit 1 }"), "Microsoft.PowerToys", "")),
        Box::new(|| dep("java", "Java (JRE)", "Runtimes", "Java Runtime Environment (Temurin 25 LTS)", run_cmd("java", &["-version"]), "EclipseAdoptium.Temurin.25.JRE", "https://adoptium.net")),
        Box::new(|| dep("golang", "Go", "Runtimes", "Langage Go (runtime + compilateur)", run_cmd("go", &["version"]), "GoLang.Go", "https://go.dev")),
    ]
}

/// Toutes les vérifications en parallèle.
pub fn check_all() -> Vec<Dependency> {
    let checks = checks();
    std::thread::scope(|s| {
        let handles: Vec<_> = checks.iter().map(|c| s.spawn(move || c())).collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
}

/// Vérifie chaque dépendance et émet un événement `deps:progress` au fur et à mesure.
pub fn check_all_streaming(app: &tauri::AppHandle) {
    let checks = checks();
    let total = checks.len() as u8;
    let (tx, rx) = std::sync::mpsc::channel::<Dependency>();
    std::thread::scope(|s| {
        for check_fn in &checks {
            let tx = tx.clone();
            s.spawn(move || { let _ = tx.send(check_fn()); });
        }
        drop(tx);
        let mut received = 0u8;
        for item in rx {
            received += 1;
            let _ = app.emit("deps:progress", DepsProgress { checked: received, total, item, done: received == total });
        }
    });
}

pub fn install_via_winget(winget_id: &str) -> Result<String, String> {
    if winget_id.is_empty() {
        return Err("Pas d'ID winget — installez manuellement.".into());
    }
    // --exact : sans lui, un id partiel pouvait installer un autre paquet.
    // Timeout 30 min : un installeur bloque sur une fenetre invisible ne doit
    // pas laisser le bouton « Installation… » tourner indefiniment.
    let r = execute_system_command(
        "winget",
        &["install", "--id", winget_id, "--exact", "--silent",
          "--accept-source-agreements", "--accept-package-agreements", "--disable-interactivity"],
        30 * 60,
    ).map_err(|e| format!("winget : {e}"))?;
    if r.success || crate::installer::winget::OK_EXIT_CODES.contains(&r.exit_code) {
        Ok(format!("{} : {}", winget_id, if r.success { "installé avec succès".to_string() } else { crate::installer::winget::describe_exit(r.exit_code) }))
    } else {
        let detail = r.stdout.lines().chain(r.stderr.lines()).map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("");
        Err(format!("{} — {}", crate::installer::winget::describe_exit(r.exit_code), detail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_ids_are_unique_and_winget_ids_current() {
        let all: Vec<Dependency> = checks().iter().map(|c| c()).collect();
        let mut ids = std::collections::HashSet::new();
        for d in &all {
            assert!(ids.insert(d.id.clone()), "doublon {}", d.id);
            // Identifiants retires/renommes de winget (verifie 2026-11-23 sur l'index winget).
            assert!(!["curl.curl", "Microsoft.OpenSSH.Beta", "Microsoft.DotNet.Runtime.8", "Oracle.JavaRuntimeEnvironment", "Python.Python.3.12"].contains(&d.winget_id.as_str()), "{}", d.winget_id);
        }
        assert_eq!(all.len(), 17);
    }
}
