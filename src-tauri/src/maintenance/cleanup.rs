use serde::Serialize;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::error::NiTriTeError;

#[derive(Debug, Clone, Serialize)]
pub struct CleanupResult {
    pub action: String,
    pub success: bool,
    pub freed_mb: f64,
    pub message: String,
}

pub fn empty_recycle_bin() -> Result<CleanupResult, NiTriTeError> {
    // `Clear-RecycleBin -EA SilentlyContinue` sortait toujours en code 0 : succès
    // affiché même si rien n'était vidé. On compte les éléments avant/après.
    let ps = "$rb = { @((New-Object -ComObject Shell.Application).Namespace(10).Items()).Count };               $before = & $rb; Clear-RecycleBin -Force -ErrorAction SilentlyContinue; $after = & $rb;               Write-Output \"$before|$after\"";
    let out = crate::maintenance::commands::execute_system_command(
        "powershell", &["-NoProfile", "-NonInteractive", "-Command", ps], 120)?;
    let mut parts = out.stdout.trim().rsplit('|');
    let after: Option<u64> = parts.next().and_then(|v| v.trim().parse().ok());
    let before: Option<u64> = parts.next().and_then(|v| v.trim().parse().ok());
    let (success, message) = match (before, after) {
        (Some(0), Some(0)) => (true, "Corbeille deja vide".to_string()),
        (Some(b), Some(0)) => (true, format!("Corbeille videe ({} element(s))", b)),
        (Some(b), Some(a)) => (false, format!("Corbeille partiellement videe : {} element(s) restant(s) sur {}", a, b)),
        _ => (false, format!("Echec du vidage de la corbeille : {}", out.stderr.trim())),
    };
    Ok(CleanupResult { action: "Vider la corbeille".into(), success, freed_mb: 0.0, message })
}

pub fn clean_temp_files() -> Result<CleanupResult, NiTriTeError> {
    let temp_dir = std::env::temp_dir();

    // Validation : %TEMP% doit pointer vers un répertoire utilisateur sûr
    // pour éviter qu'un %TEMP% malicieux ne supprime des fichiers système
    let temp_str = temp_dir.to_string_lossy().to_lowercase();
    let is_safe = temp_str.contains(r"\appdata\local\temp")
        || temp_str.contains(r"\temp")
        || temp_str.contains(r"\tmp");
    if !is_safe {
        return Err(NiTriTeError::CommandDenied(
            format!(
                "Répertoire temporaire suspect (non autorisé pour suppression): {}",
                temp_dir.display()
            ),
        ));
    }

    // Propager un échec de lecture du dossier plutôt que le ravaler en
    // silence : l'ancien code (if let Ok(entries) = ...) sans branche else
    // rendait un succès inconditionnel même si %TEMP% était illisible.
    let entries = std::fs::read_dir(&temp_dir)
        .map_err(|e| NiTriTeError::System(format!("Lecture de {} impossible: {}", temp_dir.display(), e)))?;

    let mut freed: u64 = 0;
    let mut skipped: u32 = 0;

    for entry in entries.flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                // Ne compter la taille libérée QUE si la suppression a
                // réellement réussi — l'ancien code additionnait meta.len()
                // avant même de tenter remove_file() et ignorait son
                // résultat (let _ = ...), donc un fichier verrouillé (cas
                // courant pour %TEMP% : cache navigateur, app en cours)
                // était compté comme libéré alors qu'il ne l'était pas.
                match std::fs::remove_file(entry.path()) {
                    Ok(()) => freed += meta.len(),
                    Err(_) => skipped += 1,
                }
            }
        }
    }

    let message = if skipped > 0 {
        format!("{:.1} MB liberes ({} fichier(s) verrouille(s) ignore(s))", freed as f64 / 1_048_576.0, skipped)
    } else {
        format!("{:.1} MB liberes", freed as f64 / 1_048_576.0)
    };

    Ok(CleanupResult {
        action: "Supprimer fichiers temporaires".into(),
        success: true,
        freed_mb: freed as f64 / 1_048_576.0,
        message,
    })
}

