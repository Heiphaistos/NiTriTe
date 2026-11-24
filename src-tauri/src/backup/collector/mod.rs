//! Module principal de sauvegarde
pub mod collectors;
pub mod collectors_extra;
pub mod diagnostic;
pub mod render;
pub mod formatters;
pub use collectors::*;
pub use collectors_extra::*;
pub use render::*;
pub use formatters::*;

use serde::Serialize;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::error::NiTriTeError;
use crate::utils::paths;

/// `Command::output()` borné dans le temps. La sauvegarde enchaîne ~25
/// commandes externes (winget, pip, code, wsl, cscript, PowerShell…) ; une
/// seule qui se fige (winget qui réindexe ses sources, service WSL bloqué)
/// laissait la sauvegarde en cours indéfiniment, la page en chargement
/// infini. Au-delà du délai le processus est tué et la collecte concernée
/// échoue proprement (les autres continuent).
pub(crate) trait OutputTimeout {
    fn output_timeout(&mut self) -> std::io::Result<std::process::Output>;
}

impl OutputTimeout for Command {
    fn output_timeout(&mut self) -> std::io::Result<std::process::Output> {
        const SECS: u64 = 120;
        // Mêmes flux que output() : stdin nul (jamais d'attente d'une saisie).
        let child = self
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        let pid = child.id();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || { let _ = tx.send(child.wait_with_output()); });
        match rx.recv_timeout(std::time::Duration::from_secs(SECS)) {
            Ok(r) => r,
            Err(_) => {
                #[cfg(target_os = "windows")]
                let _ = Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .creation_flags(0x08000000)
                    .output();
                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, format!("commande interrompue après {SECS} s")))
            }
        }
    }
}

