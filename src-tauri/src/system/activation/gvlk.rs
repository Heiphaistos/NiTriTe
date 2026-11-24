//! gvlk.rs — Clés d'installation client KMS (GVLK) publiées par Microsoft et
//! détection de l'édition Windows installée.
//!
//! Les GVLK sont des clés PUBLIQUES documentées par Microsoft
//! (« KMS client setup keys », learn.microsoft.com). Les poser via `slmgr /ipk`
//! bascule l'édition en canal Volume pour qu'elle puisse s'activer contre un
//! serveur KMS. Rien de secret ici.

use crate::maintenance::commands::execute_system_command;

/// Une édition Windows et sa GVLK officielle.
pub struct Gvlk {
    /// Identifiant interne (EditionID du registre quand c'est possible).
    pub edition_id: &'static str,
    /// Libellé lisible.
    pub label: &'static str,
    /// Clé GVLK officielle Microsoft.
    pub key: &'static str,
}

/// Table des GVLK client (Windows 10/11 desktop + Server courants).
/// Source : documentation Microsoft « KMS client setup keys ».
pub const GVLK_TABLE: &[Gvlk] = &[
    // ── Windows 11 / 10 (même famille d'EditionID) ──
    Gvlk { edition_id: "Professional",            label: "Windows 10/11 Pro",                     key: "W269N-WFGWX-YVC9B-4J6C9-T83GX" },
    Gvlk { edition_id: "ProfessionalN",           label: "Windows 10/11 Pro N",                   key: "MH37W-N47XK-V7XM9-C7227-GCQG9" },
    Gvlk { edition_id: "ProfessionalWorkstation", label: "Windows 10/11 Pro for Workstations",    key: "NRG8B-VKK3Q-CXVCJ-9G2XF-6Q84J" },
    Gvlk { edition_id: "ProfessionalWorkstationN",label: "Windows 10/11 Pro N for Workstations",  key: "9FNHH-K3HBT-3W4TD-6383H-6XYWF" },
    Gvlk { edition_id: "Education",               label: "Windows 10/11 Education",               key: "NW6C2-QMPVW-D7KKK-3GKT6-VCFB2" },
    Gvlk { edition_id: "EducationN",              label: "Windows 10/11 Education N",             key: "2WH4N-8QGBV-H22JP-CT43Q-MDWWJ" },
    Gvlk { edition_id: "ProfessionalEducation",   label: "Windows 10/11 Pro Education",           key: "6TP4R-GNPTD-KYYHQ-7B7DP-J447Y" },
    Gvlk { edition_id: "ProfessionalEducationN",  label: "Windows 10/11 Pro Education N",         key: "YVWGF-BXNMC-HTQYQ-CPQ99-66QFC" },
    Gvlk { edition_id: "Enterprise",              label: "Windows 10/11 Enterprise",              key: "NPPR9-FWDCX-D2C8J-H872K-2YT43" },
    Gvlk { edition_id: "EnterpriseN",             label: "Windows 10/11 Enterprise N",            key: "DPH2V-TTNVB-4X9Q3-TJR4H-KHJW4" },
    Gvlk { edition_id: "EnterpriseG",             label: "Windows 10/11 Enterprise G",            key: "YYVX9-NTFWV-6MDM3-9PT4T-4M68B" },
    Gvlk { edition_id: "EnterpriseGN",            label: "Windows 10/11 Enterprise G N",          key: "44RPN-FTY23-9VTTB-MP9BX-T84FV" },
    // LTSC (build 2021/2019)
    Gvlk { edition_id: "EnterpriseS",             label: "Windows 10/11 Enterprise LTSC 2021",    key: "M7XTQ-FN8P6-TTKYV-9D4CC-J462D" },
    Gvlk { edition_id: "EnterpriseSN",            label: "Windows 10/11 Enterprise N LTSC 2021",  key: "92NFX-8DJQP-P6BBQ-THF9C-7CG2H" },
    // ── Windows Server (éditions volume courantes) ──
    Gvlk { edition_id: "ServerStandard",          label: "Windows Server 2022 Standard",          key: "VDYBN-27WPP-V4HQT-9VMD4-VMK7H" },
    Gvlk { edition_id: "ServerDatacenter",        label: "Windows Server 2022 Datacenter",        key: "WX4NM-KYWYW-QJJR4-XV3QB-6VM33" },
];

/// Édition Windows détectée sur la machine courante.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DetectedEdition {
    /// EditionID brut du registre (ex. "Professional").
    pub edition_id: String,
    /// ProductName lisible (ex. "Windows 11 Pro").
    pub product_name: String,
    /// GVLK trouvée pour cette édition, si connue.
    pub gvlk: Option<String>,
    /// Libellé de la GVLK trouvée.
    pub gvlk_label: Option<String>,
}

/// Cherche la GVLK correspondant à un EditionID (insensible à la casse).
pub fn gvlk_for_edition_id(edition_id: &str) -> Option<&'static Gvlk> {
    GVLK_TABLE.iter().find(|g| g.edition_id.eq_ignore_ascii_case(edition_id))
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

/// Détecte l'édition installée via le registre (EditionID + ProductName) et
/// résout la GVLK associée.
#[cfg(target_os = "windows")]
pub fn detect_edition() -> DetectedEdition {
    let edition_id = read_current_version("EditionID").unwrap_or_default();
    let product_name = read_current_version("ProductName").unwrap_or_default();
    let (gvlk, gvlk_label) = match gvlk_for_edition_id(&edition_id) {
        Some(g) => (Some(g.key.to_string()), Some(g.label.to_string())),
        None => (None, None),
    };
    DetectedEdition { edition_id, product_name, gvlk, gvlk_label }
}

#[cfg(not(target_os = "windows"))]
pub fn detect_edition() -> DetectedEdition {
    DetectedEdition {
        edition_id: String::new(),
        product_name: String::new(),
        gvlk: None,
        gvlk_label: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_non_vide_et_cles_bien_formees() {
        assert!(!GVLK_TABLE.is_empty());
        for g in GVLK_TABLE {
            // Format GVLK : 5 groupes de 5 caractères séparés par des tirets.
            let groups: Vec<&str> = g.key.split('-').collect();
            assert_eq!(groups.len(), 5, "clé {} mal formée", g.label);
            assert!(groups.iter().all(|p| p.len() == 5), "clé {} : groupe != 5", g.label);
        }
    }

    #[test]
    fn lookup_insensible_casse() {
        assert!(gvlk_for_edition_id("professional").is_some());
        assert!(gvlk_for_edition_id("Enterprise").is_some());
        assert!(gvlk_for_edition_id("InexistantXYZ").is_none());
    }
}
