//! Module principal de sauvegarde
pub mod collectors;
pub mod render;
pub mod formatters;
pub use collectors::*;
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

    Ok(String::from_utf8_lossy(&result?.stdout).to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupManifest {
    pub timestamp: String,
    pub items: Vec<BackupItem>,
    pub total_items: usize,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupItem {
    pub name: String,
    pub category: String,
    pub data: String,
    pub size_bytes: usize,
}

pub fn create_backup(selected_items: Vec<String>, format: String, custom_path: Option<String>) -> Result<BackupManifest, NiTriTeError> {
    let mut items = Vec::new();
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    for item_id in &selected_items {
        match item_id.as_str() {
            "installed_apps" => {
                if let Ok(data) = collect_installed_apps() {
                    items.push(BackupItem { name: "Applications installees".into(), category: "Systeme".into(), size_bytes: data.len(), data });
                }
            }
            "drivers" => {
                if let Ok(data) = collect_drivers() {
                    items.push(BackupItem { name: "Pilotes systeme".into(), category: "Systeme".into(), size_bytes: data.len(), data });
                }
            }
            "network_config" => {
                if let Ok(data) = collect_network_config() {
                    items.push(BackupItem { name: "Configuration reseau".into(), category: "Reseau".into(), size_bytes: data.len(), data });
                }
            }
            "startup_programs" => {
                if let Ok(data) = collect_startup() {
                    items.push(BackupItem { name: "Programmes demarrage".into(), category: "Systeme".into(), size_bytes: data.len(), data });
                }
            }
            "env_variables" => {
                if let Ok(data) = collect_env_vars() {
                    items.push(BackupItem { name: "Variables d'environnement".into(), category: "Systeme".into(), size_bytes: data.len(), data });
                }
            }
            "firewall_rules" => {
                if let Ok(data) = collect_firewall_rules() {
                    items.push(BackupItem { name: "Regles pare-feu".into(), category: "Securite".into(), size_bytes: data.len(), data });
                }
            }
            "chrome_bookmarks" => {
                if let Ok(data) = collect_browser_bookmarks("Google\\Chrome") {
                    items.push(BackupItem { name: "Favoris Chrome".into(), category: "Navigateurs".into(), size_bytes: data.len(), data });
                }
            }
            "edge_bookmarks" => {
                if let Ok(data) = collect_browser_bookmarks("Microsoft\\Edge") {
                    items.push(BackupItem { name: "Favoris Edge".into(), category: "Navigateurs".into(), size_bytes: data.len(), data });
                }
            }
            "brave_bookmarks" => {
                if let Ok(data) = collect_browser_bookmarks("BraveSoftware\\Brave-Browser") {
                    items.push(BackupItem { name: "Favoris Brave".into(), category: "Navigateurs".into(), size_bytes: data.len(), data });
                }
            }
            "windows_license" => {
                if let Ok(data) = collect_windows_license() {
                    items.push(BackupItem { name: "Licence Windows".into(), category: "Licences".into(), size_bytes: data.len(), data });
                }
            }
            "bitlocker_keys" => {
                if let Ok(data) = collect_bitlocker_keys() {
                    items.push(BackupItem { name: "Cles BitLocker".into(), category: "Securite".into(), size_bytes: data.len(), data });
                }
            }
            "office_license" => {
                if let Ok(data) = collect_office_license() {
                    items.push(BackupItem { name: "Licence Office".into(), category: "Licences".into(), size_bytes: data.len(), data });
                }
            }
            "installed_fonts" => {
                if let Ok(data) = collect_installed_fonts() {
                    items.push(BackupItem { name: "Polices installees".into(), category: "Systeme".into(), size_bytes: data.len(), data });
                }
            }
            "scheduled_tasks" => {
                if let Ok(data) = collect_scheduled_tasks() {
                    items.push(BackupItem { name: "Taches planifiees".into(), category: "Systeme".into(), size_bytes: data.len(), data });
                }
            }
            "windows_features" => {
                if let Ok(data) = collect_windows_features() {
                    items.push(BackupItem { name: "Fonctionnalites Windows".into(), category: "Systeme".into(), size_bytes: data.len(), data });
                }
            }
            "folder_sizes" => {
                if let Ok(data) = collect_folder_sizes() {
                    items.push(BackupItem { name: "Taille des dossiers C:\\".into(), category: "Diagnostic".into(), size_bytes: data.len(), data });
                }
            }
            "desktop_files" => {
                if let Ok(data) = collect_desktop_files() {
                    items.push(BackupItem { name: "Fichiers Bureau".into(), category: "Utilisateur".into(), size_bytes: data.len(), data });
                }
            }
            "wifi_passwords" => {
                if let Ok(data) = collect_wifi_passwords() {
                    items.push(BackupItem { name: "Mots de passe WiFi".into(), category: "Reseau".into(), size_bytes: data.len(), data });
                }
            }
            "registry_export" => {
                if let Ok(data) = collect_registry_export() {
                    items.push(BackupItem { name: "Export registre (HKCU)".into(), category: "Systeme".into(), size_bytes: data.len(), data });
                }
            }
            "suspicious_processes" => {
                if let Ok(data) = collect_suspicious_processes() {
                    items.push(BackupItem { name: "Processus suspects".into(), category: "Securite".into(), size_bytes: data.len(), data });
                }
            }
            "winget_export" => {
                if let Ok(data) = collect_winget_export() {
                    items.push(BackupItem { name: "WinGet Export JSON".into(), category: "Systeme".into(), size_bytes: data.len(), data });
                }
            }
            "network_shares" => {
                if let Ok(data) = collect_network_shares() {
                    items.push(BackupItem { name: "Partages reseau".into(), category: "Reseau".into(), size_bytes: data.len(), data });
                }
            }
            "hosts_file" => {
                if let Ok(data) = collect_hosts_file() {
                    items.push(BackupItem { name: "Fichier hosts".into(), category: "Reseau".into(), size_bytes: data.len(), data });
                }
            }
            "ssh_keys" => {
                if let Ok(data) = collect_ssh_keys() {
                    items.push(BackupItem { name: "Cles SSH".into(), category: "Dev".into(), size_bytes: data.len(), data });
                }
            }
            "pip_packages" => {
                if let Ok(data) = collect_pip_packages() {
                    items.push(BackupItem { name: "Packages Python (pip)".into(), category: "Dev".into(), size_bytes: data.len(), data });
                }
            }
            "vscode_extensions" => {
                if let Ok(data) = collect_vscode_extensions() {
                    items.push(BackupItem { name: "Extensions VSCode".into(), category: "Dev".into(), size_bytes: data.len(), data });
                }
            }
            "wsl_config" => {
                if let Ok(data) = collect_wsl_config() {
                    items.push(BackupItem { name: "Config WSL".into(), category: "Dev".into(), size_bytes: data.len(), data });
                }
            }
            "powershell_profile" => {
                if let Ok(data) = collect_powershell_profile() {
                    items.push(BackupItem { name: "Profil PowerShell".into(), category: "Dev".into(), size_bytes: data.len(), data });
                }
            }
            "power_plans" => {
                if let Ok(data) = collect_power_plans() {
                    items.push(BackupItem { name: "Plans alimentation".into(), category: "Systeme".into(), size_bytes: data.len(), data });
                }
            }
            "printer_config" => {
                if let Ok(data) = collect_printer_config() {
                    items.push(BackupItem { name: "Imprimantes".into(), category: "Materiel".into(), size_bytes: data.len(), data });
                }
            }
            "system_components" => {
                if let Ok(data) = collect_system_components() {
                    items.push(BackupItem { name: "Composants PC".into(), category: "Materiel".into(), size_bytes: data.len(), data });
                }
            }
            "battery" => {
                if let Ok(data) = collect_battery() {
                    items.push(BackupItem { name: "Batterie".into(), category: "Materiel".into(), size_bytes: data.len(), data });
                }
            }
            _ => {}
        }
    }

    // Tri par catégorie pour éviter les sections dupliquées dans le rendu
    items.sort_by(|a, b| a.category.cmp(&b.category));
    let total_items = items.len();

    let backup_dir = if let Some(ref p) = custom_path {
        std::path::PathBuf::from(p)
    } else {
        paths::backups_dir()
    };
    std::fs::create_dir_all(&backup_dir)
        .map_err(|e| NiTriTeError::System(format!("Impossible de créer le dossier backups: {}", e)))?;

    let ext = match format.as_str() {
        "html" => "html",
        "md"   => "md",
        "txt"  => "txt",
        _      => "json",
    };
    let backup_file = backup_dir.join(format!("backup_{}.{}", timestamp, ext));
    let path_str = backup_file.to_string_lossy().to_string();

    let manifest = BackupManifest {
        timestamp: timestamp.clone(),
        items: items.clone(),
        total_items,
        path: path_str.clone(),
    };

    let content = match format.as_str() {
        "txt"  => render_txt(&manifest),
        "html" => render_html(&manifest),
        "md"   => render_md(&manifest),
        _      => serde_json::to_string_pretty(&manifest)
            .map_err(|e| NiTriTeError::System(format!("Sérialisation échouée: {}", e)))?,
    };

    std::fs::write(&backup_file, content.as_bytes())
        .map_err(|e| NiTriTeError::System(format!("Écriture fichier échouée ({}): {}", path_str, e)))?;

    Ok(manifest)
}


pub fn list_backups() -> Result<Vec<BackupEntryInfo>, NiTriTeError> {
    let backup_dir = paths::backups_dir();
    let mut backups = Vec::new();

    if backup_dir.exists() {
        for entry in std::fs::read_dir(&backup_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let is_backup = name.starts_with("backup_")
                && (name.ends_with(".json") || name.ends_with(".txt")
                    || name.ends_with(".html") || name.ends_with(".md"));
            if !is_backup { continue; }

            let meta = match entry.metadata()
                .or_else(|_| std::fs::metadata(entry.path()))
            {
                Ok(m) => m,
                Err(_) => continue, // fichier inaccessible — ignorer
            };
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
        assert!(String::from_utf8_lossy(&o.stdout).contains("ok"));
    }
}
