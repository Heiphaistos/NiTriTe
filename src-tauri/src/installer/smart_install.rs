use std::io::Write;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::time::{Duration, Instant};
use tauri::Emitter;

use crate::error::NiTriTeError;
use crate::installer::{app_match, chocolatey, manager::AppEntry, scoop, uninstaller, winget};
use crate::installer::winget::InstallResult;

const DIRECT_INSTALL_TIMEOUT: Duration = Duration::from_secs(180);
/// Un desinstalleur silencieux qui depasse 10 min attend une interaction.
const UNINSTALL_TIMEOUT: Duration = Duration::from_secs(600);

fn log(window: &tauri::Window, app_id: &str, line: &str, level: &str) {
    let _ = window.emit("install-log", serde_json::json!({ "app_id": app_id, "line": line, "level": level }));
}

/// Verifie pour de vrai qu'une app est desormais presente (registre
/// Uninstall), au lieu de se fier au seul code de sortie de l'installeur
/// (classe de bug faux-succes rencontree partout ailleurs dans ce projet).
/// La comparaison de noms est dans `app_match` (mots entiers, sans les jetons
/// de version/architecture que le registre ajoute).
fn verify_installed(app_name: &str) -> bool {
    let names: Vec<String> = uninstaller::list_installed_apps().into_iter().map(|a| a.name).collect();
    app_match::any_installed(app_name, &names)
}

/// Verdict de winget lui-meme sur la presence d'un paquet : fiable meme quand
/// le nom affiche dans le registre ne ressemble pas au nom du catalogue.
/// `winget list --id X --exact` sort en code 0 uniquement si le paquet est
/// installe (quirk connu : code non nul + message localise sinon).
pub fn winget_is_installed(id: &str) -> bool {
    let mut cmd = Command::new("winget");
    cmd.args(["list", "--id", id, "--exact", "--accept-source-agreements", "--disable-interactivity"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000);
    match cmd.output() {
        Ok(o) => o.status.success() && String::from_utf8_lossy(&o.stdout).to_lowercase().contains(&id.to_lowercase()),
        Err(_) => false,
    }
}

fn choco_is_installed(id: &str) -> bool {
    std::path::Path::new(r"C:\ProgramData\chocolatey\lib").join(id).exists()
}

fn scoop_is_installed(id: &str) -> bool {
    dirs::home_dir().map(|h| h.join("scoop").join("apps").join(id).exists()).unwrap_or(false)
}

/// Detecte le type d'installeur par signature binaire (bien plus fiable que
/// deviner par extension seule) et retourne les arguments silencieux a
/// utiliser. `None` = type inconnu -> on n'execute PAS un installeur qu'on ne
/// sait pas rendre silencieux (risque de fenetre bloquante en attente d'un
/// utilisateur qui ne viendra jamais, puisque le process est lance sans
/// fenetre visible).
fn detect_silent_args(path: &std::path::Path) -> Option<Vec<String>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "msi" {
        return Some(vec!["/quiet".into(), "/norestart".into()]);
    }
    if ext != "exe" {
        return None;
    }
    // Signature dans les premiers Mo suffit (les stubs d'installeur sont en tete).
    let bytes = std::fs::read(path).ok()?;
    let scan = &bytes[..bytes.len().min(4_000_000)];
    let contains = |needle: &[u8]| scan.windows(needle.len()).any(|w| w == needle);
    if contains(b"Nullsoft") {
        Some(vec!["/S".into()])
    } else if contains(b"Inno Setup") {
        Some(vec!["/VERYSILENT".into(), "/SUPPRESSMSGBOXES".into(), "/NORESTART".into()])
    } else if contains(b"InstallShield") {
        Some(vec!["/s".into(), "/v/qn".into()])
    } else {
        None
    }
}

