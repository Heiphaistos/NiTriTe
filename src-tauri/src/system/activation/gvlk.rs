//! gvlk.rs — Détection (lecture seule) de l'édition Windows installée, pour
//! affichage dans l'écran d'activation.
//!
//! Ne pose plus aucune clé : poser une GVLK (clé volume générique) via
//! `slmgr /ipk` remplace la clé retail/OEM active et peut désactiver un Windows
//! déjà sous licence si l'activation KMS qui devait suivre échoue ensuite
//! (vécu 2026-11-24 sur le PC de Momo avec le serveur KMS natif, retiré du
//! projet). Ce module ne fait donc plus que lire l'édition, jamais la modifier.

use crate::maintenance::commands::execute_system_command;

/// Édition Windows détectée sur la machine courante.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DetectedEdition {
    /// EditionID brut du registre (ex. "Professional").
    pub edition_id: String,
    /// ProductName lisible (ex. "Windows 11 Pro").
    pub product_name: String,
}

/// Lit une valeur sous HKLM\...\CurrentVersion via `reg query` (décodage OEM/UTF-8 géré).
#[cfg(target_os = "windows")]
fn read_current_version(value: &str) -> Option<String> {
    let r = execute_system_command(
        "reg",
        &[
            "query",
            r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "/v",
            value,
        ],
        20,
    )
    .ok()?;
    if !r.success {
        return None;
    }
    // Ligne type : "    EditionID    REG_SZ    Professional"
    for line in r.stdout.lines() {
        let t = line.trim_start();
        if t.starts_with(value) {
            if let Some(idx) = t.find("REG_SZ") {
                let v = t[idx + "REG_SZ".len()..].trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// Détecte l'édition installée via le registre (EditionID + ProductName).
#[cfg(target_os = "windows")]
pub fn detect_edition() -> DetectedEdition {
    DetectedEdition {
        edition_id: read_current_version("EditionID").unwrap_or_default(),
        product_name: read_current_version("ProductName").unwrap_or_default(),
    }
}

#[cfg(not(target_os = "windows"))]
pub fn detect_edition() -> DetectedEdition {
    DetectedEdition { edition_id: String::new(), product_name: String::new() }
}