/// Écrit le script PS dans un fichier .ps1 temporaire et l'exécute.
/// - Nom unique via UUID (anti-TOCTOU, anti-collision)
/// - ExecutionPolicy RemoteSigned (Bypass supprimé)
/// - to_str() vérifié explicitement (anti-chemin vide silencieux)
/// - Cleanup garanti même en cas d'erreur
fn run_ps_temp(script: &str) -> Result<String, NiTriTeError> {
    let uid = uuid::Uuid::new_v4().simple().to_string();
    let tmp = std::env::temp_dir().join(format!("nitrite_ps_{}.ps1", uid));

    let full_script = format!(
        "$OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8;\n{}",
        script
    );
    // BOM UTF-8 (EF BB BF) obligatoire : PowerShell 5.1 lit un fichier -File SANS
    // BOM en ANSI (Windows-1252), pas en UTF-8. Sans lui, les caractères
    // accentués du script (regex « Contenu de la clé », libellés « Périphérique »…)
    // sont corrompus → extraction WiFi FR cassée et libellés en mojibake.
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(full_script.as_bytes());
    std::fs::write(&tmp, &bytes)
        .map_err(|e| NiTriTeError::System(format!("Écriture script temp: {}", e)))?;

    let tmp_str = tmp.to_str()
        .ok_or_else(|| NiTriTeError::System("Chemin temp non-UTF8".into()))?;

    let result = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "RemoteSigned", "-File", tmp_str])
        .creation_flags(0x08000000)
        .output_timeout()
        .map_err(|e| NiTriTeError::System(format!("Lancement PowerShell: {}", e)));

    // Cleanup garanti quel que soit le résultat
    let _ = std::fs::remove_file(&tmp);

    Ok(crate::maintenance::commands::decode_output(&result?.stdout).to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupManifest {
    pub timestamp: String,
    pub items: Vec<BackupItem>,
    pub total_items: usize,
    /// Éléments réellement collectés avec des données (statut `ok`).
    pub ok_count: usize,
    /// Éléments collectés mais vides (statut `vide`, légitime).
    pub empty_count: usize,
    /// Éléments en échec réel (statut `echec`).
    pub failed_count: usize,
    /// Chemin du lot (dossier horodaté).
    pub path: String,
}

/// Statut de collecte d'un élément — rend visible ce qui a réellement été pris,
/// au lieu du saut silencieux `if let Ok(..)`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BackupStatus {
    /// Données non vides collectées.
    Ok,
    /// Collecté sans erreur mais rien à sauvegarder (ex. aucune imprimante). Légitime.
    Vide,
    /// Erreur réelle de collecte (voir `detail`).
    Echec,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupItem {
    pub id: String,
    pub name: String,
    pub category: String,
    pub data: String,
    pub size_bytes: usize,
    pub status: BackupStatus,
    /// Message d'échec (si `Echec`) ou note (si `Vide`), sinon vide.
    pub detail: String,
    /// Nom du fichier de cet élément dans le lot (vide si rien écrit).
    pub file: String,
}

/// Déclaration d'un collecteur simple : identifiant UI, libellé, catégorie, fonction.
type CollectFn = Box<dyn Fn() -> Result<String, NiTriTeError>>;
struct CollectorDecl {
    id: &'static str,
    name: &'static str,
    category: &'static str,
    run: CollectFn,
}

/// Table de tous les collecteurs « texte ». Remplace l'ancien `match` de ~160 lignes :
/// une seule boucle les exécute et chacun reçoit TOUJOURS un statut (plus de saut
/// silencieux sur échec/vide).
fn collector_table() -> Vec<CollectorDecl> {
    macro_rules! c {
        ($id:expr, $name:expr, $cat:expr, $f:expr) => {
            CollectorDecl { id: $id, name: $name, category: $cat, run: Box::new($f) }
        };
    }
    vec![
        c!("installed_apps", "Applications installees", "Logiciels", collect_installed_apps),
        c!("winget_export", "WinGet Export JSON", "Logiciels", collect_winget_export),
        c!("drivers", "Pilotes systeme", "Logiciels", collect_drivers),
        c!("windows_features", "Fonctionnalites Windows", "Logiciels", collect_windows_features),
        c!("installed_fonts", "Polices installees", "Logiciels", collect_installed_fonts),
        c!("hotfixes", "Correctifs KB installes", "Logiciels", collect_hotfixes),
        c!("network_config", "Configuration reseau", "Reseau", collect_network_config),
        c!("wifi_passwords", "Mots de passe WiFi", "Reseau", collect_wifi_passwords),
        c!("firewall_rules", "Regles pare-feu", "Reseau", collect_firewall_rules),
        c!("network_shares", "Partages reseau", "Reseau", collect_network_shares),
        c!("hosts_file", "Fichier hosts", "Reseau", collect_hosts_file),
        c!("vpn_profiles", "Profils VPN", "Reseau", collect_vpn_profiles),
        c!("windows_license", "Licence Windows", "Licences", collect_windows_license),
        c!("bitlocker_keys", "Cles BitLocker", "Licences", collect_bitlocker_keys),
        c!("office_license", "Licence Office", "Licences", collect_office_license),
        c!("chrome_bookmarks", "Favoris Chrome", "Navigateurs", || collect_browser_bookmarks("Google\\Chrome")),
        c!("edge_bookmarks", "Favoris Edge", "Navigateurs", || collect_browser_bookmarks("Microsoft\\Edge")),
        c!("brave_bookmarks", "Favoris Brave", "Navigateurs", || collect_browser_bookmarks("BraveSoftware\\Brave-Browser")),
        c!("startup_programs", "Programmes demarrage", "Demarrage & Taches", collect_startup),
        c!("scheduled_tasks", "Taches planifiees", "Demarrage & Taches", collect_scheduled_tasks),
        c!("scheduled_tasks_xml", "Taches planifiees (XML)", "Demarrage & Taches", collect_scheduled_tasks_xml),
        c!("registry_export", "Export registre (HKCU)", "Demarrage & Taches", collect_registry_export),
        c!("services", "Services Windows", "Demarrage & Taches", collect_services),
        c!("env_variables", "Variables d'environnement", "Developpeur", collect_env_vars),
        c!("ssh_keys", "Cles SSH", "Developpeur", collect_ssh_keys),
        c!("pip_packages", "Packages Python (pip)", "Developpeur", collect_pip_packages),
        c!("vscode_extensions", "Extensions VSCode", "Developpeur", collect_vscode_extensions),
        c!("wsl_config", "Config WSL", "Developpeur", collect_wsl_config),
        c!("powershell_profile", "Profil PowerShell", "Developpeur", collect_powershell_profile),
        c!("system_components", "Composants PC", "Materiel", collect_system_components),
        c!("power_plans", "Plans alimentation", "Materiel", collect_power_plans),
        c!("printer_config", "Imprimantes", "Materiel", collect_printer_config),
        c!("battery", "Batterie", "Materiel", collect_battery),
        c!("folder_sizes", "Taille des dossiers C", "Diagnostic", collect_folder_sizes),
        c!("desktop_files", "Fichiers Bureau", "Diagnostic", collect_desktop_files),
        c!("suspicious_processes", "Processus suspects", "Diagnostic", collect_suspicious_processes),
        c!("restore_points", "Points de restauration", "Diagnostic", collect_restore_points),
        c!("event_logs", "Journaux d'evenements (7j)", "Diagnostic", collect_event_logs),
    ]
}

/// Classe une sortie de collecteur en statut visible.
fn classify(result: Result<String, NiTriTeError>) -> (BackupStatus, String, String) {
    match result {
        Err(e) => (BackupStatus::Echec, String::new(), format!("{}", e)),
        Ok(data) if data.trim().is_empty() => (
            BackupStatus::Vide,
            String::new(),
            "Collecté sans erreur, mais aucune donnée à sauvegarder.".into(),
        ),
        Ok(data) => (BackupStatus::Ok, data, String::new()),
    }
}

fn item_ext(data: &str) -> &'static str {
    let t = data.trim_start();
    if t.starts_with('{') || t.starts_with('[') { "json" } else { "txt" }
}