fn run_with_timeout(mut cmd: Command, timeout: Duration) -> Result<std::process::ExitStatus, NiTriTeError> {
    let mut child = cmd.spawn()?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            return Err(NiTriTeError::Timeout("L'installeur ne repond pas (delai depasse, probablement en attente d'une interaction manuelle)".into()));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Telecharge l'installeur depuis une URL directe et tente une installation
/// silencieuse si le type est reconnu. Methode de dernier recours : winget et
/// Chocolatey encodent deja le bon switch par paquet (maintenu par la
/// communaute), donc bien plus fiables que cette detection maison.
fn install_from_url(app_name: &str, url: &str, window: &tauri::Window) -> Result<InstallResult, NiTriTeError> {
    log(window, app_name, &format!("Telechargement depuis {url}..."), "info");
    let resp = reqwest::blocking::get(url).map_err(NiTriTeError::Network)?;
    if !resp.status().is_success() {
        return Ok(InstallResult { success: false, app_id: app_name.into(), message: format!("Telechargement echoue: HTTP {}", resp.status()) });
    }
    let content_disposition_ext = resp
        .headers()
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit('.').next())
        .map(|e| e.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase());
    let url_ext = url.split('?').next().unwrap_or(url).rsplit('.').next().map(|e| e.to_lowercase());
    let ext = content_disposition_ext.filter(|e| e.len() <= 4).or(url_ext).unwrap_or_else(|| "exe".into());
    let bytes = resp.bytes().map_err(NiTriTeError::Network)?;

    let tmp_dir = std::env::temp_dir().join("nitrite-installers");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp_path = tmp_dir.join(format!("{}.{}", app_name.chars().filter(|c| c.is_alphanumeric()).collect::<String>(), ext));
    {
        let mut f = std::fs::File::create(&tmp_path)?;
        f.write_all(&bytes)?;
    }

    let result = (|| -> Result<InstallResult, NiTriTeError> {
        if ext == "msi" {
            log(window, app_name, "Installation MSI silencieuse...", "info");
            let mut cmd = Command::new("msiexec");
            cmd.args(["/i", tmp_path.to_string_lossy().as_ref(), "/quiet", "/norestart"]).creation_flags(0x08000000);
            let status = run_with_timeout(cmd, DIRECT_INSTALL_TIMEOUT)?;
            return Ok(InstallResult { success: status.success(), app_id: app_name.into(), message: if status.success() { "Installation reussie (MSI direct)".into() } else { format!("msiexec code: {}", status.code().unwrap_or(-1)) } });
        }
        let Some(args) = detect_silent_args(&tmp_path) else {
            return Ok(InstallResult { success: false, app_id: app_name.into(), message: "Type d'installeur non reconnu — pas de mode silencieux fiable detecte, installation manuelle requise".into() });
        };
        log(window, app_name, &format!("Installeur detecte, switch silencieux: {}", args.join(" ")), "info");
        let mut cmd = Command::new(&tmp_path);
        cmd.args(&args).creation_flags(0x08000000);
        let status = run_with_timeout(cmd, DIRECT_INSTALL_TIMEOUT)?;
        Ok(InstallResult { success: status.success(), app_id: app_name.into(), message: if status.success() { "Installation reussie (telechargement direct)".into() } else { format!("Code: {}", status.code().unwrap_or(-1)) } })
    })();

    let _ = std::fs::remove_file(&tmp_path);
    result
}

