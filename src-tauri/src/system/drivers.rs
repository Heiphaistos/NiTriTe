use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::LazyLock;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::error::NiTriTeError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedDriver {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub url: String,
    pub check_registry: Option<String>, // Cle registre pour verifier si installe
    pub check_name: Option<String>,     // Nom a chercher dans les programmes installes
}

#[derive(Debug, Clone, Serialize)]
pub struct DriverStatusOutput {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DriverStatus {
    pub driver: DriverStatusOutput,
    pub installed: bool,
}

static RECOMMENDED_DRIVERS: LazyLock<Vec<RecommendedDriver>> = LazyLock::new(|| {
    let json = include_str!("../../data/drivers_recommended.json");
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::error!("Erreur chargement drivers_recommended.json: {}", e);
        Vec::new()
    })
});

/// Retourne la liste des drivers recommandes avec leur statut d'installation
pub fn get_recommended_drivers() -> Result<Vec<DriverStatus>, NiTriTeError> {
    // Recuperer la liste des programmes installes via winget (cache)
    let installed_apps = get_installed_apps_list();

    let results: Vec<DriverStatus> = RECOMMENDED_DRIVERS
        .iter()
        .map(|driver| {
            let installed = check_driver_installed(driver, &installed_apps);
            DriverStatus {
                driver: DriverStatusOutput {
                    id: driver.id.clone(),
                    name: driver.name.clone(),
                    description: driver.description.clone(),
                    category: driver.category.clone(),
                    url: driver.url.clone(),
                },
                installed,
            }
        })
        .collect();

    Ok(results)
}

fn check_driver_installed(driver: &RecommendedDriver, installed_apps: &str) -> bool {
    // Verifier via le registre si une cle NON VIDE est specifiee
    if let Some(ref reg_key) = driver.check_registry {
        if !reg_key.trim().is_empty() && check_registry_key(reg_key) {
            return true;
        }
    }

    // Verifier via le nom dans la liste des programmes installes.
    // Garde contre un check_name vide : `contains("")` renverrait toujours true
    // et marquerait le driver « installé » à tort.
    if let Some(ref name) = driver.check_name {
        let name_lower = name.trim().to_lowercase();
        if !name_lower.is_empty() && installed_apps.to_lowercase().contains(&name_lower) {
            return true;
        }
    }

    false
}

fn check_registry_key(key_path: &str) -> bool {
    // Verifier l'existence d'une cle registre via reg query
    let output = Command::new("reg")
        .args(["query", key_path])
        .creation_flags(0x08000000)
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

fn get_installed_apps_list() -> String {
    // Lecture directe du registre — plus rapide et fiable que wmic (déprécié W11)
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;
        let uninstall_paths = [
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
            (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        ];
        let mut names = Vec::new();
        for (hive, path) in &uninstall_paths {
            let Ok(hive_key) = RegKey::predef(*hive).open_subkey(path) else { continue };
            for subkey_name in hive_key.enum_keys().flatten() {
                let Ok(subkey) = hive_key.open_subkey(&subkey_name) else { continue };
                if let Ok(name) = subkey.get_value::<String, _>("DisplayName") {
                    names.push(name);
                }
            }
        }
        return names.join("\n");
    }
    #[allow(unreachable_code)]
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_driver(check_name: Option<&str>, check_registry: Option<&str>) -> RecommendedDriver {
        RecommendedDriver {
            id: "test".into(),
            name: "Test Driver".into(),
            description: "Test".into(),
            category: "Test".into(),
            url: "https://example.com".into(),
            check_registry: check_registry.map(|s| s.to_string()),
            check_name: check_name.map(|s| s.to_string()),
        }
    }

    #[test]
    fn check_name_found_in_installed_list() {
        let driver = make_driver(Some("NVIDIA Graphics Driver"), None);
        let installed = "NVIDIA GeForce Experience\nNVIDIA Graphics Driver\nMicrosoft Edge";
        assert!(check_driver_installed(&driver, installed));
    }

    #[test]
    fn check_name_case_insensitive() {
        let driver = make_driver(Some("nvidia graphics driver"), None);
        let installed = "NVIDIA Graphics Driver";
        assert!(check_driver_installed(&driver, installed));
    }

    #[test]
    fn check_name_not_found_returns_false() {
        let driver = make_driver(Some("AMD Radeon Driver"), None);
        let installed = "NVIDIA Graphics Driver\nIntel HD Graphics";
        assert!(!check_driver_installed(&driver, installed));
    }

    #[test]
    fn no_check_name_no_check_registry_returns_false() {
        let driver = make_driver(None, None);
        assert!(!check_driver_installed(&driver, "anything installed"));
    }

    #[test]
    fn check_name_empty_installed_list_returns_false() {
        let driver = make_driver(Some("Some Driver"), None);
        assert!(!check_driver_installed(&driver, ""));
    }

    /// Les check_name du catalogue doivent correspondre aux DisplayName RÉELS
    /// (relevés sur Windows 11 le 2026-11-23). « .NET Runtime 8 » ne
    /// correspondait jamais à « Microsoft .NET Runtime - 8.0.31 (x64) ».
    #[test]
    fn catalog_check_names_match_real_display_names() {
        let installed = "Microsoft .NET Runtime - 8.0.31 (x64)\nMicrosoft .NET Runtime - 10.0.12 (x64)\nMicrosoft Visual C++ v14 Redistributable (x64) - 14.51.36247";
        let find = |id: &str| RECOMMENDED_DRIVERS.iter().find(|d| d.id == id).cloned().expect("entrée du catalogue");
        for id in ["dotnet-8", "dotnet-10", "vcredist-2015-2022"] {
            let mut d = find(id);
            d.check_registry = None;
            assert!(check_driver_installed(&d, installed), "{id} non détecté");
        }
        let mut d9 = find("dotnet-9");
        d9.check_registry = None;
        assert!(!check_driver_installed(&d9, installed), "dotnet-9 ne doit pas matcher 8.x ni 10.x");
    }

    #[test]
    fn empty_check_name_does_not_false_positive() {
        // check_name vide ne doit PAS marquer le driver installé (contains("")=true)
        let driver = make_driver(Some(""), None);
        assert!(!check_driver_installed(&driver, "NVIDIA Graphics Driver\nIntel HD"));
        let driver_ws = make_driver(Some("   "), None);
        assert!(!check_driver_installed(&driver_ws, "anything"));
    }
}