pub fn create_backup(selected_items: Vec<String>, format: String, custom_path: Option<String>) -> Result<BackupManifest, NiTriTeError> {
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let table = collector_table();
    let mut items: Vec<BackupItem> = Vec::new();
    let mut file_index = 0usize;

    // Scan PC complet du Diagnostic (gros élément : matériel, sécurité, processus, services…).
    let mut scan_html: Option<(String, String)> = None; // (nom fichier HTML, contenu HTML)
    if selected_items.iter().any(|s| s == "diagnostic_scan") {
        file_index += 1;
        let scan = diagnostic::run_scan_blocking();
        let (status, data, detail, file) = match serde_json::to_string_pretty(&scan) {
            Ok(json) => {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
                    scan_html = Some((format!("{:02}_diagnostic-scan.html", file_index), render_scan_html(&value)));
                }
                (BackupStatus::Ok, json, String::new(), format!("{:02}_diagnostic-scan.json", file_index))
            }
            Err(e) => (BackupStatus::Echec, String::new(), format!("Sérialisation du scan: {}", e), String::new()),
        };
        items.push(BackupItem {
            id: "diagnostic_scan".into(),
            name: "Scan PC complet (Diagnostic)".into(),
            category: "Diagnostic".into(),
            size_bytes: data.len(),
            data,
            status,
            detail,
            file,
        });
    }

    for item_id in &selected_items {
        if item_id == "diagnostic_scan" { continue; }
        let Some(decl) = table.iter().find(|d| d.id == *item_id) else { continue };
        let (status, data, detail) = classify((decl.run)());
        file_index += 1;
        let file = if status == BackupStatus::Ok {
            format!("{:02}_{}.{}", file_index, formatters::safe_filename(decl.name), item_ext(&data))
        } else {
            String::new()
        };
        items.push(BackupItem {
            id: decl.id.to_string(),
            name: decl.name.to_string(),
            category: decl.category.to_string(),
            size_bytes: data.len(),
            data,
            status,
            detail,
            file,
        });
    }

    // Tri par catégorie pour un rendu groupé sans sections dupliquées.
    items.sort_by(|a, b| a.category.cmp(&b.category));
    let total_items = items.len();
    let ok_count = items.iter().filter(|i| i.status == BackupStatus::Ok).count();
    let empty_count = items.iter().filter(|i| i.status == BackupStatus::Vide).count();
    let failed_count = items.iter().filter(|i| i.status == BackupStatus::Echec).count();

    // Un seul lot regroupé : dossier horodaté.
    let base_dir = match custom_path {
        Some(ref p) => std::path::PathBuf::from(p),
        None => paths::backups_dir(),
    };
    let bundle_dir = base_dir.join(format!("backup_{}", timestamp));
    let elements_dir = bundle_dir.join("elements");
    std::fs::create_dir_all(&elements_dir)
        .map_err(|e| NiTriTeError::System(format!("Impossible de créer le lot de sauvegarde: {}", e)))?;

    // Fichier par élément (données brutes, UTF-8, binaire-safe pour hosts/bookmarks/clés).
    for item in &items {
        if item.file.is_empty() { continue; }
        std::fs::write(elements_dir.join(&item.file), item.data.as_bytes())
            .map_err(|e| NiTriTeError::System(format!("Écriture élément {}: {}", item.file, e)))?;
    }
    // Rapport HTML lisible du scan PC.
    if let Some((file, html)) = &scan_html {
        std::fs::write(elements_dir.join(file), html.as_bytes())
            .map_err(|e| NiTriTeError::System(format!("Écriture scan HTML: {}", e)))?;
    }

    let path_str = bundle_dir.to_string_lossy().to_string();
    let manifest = BackupManifest {
        timestamp: timestamp.clone(),
        items,
        total_items,
        ok_count,
        empty_count,
        failed_count,
        path: path_str.clone(),
    };

    // manifeste.json : statuts + tailles (rejouable, vérifiable).
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| NiTriTeError::System(format!("Sérialisation manifeste: {}", e)))?;
    std::fs::write(bundle_dir.join("manifeste.json"), manifest_json.as_bytes())
        .map_err(|e| NiTriTeError::System(format!("Écriture manifeste: {}", e)))?;

    // LISEZ-MOI.html : récapitulatif lisible (statuts visibles).
    std::fs::write(bundle_dir.join("LISEZ-MOI.html"), render_html(&manifest).as_bytes())
        .map_err(|e| NiTriTeError::System(format!("Écriture LISEZ-MOI: {}", e)))?;

    // Rapport combiné au format choisi (continuité txt/md), en plus du HTML/JSON déjà écrits.
    match format.as_str() {
        "txt" => { let _ = std::fs::write(bundle_dir.join("rapport.txt"), render_txt(&manifest).as_bytes()); }
        "md"  => { let _ = std::fs::write(bundle_dir.join("rapport.md"), render_md(&manifest).as_bytes()); }
        _     => {} // html = LISEZ-MOI.html, json = manifeste.json
    }

    Ok(manifest)
}