/// Chaine de fallback complete pour installer une application, dans l'ordre
/// de fiabilite decroissante : winget (officiel Microsoft) -> Chocolatey
/// (communaute, tres large couverture) -> Scoop (CLI/outils dev/portables) ->
/// telechargement direct + detection de switch silencieux (dernier recours,
/// moins fiable). Chaque etape est verifiee pour de vrai via le registre
/// avant d'etre consideree comme un succes.
pub fn install_app_smart(app: &AppEntry, window: &tauri::Window) -> Result<InstallResult, NiTriTeError> {
    let mut attempts: Vec<String> = Vec::new();

    if let Some(wid) = app.winget_id.as_deref().filter(|w| !w.is_empty()) {
        log(window, &app.name, &format!("Tentative via winget ({wid})..."), "info");
        match winget::install_package(wid, window) {
            Ok(r) if r.success && (winget_is_installed(wid) || verify_installed(&app.name)) => return Ok(r),
            Ok(r) => attempts.push(format!("winget: {}", r.message)),
            Err(e) => attempts.push(format!("winget: {e}")),
        }
    }

    // Bootstrap AVANT la recherche : sur une machine fraiche, `choco search`/
    // `scoop search` echouent silencieusement tant que le gestionnaire lui-meme
    // n'est pas installe (probleme de l'oeuf et la poule) — sans bootstrap
    // prealable, choco_id/scoop_id ne sont jamais trouves et ces methodes
    // restent mortes en permanence sur une install fraiche de Windows.
    if chocolatey::bootstrap_chocolatey().is_ok() {
        let choco_id = app.choco_id.clone().filter(|c| !c.is_empty()).or_else(|| chocolatey::search_choco_id(&app.name));
        if let Some(cid) = choco_id {
            log(window, &app.name, &format!("Tentative via Chocolatey ({cid})..."), "info");
            match chocolatey::install_via_chocolatey(&cid, window) {
                Ok(r) if r.success && (choco_is_installed(&cid) || verify_installed(&app.name)) => return Ok(r),
                Ok(r) => attempts.push(format!("chocolatey: {}", r.message)),
                Err(e) => attempts.push(format!("chocolatey: {e}")),
            }
        } else {
            attempts.push("chocolatey: aucun paquet trouve".into());
        }
    } else {
        attempts.push("chocolatey: bootstrap impossible".into());
    }

    if scoop::bootstrap_scoop().is_ok() {
        if let Some(sid) = scoop::search_scoop_id(&app.name) {
            log(window, &app.name, &format!("Tentative via Scoop ({sid})..."), "info");
            match scoop::install_via_scoop(&sid, window) {
                Ok(r) if r.success && (scoop_is_installed(&sid) || verify_installed(&app.name)) => return Ok(r),
                Ok(r) => attempts.push(format!("scoop: {}", r.message)),
                Err(e) => attempts.push(format!("scoop: {e}")),
            }
        } else {
            attempts.push("scoop: aucun paquet trouve".into());
        }
    } else {
        attempts.push("scoop: bootstrap impossible".into());
    }

    if let Some(url) = app.url.as_deref().filter(|u| !u.is_empty()) {
        match install_from_url(&app.name, url, window) {
            Ok(r) if r.success && verify_installed(&app.name) => return Ok(r),
            Ok(r) => attempts.push(format!("direct: {}", r.message)),
            Err(e) => attempts.push(format!("direct: {e}")),
        }
    }

    Ok(InstallResult {
        success: false,
        app_id: app.id.clone(),
        message: if attempts.is_empty() {
            "Aucune methode d'installation disponible pour cette application".into()
        } else {
            format!("Toutes les methodes ont echoue — {}", attempts.join(" | "))
        },
    })
}

fn winget_uninstall(id: &str) -> Result<bool, NiTriTeError> {
    let mut cmd = Command::new("winget");
    cmd.args([
        "uninstall", "--id", id, "--exact", "--silent",
        "--accept-source-agreements", "--disable-interactivity",
    ])
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .creation_flags(0x08000000);
    Ok(run_with_timeout(cmd, UNINSTALL_TIMEOUT)?.success())
}

fn choco_uninstall(id: &str) -> Result<bool, NiTriTeError> {
    let mut cmd = Command::new(chocolatey::choco_exe());
    cmd.args(["uninstall", id, "-y", "--no-progress"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000);
    Ok(run_with_timeout(cmd, UNINSTALL_TIMEOUT)?.success())
}

fn scoop_uninstall(id: &str) -> Result<bool, NiTriTeError> {
    let mut cmd = Command::new(scoop::scoop_exe());
    cmd.args(["uninstall", id])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000);
    Ok(run_with_timeout(cmd, UNINSTALL_TIMEOUT)?.success())
}

fn ok_uninstalled(app: &AppEntry, via: &str) -> InstallResult {
    InstallResult { success: true, app_id: app.id.clone(), message: format!("Desinstallation reussie ({via})") }
}

