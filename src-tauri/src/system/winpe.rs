//! winpe.rs — Commandes spécifiques au mode Windows PE
//! Détection, réparation MBR/BCD, SFC offline, reset mot de passe, etc.
use serde::{Deserialize, Serialize};
use crate::error::NiTriTeError;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const NO_WINDOW: u32 = 0x08000000;

// ── Structures ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeDrive {
    pub letter: String,
    pub label: String,
    pub size_gb: f64,
    pub free_gb: f64,
    pub fs: String,
    pub is_system: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeSystemInfo {
    pub is_winpe: bool,
    pub pe_version: String,
    pub cpu: String,
    pub ram_gb: f64,
    pub drives: Vec<PeDrive>,
    pub arch: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepairResult {
    pub success: bool,
    pub output: String,
    pub command: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OfflineUser {
    pub username: String,
    pub full_name: String,
    pub account_type: String,
    pub enabled: bool,
}

// ── Détection WinPE ─────────────────────────────────────────────────────────

pub(crate) fn detect_winpe() -> bool {
    // Méthode 1 : clé registre MiniNT présente uniquement en WinPE
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let out = Command::new("reg")
            .args(["query", r"HKLM\System\CurrentControlSet\Control\MiniNT"])
            .creation_flags(NO_WINDOW)
            .output();
        if let Ok(o) = out {
            if o.status.success() {
                return true;
            }
        }
    }
    // Méthode 2 : disque X: (lettre de la RAM disk WinPE)
    if std::env::var("SYSTEMDRIVE").map(|v| v.to_uppercase() == "X:").unwrap_or(false) {
        return true;
    }
    // Méthode 3 : fichier wpeinit.exe sur X:
    std::path::Path::new(r"X:\Windows\System32\wpeinit.exe").exists()
}

// ── Commandes Tauri ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn is_winpe_mode() -> Result<bool, NiTriTeError> {
    tokio::task::spawn_blocking(detect_winpe)
        .await
        .map_err(|e| NiTriTeError::System(e.to_string()))
}