pub fn run_disk_cleanup() -> Result<CleanupResult, NiTriTeError> {
    let status = Command::new("cleanmgr").arg("/d").arg("C:").creation_flags(0x08000000).status()?;

    Ok(CleanupResult {
        action: "Nettoyage disque Windows".into(),
        success: status.success(),
        freed_mb: 0.0,
        message: "Nettoyage disque lance".into(),
    })
}

pub fn get_startup_programs() -> Result<Vec<StartupEntry>, NiTriTeError> {
    // execute_system_command décode via decode_output (champ User accentué :
    // « AUTORITE NT\Système ») et borne la requête WMI dans le temps.
    let output = crate::maintenance::commands::execute_system_command("powershell", &["-NoProfile", "-NonInteractive", "-Command",
            "Get-CimInstance Win32_StartupCommand | Select-Object Name, Command, Location, User | ConvertTo-Json"], 60)?;
    let text = output.stdout;
    let trimmed = text.trim();
    // PowerShell retourne un objet JSON unique si un seul programme est trouvé
    let entries: Vec<serde_json::Value> = serde_json::from_str(trimmed)
        .or_else(|_| {
            serde_json::from_str::<serde_json::Value>(trimmed)
                .map(|obj| if obj.is_object() { vec![obj] } else { vec![] })
        })
        .unwrap_or_default();

    Ok(entries.iter().map(|e| StartupEntry {
        name: e["Name"].as_str().unwrap_or("").to_string(),
        command: e["Command"].as_str().unwrap_or("").to_string(),
        location: e["Location"].as_str().unwrap_or("").to_string(),
        user: e["User"].as_str().unwrap_or("").to_string(),
        enabled: true,
    }).collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct StartupEntry {
    pub name: String,
    pub command: String,
    pub location: String,
    pub user: String,
    pub enabled: bool,
}

/// Traduit la `Location` de Win32_StartupCommand en chemin PowerShell.
/// WMI ne renvoie jamais `HKCU` : les entrées de l'utilisateur arrivent en
/// `HKU\<SID>\...`, celles de la machine en `HKLM\...`. L'ancienne validation
/// (HKCU seul, HKLM/HKU refusés « droits administrateur ») rejetait donc TOUTES
/// les entrées, alors que l'application tourne déjà en administrateur.
fn startup_reg_path(location: &str) -> Result<String, NiTriTeError> {
    const RUN_KEYS: [&str; 4] = [
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\RunOnce",
    ];
    let denied = || NiTriTeError::CommandDenied(format!(
        "Emplacement non pris en charge pour la désactivation : {} (entrées du dossier Démarrage : retirer le raccourci via shell:startup)",
        location
    ));
    let (root, rest) = location.split_once('\\').ok_or_else(denied)?;
    let (ps_root, key) = match root.to_ascii_uppercase().as_str() {
        "HKLM" => ("HKLM:\\".to_string(), rest),
        "HKCU" => ("HKCU:\\".to_string(), rest),
        "HKU" => {
            let (sid, key) = rest.split_once('\\').ok_or_else(denied)?;
            let sid_ok = !sid.is_empty()
                && sid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
            if !sid_ok { return Err(denied()); }
            (format!("Registry::HKEY_USERS\\{}\\", sid), key)
        }
        _ => return Err(denied()),
    };
    if !RUN_KEYS.iter().any(|k| k.eq_ignore_ascii_case(key)) {
        return Err(denied());
    }
    Ok(format!("{}{}", ps_root, key))
}

/// Desactive un programme au demarrage via le registre (suppression de la valeur)
pub fn disable_startup_program(name: &str, location: &str) -> Result<CleanupResult, NiTriTeError> {
    let reg_path = startup_reg_path(location)?;

    // Valider name : pas de métacaractères PS/shell
    if name.contains('\'') || name.contains('"') || name.contains('`') || name.contains('$') || name.is_empty() {
        return Err(NiTriTeError::CommandDenied(
            "Nom de programme invalide pour désactivation démarrage".into(),
        ));
    }

    let ps_script = format!(
        r#"$regPath = '{}'
$entryName = '{}'
Remove-ItemProperty -LiteralPath $regPath -Name $entryName -ErrorAction Stop"#,
        reg_path.replace('\'', "''"),
        name.replace('\'', "''")
    );

    let output = crate::maintenance::commands::execute_system_command(
        "powershell", &["-NoProfile", "-NonInteractive", "-Command", &ps_script], 30)?;

    if output.success {
        Ok(CleanupResult {
            action: format!("Desactiver {}", name),
            success: true,
            freed_mb: 0.0,
            message: format!("{} retire du demarrage", name),
        })
    } else {
        Err(NiTriTeError::System(format!("Impossible de desactiver {}: {}", name, output.stderr.trim())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_reg_path_maps_wmi_locations() {
        // Formats réels renvoyés par Win32_StartupCommand (relevés sur Windows 11 FR).
        assert_eq!(
            startup_reg_path(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run").unwrap(),
            r"HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"
        );
        assert_eq!(
            startup_reg_path(r"HKU\S-1-5-21-114108366-2118826573-2109801171-1001\SOFTWARE\Microsoft\Windows\CurrentVersion\Run").unwrap(),
            r"Registry::HKEY_USERS\S-1-5-21-114108366-2118826573-2109801171-1001\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"
        );
        assert!(startup_reg_path(r"HKU\.DEFAULT\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce").is_ok());
        assert!(startup_reg_path(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run").is_ok());
    }

    #[test]
    fn startup_reg_path_rejects_other_keys() {
        assert!(startup_reg_path(r"HKCU\..\SYSTEM").is_err());
        assert!(startup_reg_path(r"HKLM\SYSTEM\CurrentControlSet\Services").is_err());
        assert!(startup_reg_path(r"HKU\S-1-5-18'; x\SOFTWARE\Microsoft\Windows\CurrentVersion\Run").is_err());
        assert!(startup_reg_path("Startup").is_err());
        assert!(startup_reg_path("Common Startup").is_err());
    }

    #[test]
    fn disable_startup_unknown_path_rejected() {
        let r = disable_startup_program("app", r"HKCU\..\SYSTEM");
        assert!(r.is_err());
    }

    #[test]
    fn disable_startup_metachar_name_rejected() {
        let r = disable_startup_program("evil'; rm -rf /", "HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
        assert!(r.is_err());
        let msg = format!("{:?}", r.unwrap_err());
        assert!(msg.contains("CommandDenied") || msg.contains("invalide") || msg.contains("invalid"));
    }

    #[test]
    fn disable_startup_empty_name_rejected() {
        let r = disable_startup_program("", "HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
        assert!(r.is_err());
    }

    // Live, non-CI (ignorée par défaut) : confirme sur une vraie machine Windows
    // que get_startup_programs() ne produit plus de mojibake sur le champ User
    // (comptes système intégrés eux-mêmes accentués, ex. « AUTORITE NT\Système »,
    // « SERVICE RÉSEAU » — corruption confirmée avant fix via repro Rust direct).
    #[test]
    #[ignore]
    fn get_startup_programs_no_mojibake_on_real_data() {
        let entries = get_startup_programs().expect("get_startup_programs failed");
        let bad = entries.iter().find(|e|
            e.name.contains('\u{FFFD}') || e.command.contains('\u{FFFD}') ||
            e.location.contains('\u{FFFD}') || e.user.contains('\u{FFFD}')
        );
        assert!(bad.is_none(), "mojibake détecté dans une entrée réelle : {:?}", bad);
    }
}