/// Meme logique de cascade que l'installation, dans le meme ordre de
/// fiabilite, et verifiee pour de vrai (l'app doit avoir disparu). Dernier
/// recours : la commande de desinstallation du registre, lancee en mode
/// silencieux (MSI /qn, Inno /VERYSILENT, NSIS /S) — couvre les apps
/// installees a la main ou par telechargement direct, qu'aucun gestionnaire
/// de paquets ne connait.
pub fn uninstall_app_smart(app: &AppEntry) -> Result<InstallResult, NiTriTeError> {
    let mut attempts: Vec<String> = Vec::new();

    if let Some(wid) = app.winget_id.as_deref().filter(|w| !w.is_empty()) {
        if winget_is_installed(wid) {
            match winget_uninstall(wid) {
                Ok(true) if !winget_is_installed(wid) => return Ok(ok_uninstalled(app, "winget")),
                Ok(_) => attempts.push("winget: echec ou app toujours presente".into()),
                Err(e) => attempts.push(format!("winget: {e}")),
            }
        }
    }

    if chocolatey::check_chocolatey() {
        let choco_id = app.choco_id.clone().filter(|c| !c.is_empty()).or_else(|| chocolatey::search_choco_id(&app.name));
        if let Some(cid) = choco_id.filter(|c| choco_is_installed(c)) {
            match choco_uninstall(&cid) {
                Ok(true) if !choco_is_installed(&cid) => return Ok(ok_uninstalled(app, "Chocolatey")),
                Ok(_) => attempts.push("chocolatey: echec ou app toujours presente".into()),
                Err(e) => attempts.push(format!("chocolatey: {e}")),
            }
        }
    }

    if scoop::check_scoop() {
        if let Some(sid) = scoop::search_scoop_id(&app.name).filter(|s| scoop_is_installed(s)) {
            match scoop_uninstall(&sid) {
                Ok(true) if !scoop_is_installed(&sid) => return Ok(ok_uninstalled(app, "Scoop")),
                Ok(_) => attempts.push("scoop: echec ou app toujours presente".into()),
                Err(e) => attempts.push(format!("scoop: {e}")),
            }
        }
    }

    // Registre : toutes les entrees qui designent cette app (une app peut en
    // avoir plusieurs, ex. x86 + x64). Chacune doit disparaitre.
    let entries: Vec<_> = uninstaller::list_installed_apps()
        .into_iter()
        .filter(|a| app_match::names_match(&app.name, &a.name))
        .collect();
    if !entries.is_empty() {
        let mut all_ok = true;
        for e in &entries {
            if !uninstaller::uninstall_silent(&e.name, &e.uninstall_string) {
                all_ok = false;
                attempts.push(format!("registre: echec pour « {} »", e.name));
            }
        }
        if all_ok && !verify_installed(&app.name) {
            return Ok(ok_uninstalled(app, "desinstalleur natif"));
        }
    } else if attempts.is_empty() {
        // Rien ne la connait : ni les gestionnaires, ni le registre.
        return Ok(InstallResult {
            success: true,
            app_id: app.id.clone(),
            message: "Application deja absente du systeme".into(),
        });
    }

    Ok(InstallResult {
        success: false,
        app_id: app.id.clone(),
        message: if attempts.is_empty() {
            "Aucune methode de desinstallation automatique n'a abouti — passez par Parametres > Applications".into()
        } else {
            format!("Toutes les methodes ont echoue — {}", attempts.join(" | "))
        },
    })
}

/// Detection en une passe des apps du catalogue deja installees : registre
/// Uninstall (noms) + `winget export` (identifiants exacts). Renvoie les `id`
/// du catalogue. Une seule lecture du registre et un seul appel winget pour
/// tout le catalogue (pas un appel par app : 700+ apps).
pub fn detect_installed(apps: &[AppEntry]) -> Vec<String> {
    let names: Vec<String> = uninstaller::list_installed_apps().into_iter().map(|a| a.name).collect();
    let winget_ids = winget::installed_ids();
    apps.iter()
        .filter(|a| {
            a.winget_id.as_deref().is_some_and(|w| winget_ids.contains(&w.to_lowercase()))
                || app_match::any_installed(&a.name, &names)
        })
        .map(|a| a.id.clone())
        .collect()
}