pub fn list_backups() -> Result<Vec<BackupEntryInfo>, NiTriTeError> {
    let backup_dir = paths::backups_dir();
    let mut backups = Vec::new();

    if backup_dir.exists() {
        for entry in std::fs::read_dir(&backup_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let meta = match entry.metadata()
                .or_else(|_| std::fs::metadata(entry.path()))
            {
                Ok(m) => m,
                Err(_) => continue, // inaccessible — ignorer
            };

            // Nouveau format : lot = dossier backup_<ts>/ avec manifeste.json.
            if meta.is_dir() {
                if !name.starts_with("backup_") { continue; }
                let manifest_path = entry.path().join("manifeste.json");
                if !manifest_path.exists() { continue; }
                let (size, items_count) = match std::fs::read_to_string(&manifest_path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                {
                    Some(v) => {
                        let count = v.get("total_items").and_then(|n| n.as_u64()).unwrap_or(0) as usize;
                        let ok = v.get("ok_count").and_then(|n| n.as_u64()).unwrap_or(0);
                        let failed = v.get("failed_count").and_then(|n| n.as_u64()).unwrap_or(0);
                        (format!("{} ok · {} échec(s)", ok, failed), count)
                    }
                    None => ("—".to_string(), 0),
                };
                let stem = name.strip_prefix("backup_").unwrap_or(&name);
                let date = {
                    let parts: Vec<&str> = stem.splitn(2, '_').collect();
                    match parts.as_slice() {
                        [d, t] => format!("{} {}", d, t.replace('-', ":")),
                        _ => stem.replace('-', "/"),
                    }
                };
                backups.push(BackupEntryInfo { filename: name, date, size, items_count });
                continue;
            }

            // Ancien format : fichier unique backup_<ts>.<ext> (rétro-compat).
            let is_backup = name.starts_with("backup_")
                && (name.ends_with(".json") || name.ends_with(".txt")
                    || name.ends_with(".html") || name.ends_with(".md"));
            if !is_backup { continue; }

            let size_bytes = meta.len();
            let size = if size_bytes < 1_048_576 {
                format!("{:.1} Ko", size_bytes as f64 / 1024.0)
            } else {
                format!("{:.1} Mo", size_bytes as f64 / 1_048_576.0)
            };

            // Parse date from filename: backup_2026-03-09_14-30-00.txt
            let stem = name.strip_prefix("backup_").unwrap_or(&name);
            let stem = stem.split('.').next().unwrap_or(stem);
            let date = {
                // "2026-04-03_14-30-00" → "2026-04-03 14:30:00"
                let parts: Vec<&str> = stem.splitn(2, '_').collect();
                match parts.as_slice() {
                    [date_part, time_part] => format!("{} {}", date_part, time_part.replace('-', ":")),
                    _ => stem.replace('-', "/"),
                }
            };

            backups.push(BackupEntryInfo { filename: name, date, size, items_count: 0 });
        }
    }

    backups.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(backups)
}


#[cfg(test)]
mod timeout_tests {
    use super::*;

    #[test]
    fn output_timeout_returns_output_of_quick_command() {
        let o = Command::new("cmd").args(["/c", "echo", "ok"]).output_timeout().unwrap();
        assert!(o.status.success());
        assert!(crate::maintenance::commands::decode_output(&o.stdout).contains("ok"));
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    #[test]
    fn classify_ok_on_non_empty() {
        let (s, data, detail) = classify(Ok("des données".into()));
        assert_eq!(s, BackupStatus::Ok);
        assert_eq!(data, "des données");
        assert!(detail.is_empty());
    }

    #[test]
    fn classify_vide_on_blank() {
        let (s, data, detail) = classify(Ok("   \n ".into()));
        assert_eq!(s, BackupStatus::Vide);
        assert!(data.is_empty());
        assert!(!detail.is_empty()); // note explicite, jamais un saut silencieux
    }

    #[test]
    fn classify_echec_keeps_error_message() {
        let (s, _data, detail) = classify(Err(NiTriTeError::System("accès refusé".into())));
        assert_eq!(s, BackupStatus::Echec);
        assert!(detail.contains("accès refusé"));
    }

    #[test]
    fn status_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&BackupStatus::Ok).unwrap(), "\"ok\"");
        assert_eq!(serde_json::to_string(&BackupStatus::Vide).unwrap(), "\"vide\"");
        assert_eq!(serde_json::to_string(&BackupStatus::Echec).unwrap(), "\"echec\"");
    }

    #[test]
    fn collector_table_ids_are_unique() {
        let table = collector_table();
        let mut ids: Vec<&str> = table.iter().map(|d| d.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "identifiants de collecteurs dupliqués");
    }

    #[test]
    fn item_ext_detects_json() {
        assert_eq!(item_ext("{\"a\":1}"), "json");
        assert_eq!(item_ext("[1,2]"), "json");
        assert_eq!(item_ext("texte simple"), "txt");
    }

    #[test]
    fn safe_filename_strips_accents_and_specials() {
        assert_eq!(formatters::safe_filename("Taille des dossiers C:\\"), "taille-des-dossiers-c");
        assert_eq!(formatters::safe_filename("Variables d'environnement"), "variables-d-environnement");
        assert!(!formatters::safe_filename("Évènement / spécial").contains('/'));
    }
}