#[tauri::command]
pub async fn get_pe_system_info() -> Result<PeSystemInfo, NiTriTeError> {
    tokio::task::spawn_blocking(|| {
        let is_winpe = detect_winpe();

        // CPU et RAM par l'API Windows (sysinfo), sans processus externe :
        // `wmic` est retire de Windows 11 24H2/25H2 (et des WinPE recents) et
        // PowerShell est un composant facultatif de WinPE. Avec wmic absent,
        // la page affichait « Inconnu », 0 Go et AUCUN lecteur.
        let mut sys = sysinfo::System::new();
        sys.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing());
        sys.refresh_memory();
        let cpu = sys.cpus().first()
            .map(|c| c.brand().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Inconnu".to_string());
        let ram_gb = sys.total_memory() as f64 / 1_073_741_824.0;

        // Drives
        let drives = collect_pe_drives();

        // PE version
        let pe_version = if is_winpe {
            get_pe_version().unwrap_or_else(|| "WinPE 11".to_string())
        } else {
            "N/A".to_string()
        };

        let arch = std::env::var("PROCESSOR_ARCHITECTURE").unwrap_or_else(|_| "AMD64".to_string());

        Ok(PeSystemInfo { is_winpe, pe_version, cpu, ram_gb, drives, arch })
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

fn get_pe_version() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("reg")
            .args(["query", r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion", "/v", "ReleaseId"])
            .creation_flags(NO_WINDOW)
            .output()
            .ok()?;
        let s = crate::maintenance::commands::decode_output(&out.stdout);
        let ver = s.lines()
            .find(|l| l.contains("ReleaseId"))?
            .split_whitespace()
            .last()?
            .to_string();
        Some(format!("WinPE {}", ver))
    }
    #[cfg(not(target_os = "windows"))]
    { None }
}

fn collect_pe_drives() -> Vec<PeDrive> {
    // API Windows (sysinfo) plutot que `wmic logicaldisk` : voir get_pe_system_info.
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut drives: Vec<PeDrive> = disks.iter()
        .filter_map(|d| {
            let letter: String = d.mount_point().to_string_lossy().trim_end_matches('\\').to_uppercase();
            if letter.len() != 2 || !letter.ends_with(':') { return None; }
            let is_system = letter == "C:" || letter == "X:";
            Some(PeDrive {
                label: d.name().to_string_lossy().into_owned(),
                size_gb: d.total_space() as f64 / 1_073_741_824.0,
                free_gb: d.available_space() as f64 / 1_073_741_824.0,
                fs: d.file_system().to_string_lossy().into_owned(),
                is_system,
                letter,
            })
        })
        .collect();
    drives.sort_by(|a, b| a.letter.cmp(&b.letter));
    drives
}

#[tauri::command]
pub async fn get_pe_drives() -> Result<Vec<PeDrive>, NiTriTeError> {
    tokio::task::spawn_blocking(collect_pe_drives)
        .await
        .map_err(|e| NiTriTeError::System(e.to_string()))
}

// ── Réparation Boot ─────────────────────────────────────────────────────────

fn run_bootrec(args: &[&str]) -> RepairResult {
    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("bootrec")
            .args(args)
            .creation_flags(NO_WINDOW)
            .output();
        match out {
            Ok(o) => {
                let stdout = crate::maintenance::commands::decode_output(&o.stdout).to_string();
                let stderr = crate::maintenance::commands::decode_output(&o.stderr).to_string();
                let combined = if stderr.is_empty() { stdout } else { format!("{}\n{}", stdout, stderr) };
                RepairResult {
                    success: o.status.success(),
                    output: combined.trim().to_string(),
                    command: format!("bootrec {}", args.join(" ")),
                }
            }
            Err(e) => RepairResult {
                success: false,
                output: e.to_string(),
                command: format!("bootrec {}", args.join(" ")),
            },
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        RepairResult {
            success: false,
            output: "Non disponible hors Windows".to_string(),
            command: format!("bootrec {}", args.join(" ")),
        }
    }
}

#[tauri::command]
pub async fn repair_mbr() -> Result<RepairResult, NiTriTeError> {
    tokio::task::spawn_blocking(|| run_bootrec(&["/fixmbr"]))
        .await
        .map_err(|e| NiTriTeError::System(e.to_string()))
}

#[tauri::command]
pub async fn repair_boot() -> Result<RepairResult, NiTriTeError> {
    tokio::task::spawn_blocking(|| run_bootrec(&["/fixboot"]))
        .await
        .map_err(|e| NiTriTeError::System(e.to_string()))
}

#[tauri::command]
pub async fn rebuild_bcd() -> Result<RepairResult, NiTriTeError> {
    tokio::task::spawn_blocking(|| run_bootrec(&["/rebuildbcd"]))
        .await
        .map_err(|e| NiTriTeError::System(e.to_string()))
}

#[tauri::command]
pub async fn scan_os_installations() -> Result<RepairResult, NiTriTeError> {
    tokio::task::spawn_blocking(|| run_bootrec(&["/scanos"]))
        .await
        .map_err(|e| NiTriTeError::System(e.to_string()))
}

// ── ChkDsk & SFC Offline ────────────────────────────────────────────────────

#[tauri::command]
pub async fn run_chkdsk_pe(drive: String, fix: bool) -> Result<RepairResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let mut args = vec![drive.as_str()];
            if fix { args.push("/f"); }
            args.push("/r");
            let out = std::process::Command::new("chkdsk")
                .args(&args)
                .creation_flags(NO_WINDOW)
                .output();
            match out {
                Ok(o) => {
                    let output = crate::maintenance::commands::decode_output(&o.stdout).trim().to_string();
                    Ok(RepairResult {
                        success: o.status.code().map(|c| c < 8).unwrap_or(false),
                        output,
                        command: format!("chkdsk {} {}", drive, if fix { "/f /r" } else { "/r" }),
                    })
                }
                Err(e) => Ok(RepairResult { success: false, output: e.to_string(), command: "chkdsk".to_string() }),
            }
        }
        #[cfg(not(target_os = "windows"))]
        { Ok(RepairResult { success: false, output: "Non Windows".to_string(), command: "chkdsk".to_string() }) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

#[tauri::command]
pub async fn run_sfc_offline(windows_dir: String) -> Result<RepairResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            // Canonicalisation du chemin pour éviter le path traversal
            let canon = std::fs::canonicalize(&windows_dir)
                .map_err(|e| NiTriTeError::System(format!("Chemin windows_dir invalide: {}", e)))?;

            let canon_str = canon.to_str()
                .ok_or_else(|| NiTriTeError::System("Chemin windows_dir contient des caractères non UTF-8".to_string()))?
                .to_string();

            // Vérifie que le chemin est absolu (commence par une lettre de lecteur ex: C:\)
            let is_abs_win = canon_str.len() >= 3
                && canon_str.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
                && canon_str[1..].starts_with(":\\");
            if !is_abs_win {
                return Err(NiTriTeError::System(format!(
                    "windows_dir doit être un chemin absolu Windows valide (ex: C:\\Windows), reçu: {}",
                    canon_str
                )));
            }

            // Vérifie que System32 existe dans ce répertoire (confirme que c'est bien un répertoire Windows)
            let system32_path = canon.join("System32");
            if !system32_path.exists() {
                return Err(NiTriTeError::System(format!(
                    "{}\\System32 introuvable — ce n'est pas un répertoire Windows valide.",
                    canon_str
                )));
            }

            // SFC /scannow /offbootdir=C:\ /offwindir=C:\Windows
            let drive = canon_str[..2].to_string() + "\\";
            let windir = canon_str.clone();
            let out = std::process::Command::new("sfc")
                .args([
                    "/scannow",
                    &format!("/offbootdir={}", drive),
                    &format!("/offwindir={}", windir),
                ])
                .creation_flags(NO_WINDOW)
                .output();
            match out {
                Ok(o) => {
                    let output = crate::maintenance::commands::decode_output(&o.stdout).trim().to_string();
                    Ok(RepairResult {
                        success: o.status.success(),
                        output,
                        command: format!("sfc /scannow /offbootdir={} /offwindir={}", drive, windir),
                    })
                }
                Err(e) => Ok(RepairResult { success: false, output: e.to_string(), command: "sfc".to_string() }),
            }
        }
        #[cfg(not(target_os = "windows"))]
        { Ok(RepairResult { success: false, output: "Non Windows".to_string(), command: "sfc".to_string() }) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

// ── Comptes utilisateurs offline ────────────────────────────────────────────

/// Guard RAII pour décharger automatiquement une ruche de registre, même en cas d'erreur.
#[cfg(target_os = "windows")]
pub(crate) struct RegHiveGuard {
    pub(crate) hive_key: String,
}

#[cfg(target_os = "windows")]
impl Drop for RegHiveGuard {
    fn drop(&mut self) {
        let _ = std::process::Command::new("reg")
            .args(["unload", &self.hive_key])
            .creation_flags(NO_WINDOW)
            .status();
    }
}

#[tauri::command]
pub async fn list_offline_users(windows_dir: String) -> Result<Vec<OfflineUser>, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            // Monte la ruche SAM de l'OS offline
            let sam_path = format!("{}\\System32\\config\\SAM", windows_dir);
            if !std::path::Path::new(&sam_path).exists() {
                return Err(NiTriTeError::System(format!("SAM introuvable: {}", sam_path)));
            }
            // Charge la ruche dans HKLM\OFFLINE_SAM
            // Echec ignore avant : liste vide sans explication (ruche du Windows
            // en cours verrouillee, droits admin manquants).
            let charge = std::process::Command::new("reg")
                .args(["load", r"HKLM\OFFLINE_SAM", &sam_path])
                .creation_flags(NO_WINDOW)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !charge {
                return Err(NiTriTeError::System(format!(
                    "Impossible de charger la ruche {sam_path} (Windows en cours d'exécution ou droits administrateur manquants)."
                )));
            }

            // Guard RAII : décharge la ruche à la sortie du scope (succès ou erreur)
            let _hive_guard = RegHiveGuard { hive_key: r"HKLM\OFFLINE_SAM".to_string() };

            // Enumère les comptes sous HKLM\OFFLINE_SAM\SAM\Domains\Account\Users\Names
            let out = std::process::Command::new("reg")
                .args(["query", r"HKLM\OFFLINE_SAM\SAM\Domains\Account\Users\Names"])
                .creation_flags(NO_WINDOW)
                .output()
                .map(|o| o.stdout)
                .unwrap_or_default();

            let users: Vec<OfflineUser> = crate::maintenance::commands::decode_output(&out)
                .lines()
                .filter(|l| l.trim().starts_with(r"HKLM\"))
                .map(|l| {
                    let username = l.rsplit('\\').next().unwrap_or("?").to_string();
                    OfflineUser {
                        username: username.clone(),
                        full_name: username,
                        account_type: "Utilisateur".to_string(),
                        enabled: true,
                    }
                })
                .collect();

            // _hive_guard dropped ici → reg unload garanti
            Ok(users)
        }
        #[cfg(not(target_os = "windows"))]
        { Ok(vec![]) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

#[tauri::command]
pub async fn reset_user_password(windows_dir: String, username: String, new_password: String) -> Result<RepairResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            garde_net_user(&username, Some(&new_password))?;
            // Utilise net user via l'image offline (Windows installé) — nécessite que l'OS soit accessible
            let drive = windows_dir.get(..2).unwrap_or("C:").to_string();
            let system32 = format!("{}\\Windows\\System32", drive);
            let net_exe = format!("{}\\net.exe", system32);

            if !std::path::Path::new(&net_exe).exists() {
                return Ok(RepairResult {
                    success: false,
                    output: format!("net.exe introuvable dans {}", system32),
                    command: "net user".to_string(),
                });
            }

            let out = std::process::Command::new(&net_exe)
                .args(["user", &username, &new_password])
                .creation_flags(NO_WINDOW)
                .output();

            match out {
                Ok(o) => Ok(RepairResult {
                    success: o.status.success(),
                    output: crate::maintenance::commands::decode_output(&o.stdout).trim().to_string(),
                    command: format!("net user {} ****", username),
                }),
                Err(e) => Ok(RepairResult { success: false, output: e.to_string(), command: "net user".to_string() }),
            }
        }
        #[cfg(not(target_os = "windows"))]
        { Ok(RepairResult { success: false, output: "Non Windows".to_string(), command: "net user".to_string() }) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

// ── Effacement disque ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn disk_wipe(disk_index: u32, method: String) -> Result<RepairResult, NiTriTeError> {
    // Refus catégorique du disque 0 (disque système en WinPE)
    if disk_index == 0 {
        return Err(NiTriTeError::System(
            "Effacement du disque 0 refusé (disque système le plus probable).".to_string(),
        ));
    }

    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            // Le seul refus du disque 0 laissait effacer le disque du Windows en
            // cours quand il n'est pas le n°0 (ordre NVMe/SATA variable), ou la
            // cle WinPE elle-meme.
            match super::winpe_disques::disques_proteges(detect_winpe()) {
                Ok(p) if p.contains(&disk_index) => return Err(NiTriTeError::System(format!(
                    "Effacement refusé : le disque {} porte le Windows en cours ou NiTriTe lui-même.", disk_index))),
                Ok(_) => {}
                Err(e) => return Err(NiTriTeError::System(format!(
                    "Effacement refusé : impossible de vérifier le disque système ({e})."))),
            }
            // method: "quick" = clean, "secure" = clean all (zeros)
            let diskpart_cmd = if method == "secure" {
                format!("select disk {}\nclean all\n", disk_index)
            } else {
                format!("select disk {}\nclean\n", disk_index)
            };

            let script_path = std::env::temp_dir().join("nitrite_wipe.txt");
            std::fs::write(&script_path, &diskpart_cmd)
                .map_err(|e| NiTriTeError::System(e.to_string()))?;

            let out = std::process::Command::new("diskpart")
                .args(["/s", script_path.to_str().unwrap_or("")])
                .creation_flags(NO_WINDOW)
                .output();

            let _ = std::fs::remove_file(&script_path);

            match out {
                Ok(o) => Ok(RepairResult {
                    success: o.status.success(),
                    output: crate::maintenance::commands::decode_output(&o.stdout).trim().to_string(),
                    command: format!("diskpart clean{}", if method == "secure" { " all" } else { "" }),
                }),
                Err(e) => Ok(RepairResult { success: false, output: e.to_string(), command: "diskpart".to_string() }),
            }
        }
        #[cfg(not(target_os = "windows"))]
        { Ok(RepairResult { success: false, output: "Non Windows".to_string(), command: "diskpart".to_string() }) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

// ── DISM offline ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn run_dism_offline_repair(windows_dir: String) -> Result<RepairResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let out = std::process::Command::new("dism")
                .args([
                    "/image:".to_string() + &windows_dir.replace("\\Windows", "\\"),
                    "/cleanup-image".to_string(),
                    "/restorehealth".to_string(),
                ])
                .creation_flags(NO_WINDOW)
                .output();
            match out {
                Ok(o) => {
                    let output = crate::maintenance::commands::decode_output(&o.stdout).trim().to_string();
                    Ok(RepairResult { success: o.status.success(), output, command: "dism /cleanup-image /restorehealth".to_string() })
                }
                Err(e) => Ok(RepairResult { success: false, output: e.to_string(), command: "dism".to_string() }),
            }
        }
        #[cfg(not(target_os = "windows"))]
        { Ok(RepairResult { success: false, output: "Non Windows".to_string(), command: "dism".to_string() }) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

// ── Détection installations Windows ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WindowsInstall {
    pub drive: String,
    pub windows_dir: String,
    pub version: String,
    pub build: String,
}

#[tauri::command]
pub async fn detect_windows_installs() -> Result<Vec<WindowsInstall>, NiTriTeError> {
    tokio::task::spawn_blocking(|| {
        let mut installs = Vec::new();

        #[cfg(target_os = "windows")]
        {
            // Cherche Windows dans chaque lecteur (sauf X: qui est la RAM disk WinPE)
            for letter in ['C', 'D', 'E', 'F', 'G', 'H', 'Y', 'Z'] {
                let win_dir = format!("{}:\\Windows", letter);
                let sys32 = format!("{}:\\Windows\\System32", letter);
                if std::path::Path::new(&sys32).exists() {
                    // Lit la version depuis le registre offline
                    let version_key = format!("{}:\\Windows\\System32\\config\\SOFTWARE", letter);
                    // Windows en cours d'exécution : sa ruche SOFTWARE est verrouillée (reg load
                    // échoue) -> on lit la clé vivante, sinon version « Windows » et build « ? ».
                    let live = std::env::var("SystemRoot")
                        .map(|r| r.to_ascii_uppercase().starts_with(letter))
                        .unwrap_or(false);
                    let (version, build) = if live || std::path::Path::new(&version_key).exists() {
                        let hive_key = if live { "HKLM\\SOFTWARE".to_string() } else { format!("HKLM\\OFFLINE_SW_{}", letter) };
                        if !live {
                            let _ = std::process::Command::new("reg")
                                .args(["load", &hive_key, &version_key])
                                .creation_flags(NO_WINDOW)
                                .status();
                        }

                        // Guard RAII : décharge la ruche à la sortie du scope même en cas d'erreur
                        let _hive_guard = (!live).then(|| RegHiveGuard { hive_key: hive_key.clone() });

                        let ver_out = std::process::Command::new("reg")
                            .args(["query", &format!("{}\\Microsoft\\Windows NT\\CurrentVersion", hive_key), "/v", "DisplayVersion"])
                            .creation_flags(NO_WINDOW)
                            .output()
                            .map(|o| o.stdout)
                            .unwrap_or_default();
                        let build_out = std::process::Command::new("reg")
                            .args(["query", &format!("{}\\Microsoft\\Windows NT\\CurrentVersion", hive_key), "/v", "CurrentBuildNumber"])
                            .creation_flags(NO_WINDOW)
                            .output()
                            .map(|o| o.stdout)
                            .unwrap_or_default();
                        // _hive_guard dropped à la fin de ce bloc if → reg unload garanti
                        let ver = crate::maintenance::commands::decode_output(&ver_out)
                            .lines()
                            .find(|l| l.contains("DisplayVersion"))
                            .and_then(|l| l.split_whitespace().last())
                            .map(|s| format!("Windows {}", s))
                            .unwrap_or_else(|| "Windows".to_string());
                        let bld = crate::maintenance::commands::decode_output(&build_out)
                            .lines()
                            .find(|l| l.contains("CurrentBuildNumber"))
                            .and_then(|l| l.split_whitespace().last())
                            .unwrap_or("?")
                            .to_string();
                        (ver, bld)
                    } else {
                        ("Windows".to_string(), "?".to_string())
                    };
                    installs.push(WindowsInstall {
                        drive: format!("{}:", letter),
                        windows_dir: win_dir,
                        version,
                        build,
                    });
                }
            }
        }

        Ok(installs)
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

// ── BitLocker ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BitLockerStatus {
    pub drive: String,
    pub encrypted: bool,
    pub locked: bool,
    pub status_text: String,
}

#[tauri::command]
pub async fn get_bitlocker_status(drive: String) -> Result<BitLockerStatus, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let out = std::process::Command::new("manage-bde")
                .args(["-status", &drive])
                .creation_flags(NO_WINDOW)
                .output()
                .map(|o| crate::maintenance::commands::decode_output(&o.stdout).to_string())
                .unwrap_or_default();
            let lower = out.to_lowercase();
            let encrypted = lower.contains("bitlocker") && !lower.contains("protection off") && !lower.contains("fully decrypted");
            let locked = lower.contains("locked") || lower.contains("key protectors: none");
            let status_text = if encrypted && locked { "Chiffré et verrouillé".to_string() }
                else if encrypted { "Chiffré mais accessible".to_string() }
                else { "Pas de BitLocker".to_string() };
            Ok(BitLockerStatus { drive, encrypted, locked, status_text })
        }
        #[cfg(not(target_os = "windows"))]
        { Ok(BitLockerStatus { drive, encrypted: false, locked: false, status_text: "Non Windows".to_string() }) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

#[tauri::command]
pub async fn unlock_bitlocker(drive: String, recovery_key: String) -> Result<RepairResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let out = std::process::Command::new("manage-bde")
                .args(["-unlock", &drive, "-RecoveryPassword", &recovery_key])
                .creation_flags(NO_WINDOW)
                .output();
            match out {
                Ok(o) => {
                    let output = format!("{}\n{}", crate::maintenance::commands::decode_output(&o.stdout).trim(), crate::maintenance::commands::decode_output(&o.stderr).trim()).trim().to_string();
                    Ok(RepairResult { success: o.status.success(), output, command: format!("manage-bde -unlock {} -RecoveryPassword ****", drive) })
                }
                Err(e) => Ok(RepairResult { success: false, output: e.to_string(), command: "manage-bde -unlock".to_string() }),
            }
        }
        #[cfg(not(target_os = "windows"))]
        { Ok(RepairResult { success: false, output: "Non Windows".to_string(), command: "manage-bde".to_string() }) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

// ── Outils mot de passe avancés ───────────────────────────────────────────────

/// Garde commune des actions `net user` « hors ligne ».
///
/// `net.exe` agit TOUJOURS sur les comptes du Windows en cours d'execution,
/// jamais sur la ruche SAM de l'installation choisie. Hors WinPE (page ouverte
/// sur un Windows normal, avec un 2e Windows en double amorcage), « Définir
/// MDP » changeait donc le mot de passe du compte homonyme du PC en marche, et
/// « Activer compte » pouvait activer son Administrateur. Fail-closed.
/// Un argument commencant par `/` est une option de net.exe : un mot de passe
/// « /delete » supprimait le compte.
fn garde_net_user(username: &str, mot_de_passe: Option<&str>) -> Result<(), NiTriTeError> {
    if !detect_winpe() {
        return Err(NiTriTeError::CommandDenied(
            "Action réservée au mode WinPE : hors WinPE, net.exe modifierait les comptes du Windows en cours d'exécution, pas ceux de l'installation choisie.".into(),
        ));
    }
    let invalide = |s: &str| s.starts_with('/') || s.chars().any(|c| c.is_control());
    if username.trim().is_empty() || invalide(username) || mot_de_passe.is_some_and(invalide) {
        return Err(NiTriTeError::CommandDenied("Nom d'utilisateur ou mot de passe invalide (pas de « / » en tête).".into()));
    }
    Ok(())
}

#[tauri::command]
pub async fn clear_offline_password(windows_dir: String, username: String) -> Result<RepairResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            garde_net_user(&username, None)?;
            let drive = windows_dir.get(..2).unwrap_or("C:").to_string();
            let net_exe = format!("{}\\Windows\\System32\\net.exe", drive);
            if !std::path::Path::new(&net_exe).exists() {
                return Ok(RepairResult { success: false, output: format!("net.exe introuvable dans {}", drive), command: "net user".to_string() });
            }
            // net user <username> "" — mot de passe vide
            let out = std::process::Command::new(&net_exe)
                .args(["user", &username, ""])
                .creation_flags(NO_WINDOW)
                .output();
            match out {
                Ok(o) => Ok(RepairResult {
                    success: o.status.success(),
                    output: crate::maintenance::commands::decode_output(&o.stdout).trim().to_string(),
                    command: format!("net user {} [vide]", username),
                }),
                Err(e) => Ok(RepairResult { success: false, output: e.to_string(), command: "net user".to_string() }),
            }
        }
        #[cfg(not(target_os = "windows"))]
        { Ok(RepairResult { success: false, output: "Non Windows".to_string(), command: "net user".to_string() }) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

#[tauri::command]
pub async fn enable_offline_account(windows_dir: String, username: String) -> Result<RepairResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            garde_net_user(&username, None)?;
            let drive = windows_dir.get(..2).unwrap_or("C:").to_string();
            let net_exe = format!("{}\\Windows\\System32\\net.exe", drive);
            if !std::path::Path::new(&net_exe).exists() {
                return Ok(RepairResult { success: false, output: format!("net.exe introuvable dans {}", drive), command: "net user".to_string() });
            }
            let out = std::process::Command::new(&net_exe)
                .args(["user", &username, "/active:yes"])
                .creation_flags(NO_WINDOW)
                .output();
            match out {
                Ok(o) => Ok(RepairResult {
                    success: o.status.success(),
                    output: crate::maintenance::commands::decode_output(&o.stdout).trim().to_string(),
                    command: format!("net user {} /active:yes", username),
                }),
                Err(e) => Ok(RepairResult { success: false, output: e.to_string(), command: "net user".to_string() }),
            }
        }
        #[cfg(not(target_os = "windows"))]
        { Ok(RepairResult { success: false, output: "Non Windows".to_string(), command: "net user".to_string() }) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

// ── Whitelist commandes WinPE ─────────────────────────────────────────────────

/// Vérifie que le premier token de la commande est dans la whitelist autorisée
/// et qu'aucun métacaractère de chaining shell n'est présent.
fn validate_winpe_command(command: &str) -> Result<(), NiTriTeError> {
    const ALLOWED_COMMANDS: &[&str] = &[
        // Réseau
        "ipconfig", "ping", "netsh", "wmic",
        // Disque / Partition
        "diskpart", "chkdsk", "format", "label", "vol", "mountvol", "winsat",
        // Boot
        "bcdedit", "bcdboot", "bootrec",
        // Système / Fichiers
        "sfc", "dism", "reg", "regedit", "robocopy", "xcopy", "dir", "attrib",
        // Processus / Services
        "net", "tasklist", "taskkill", "sc",
        // Infos système
        "systeminfo", "query", "schtasks", "driverquery", "set", "where",
        // Utilitaire cmd (pour pipage diskpart multi-commandes)
        "echo",
        // GUI WinPE
        "explorer", "taskmgr", "msconfig", "eventvwr", "compmgmt",
        "diskmgmt", "services", "notepad", "mstsc",
    ];

    // Block PowerShell injection patterns and arbitrary file redirection.
    // Note: && and || are NOT blocked here because legitimate quick-commands use
    // them for service restart sequences (e.g. "net stop X && net start X").
    // The first-token whitelist below limits which executables can be invoked.
    if command.contains("$(") {
        return Err(NiTriTeError::System(
            "Substitution de commande PowerShell '$(...)' interdite.".to_string()
        ));
    }
    const BLOCKED_CHARS: &[char] = &[';', '`', '<', '>'];
    if let Some(c) = command.chars().find(|c| BLOCKED_CHARS.contains(c)) {
        return Err(NiTriTeError::System(format!(
            "Caractère interdit '{}' dans la commande.",
            c
        )));
    }

    if command.trim().is_empty() {
        return Err(NiTriTeError::System("Commande vide.".to_string()));
    }

    // PowerShell cmdlets (Get-*, Set-*, etc.) are routed to powershell.exe -Command,
    // not cmd.exe /c, so they cannot chain shell commands via metacharacters.
    // The metacharacter blocking above already covers injection in these commands.
    const PS_PREFIXES: &[&str] = &[
        "Get-", "Set-", "Start-", "Stop-", "New-", "Remove-", "Add-",
        "Invoke-", "Test-", "Write-", "ConvertFrom-", "Format-",
        "Where-", "Sort-", "Select-",
    ];
    if PS_PREFIXES.iter().any(|p| command.starts_with(p)) {
        return Ok(());
    }

    // Strip a leading '(' to support grouped diskpart pipe sequences such as
    // "(echo select disk 0 & echo clean) | diskpart". Parentheses are cmd.exe
    // grouping operators and introduce no additional injection surface.
    let stripped = command.trim().trim_start_matches('(');
    let first_token = stripped
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();

    // Extrait le nom de fichier sans extension au cas où le chemin complet est fourni
    let token_name = std::path::Path::new(&first_token)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&first_token)
        .to_string();

    if ALLOWED_COMMANDS.contains(&token_name.as_str()) {
        Ok(())
    } else {
        Err(NiTriTeError::System(format!(
            "Commande non autorisée: '{}'. Seules les commandes système WinPE sont acceptées.",
            first_token
        )))
    }
}

// ── Exécution de commande générique WinPE ─────────────────────────────────────

#[tauri::command]
pub async fn winpe_run_command(command: String) -> Result<RepairResult, NiTriTeError> {
    // Validation whitelist avant d'exécuter quoi que ce soit
    validate_winpe_command(&command)?;

    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let is_ps = command.starts_with("Get-") || command.starts_with("$")
                || command.starts_with("Set-") || command.starts_with("Start-")
                || command.starts_with("Stop-") || command.contains("-Object")
                || command.contains("Write-Host") || command.contains("Where-Object");
            let output = if is_ps {
                let ps_utf8 = format!(
                    "$OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; {}",
                    command
                );
                std::process::Command::new("powershell.exe")
                    .args(["-NoProfile", "-NonInteractive", "-Command", &ps_utf8])
                    .creation_flags(NO_WINDOW).output()
            } else {
                std::process::Command::new("cmd.exe")
                    .args(["/c", &command])
                    .creation_flags(NO_WINDOW).output()
            };
            match output {
                Ok(o) => {
                    let stdout = crate::maintenance::commands::decode_output(&o.stdout).to_string();
                    let stderr = crate::maintenance::commands::decode_output(&o.stderr).to_string();
                    let combined = if stderr.trim().is_empty() { stdout.trim().to_string() }
                        else { format!("{}\n[stderr] {}", stdout.trim(), stderr.trim()) };
                    Ok(RepairResult { success: o.status.success(), output: combined, command })
                }
                Err(e) => Ok(RepairResult { success: false, output: e.to_string(), command }),
            }
        }
        #[cfg(not(target_os = "windows"))]
        { Ok(RepairResult { success: false, output: "Disponible en WinPE uniquement.".to_string(), command }) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ce PC n'est pas en WinPE : les actions net user doivent etre refusees
    // (elles modifieraient les comptes du Windows en cours).
    #[test]
    fn net_user_refuse_hors_winpe() {
        assert!(!detect_winpe());
        assert!(garde_net_user("Momo", Some("x")).is_err());
    }

    #[test]
    fn drives_viennent_de_l_api_windows() {
        let d = collect_pe_drives();
        assert!(d.iter().any(|x| x.letter == "C:" && x.size_gb > 0.0));
    }

    #[test]
    fn winpe_cmd_allowed_list() {
        assert!(validate_winpe_command("ipconfig /all").is_ok());
        assert!(validate_winpe_command("sfc /scannow").is_ok());
        assert!(validate_winpe_command("diskpart").is_ok());
        assert!(validate_winpe_command("dism /Online /Cleanup-Image /RestoreHealth").is_ok());
        assert!(validate_winpe_command("chkdsk C: /f").is_ok());
        // Process / service management
        assert!(validate_winpe_command("tasklist /fo table").is_ok());
        assert!(validate_winpe_command("taskkill /f /pid 1234").is_ok());
        assert!(validate_winpe_command("sc query state= all").is_ok());
        // System info
        assert!(validate_winpe_command("systeminfo").is_ok());
        assert!(validate_winpe_command("query user").is_ok());
        assert!(validate_winpe_command("driverquery /fo table").is_ok());
        // echo for diskpart piping
        assert!(validate_winpe_command("echo list disk | diskpart").is_ok());
        assert!(validate_winpe_command("echo list volume | diskpart").is_ok());
        // GUI tools
        assert!(validate_winpe_command("notepad").is_ok());
        assert!(validate_winpe_command("taskmgr").is_ok());
        assert!(validate_winpe_command("msconfig").is_ok());
    }

    #[test]
    fn winpe_cmd_allows_ps_cmdlets() {
        // PowerShell cmdlets bypass first-token check (routed to powershell.exe)
        assert!(validate_winpe_command("Get-Process | Select-Object Name,Id | Format-Table").is_ok());
        assert!(validate_winpe_command("Get-PhysicalDisk | Select-Object FriendlyName,HealthStatus").is_ok());
        assert!(validate_winpe_command("Get-Volume | Format-Table").is_ok());
        assert!(validate_winpe_command("Get-Service | Where-Object { $_.Status -eq 'Running' }").is_ok());
        assert!(validate_winpe_command("Get-NetAdapter | Format-Table").is_ok());
    }

    #[test]
    fn winpe_cmd_allows_grouped_diskpart() {
        // Parenthetical grouping for multi-command diskpart sequences
        assert!(validate_winpe_command("(echo select disk 0 & echo list partition) | diskpart").is_ok());
        assert!(validate_winpe_command("(echo select volume 1 & echo assign letter=D) | diskpart").is_ok());
        assert!(validate_winpe_command("(echo select disk 0 & echo select partition 1 & echo extend) | diskpart").is_ok());
    }

    #[test]
    fn winpe_cmd_blocked_unknown() {
        assert!(validate_winpe_command("cmd /c del *.*").is_err());
        assert!(validate_winpe_command("powershell -c rm -rf /").is_err());
        assert!(validate_winpe_command("").is_err());
        assert!(validate_winpe_command("printf 'x' | diskpart").is_err());
    }

    #[test]
    fn winpe_cmd_full_path_extracted() {
        // If a full path is provided, only the filename stem is checked
        assert!(validate_winpe_command("C:\\Windows\\System32\\sfc.exe /scannow").is_ok());
    }

    #[test]
    fn winpe_cmd_blocks_injection_patterns() {
        // Semicolon (PowerShell command separator)
        assert!(validate_winpe_command("ipconfig; del /q C:\\*").is_err());
        // Redirection — could overwrite arbitrary files
        assert!(validate_winpe_command("ipconfig > C:\\evil.txt").is_err());
        assert!(validate_winpe_command("ipconfig < C:\\input.txt").is_err());
        // Backtick (PowerShell command substitution)
        assert!(validate_winpe_command("ipconfig `whoami`").is_err());
        // $( substitution (PowerShell)
        assert!(validate_winpe_command("ipconfig $(calc)").is_err());
        // PS cmdlets still blocked if metacharacters present
        assert!(validate_winpe_command("Get-Process; Remove-Item C:\\*").is_err());
    }

    #[test]
    fn winpe_cmd_allows_legitimate_chaining() {
        // && is needed for legitimate service restart sequences
        assert!(validate_winpe_command("net stop spooler && net start spooler").is_ok());
        assert!(validate_winpe_command("bcdedit /set {current} safeboot network && bcdedit /set {current} safebootalternateshell yes").is_ok());
        // || is ok too (conditional execution)
        assert!(validate_winpe_command("net stop wuauserv || echo failed").is_ok());
    }
}
