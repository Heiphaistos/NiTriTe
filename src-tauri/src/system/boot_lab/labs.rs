//! Modèles de labo (trousses VM et mini-serveurs), plans réseau enregistrés et profil du PC.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : labs.json, workshop.py (lab_options, machine_profile),
//! deployment.py (LABS, lab_action) et diagnostics.py (Plans).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use super::diagnostics::{host, port};
use super::{home, json_read, json_write, new_id, now_iso, R};

const LABS_JSON: &str = include_str!("labs.json");
pub const PLATFORMS: [(&str, &str); 2] = [("virtualbox", "VirtualBox"), ("vmware", "VMware Workstation")];
const ROLES: [&str; 5] = ["home", "storage", "web", "network", "other"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmLab {
    pub name: String,
    pub description: String,
    pub items: Vec<String>,
    pub steps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// Trousses VM (labs.json), dans l'ordre du fichier.
pub fn vm_labs() -> BTreeMap<String, VmLab> {
    serde_json::from_str(LABS_JSON).unwrap_or_default()
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerLab {
    pub key: &'static str,
    pub name: &'static str,
    pub role: &'static str,
    pub ports: &'static [u16],
    pub items: &'static [&'static str],
    pub ram: u32,
    pub cpu: u32,
    pub disk: u32,
    pub steps: &'static [&'static str],
}

pub const SERVER_LABS: [ServerLab; 4] = [
    ServerLab {
        key: "backup", name: "Serveur de sauvegarde", role: "storage", ports: &[8007], items: &["proxmox-backup"],
        ram: 4, cpu: 2, disk: 80,
        steps: &["Préparer un disque virtuel pour le système et un second pour les sauvegardes.",
                 "Créer le stockage de sauvegarde et limiter l’accès aux comptes nécessaires.",
                 "Sauvegarder un fichier de test, puis vérifier sa restauration."],
    },
    ServerLab {
        key: "files", name: "Partage de fichiers", role: "storage", ports: &[445, 80], items: &["openmediavault"],
        ram: 4, cpu: 2, disk: 40,
        steps: &["Préparer un disque de données séparé.",
                 "Créer un partage et attribuer des droits à un compte de test.",
                 "Tester la lecture et l’écriture depuis un deuxième appareil."],
    },
    ServerLab {
        key: "home", name: "Domotique Home Assistant", role: "home", ports: &[8123], items: &["vm-home-assistant"],
        ram: 4, cpu: 2, disk: 32,
        steps: &["Sélectionner l’image Home Assistant adaptée à ton hyperviseur.",
                 "Ouvrir l’interface locale et créer le premier compte.",
                 "Connecter un appareil et exporter une sauvegarde de la configuration."],
    },
    ServerLab {
        key: "web", name: "Labo web local", role: "web", ports: &[22, 80, 443], items: &["21", "94"],
        ram: 4, cpu: 2, disk: 32,
        steps: &["Préparer Ubuntu Server en VM ou installer XAMPP sur un PC Windows de test.",
                 "Servir une page de test et vérifier les ports utilisés.",
                 "Tester l’accès depuis un appareil du réseau local."],
    },
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabOption {
    pub id: String,
    pub platform: String,
}

pub fn lab_option(value: Option<&LabOption>) -> Option<LabOption> {
    value.filter(|v| vm_labs().contains_key(&v.id) && PLATFORMS.iter().any(|(k, _)| *k == v.platform)).cloned()
}

#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    pub arch: String,
    pub firmware: String,
    pub winget: bool,
}

/// Architecture, micrologiciel (GetFirmwareType) et présence de WinGet.
pub fn machine_profile() -> Profile {
    let arch = std::env::var("PROCESSOR_ARCHITEW6432")
        .or_else(|_| std::env::var("PROCESSOR_ARCHITECTURE"))
        .unwrap_or_default()
        .to_uppercase();
    let arch = match arch.as_str() {
        "AMD64" | "X86_64" => "x64",
        "ARM64" | "AARCH64" => "ARM64",
        "X86" => "x86",
        _ => "Inconnu",
    };
    let mut firmware = "Inconnu";
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::{GetFirmwareType, FIRMWARE_TYPE};
        let mut kind = FIRMWARE_TYPE::default();
        // SAFETY: GetFirmwareType écrit seulement dans `kind`, qui vit pendant l'appel.
        if unsafe { GetFirmwareType(&mut kind) }.is_ok() {
            firmware = match kind.0 {
                1 => "BIOS",
                2 => "UEFI",
                _ => "Inconnu",
            };
        }
    }
    let winget = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join("winget.exe").is_file()))
        .unwrap_or(false)
        || std::env::var_os("LOCALAPPDATA")
            .map(|d| std::path::Path::new(&d).join(r"Microsoft\WindowsApps\winget.exe").exists())
            .unwrap_or(false);
    Profile { arch: arch.into(), firmware: firmware.into(), winget }
}

// ─── Plans de mini-serveurs (projects.json) ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlanMachine {
    pub name: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub item: String,
    #[serde(default)]
    pub ports: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub name: String,
    pub machines: Vec<PlanMachine>,
    pub updated: String,
}

fn plans_lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
}

fn plans_path() -> std::path::PathBuf {
    home().join("projects.json")
}

pub fn plans() -> Vec<Plan> {
    let _guard = plans_lock().lock().unwrap_or_else(|p| p.into_inner());
    json_read::<BTreeMap<String, Plan>>(&plans_path()).into_values().collect()
}

fn valid_key(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'a'..=b'f' | b'0'..=b'9'))
}

fn clean(value: &str, limit: usize) -> String {
    value.trim().chars().take(limit).collect::<String>().trim().to_string()
}

/// Ajoute ou remplace un plan (80 plans, 16 machines, 8 ports par machine au maximum).
pub fn plan_save(id: Option<&str>, name: &str, machines: &[PlanMachine]) -> R<(Vec<Plan>, String)> {
    let _guard = plans_lock().lock().unwrap_or_else(|p| p.into_inner());
    let key = id.filter(|k| !k.is_empty()).map(String::from).unwrap_or_else(new_id);
    if !valid_key(&key) {
        return Err("Projet introuvable.".into());
    }
    let mut all: BTreeMap<String, Plan> = json_read(&plans_path());
    let name = clean(name, 70);
    if name.is_empty() || (all.len() >= 80 && !all.contains_key(&key)) {
        return Err("Donne un nom au projet. La limite est de 80 projets.".into());
    }
    if machines.len() > 16 {
        return Err("Un projet peut contenir au plus 16 machines.".into());
    }
    let mut rows = Vec::new();
    for row in machines {
        let label = clean(&row.name, 70);
        let address = if row.host.trim().is_empty() { String::new() } else { host(&row.host)? };
        let role = if row.role.is_empty() { "other".to_string() } else { row.role.clone() };
        let item_ok = row.item.len() <= 64 && row.item.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        if label.is_empty() || !ROLES.contains(&role.as_str()) || !item_ok {
            return Err("Vérifie le nom et le rôle de la machine.".into());
        }
        if row.ports.len() > 8 {
            return Err("Une machine peut contenir au plus huit ports TCP.".into());
        }
        let mut ports: Vec<i64> = Vec::new();
        for value in &row.ports {
            let p = i64::from(port(*value)?);
            if !ports.contains(&p) {
                ports.push(p);
            }
        }
        rows.push(PlanMachine { name: label, host: address, role, item: row.item.clone(), ports });
    }
    all.insert(key.clone(), Plan { id: key.clone(), name, machines: rows, updated: now_iso() });
    json_write(&plans_path(), &all)?;
    Ok((all.into_values().collect(), key))
}

pub fn plan_delete(id: &str) -> R<Vec<Plan>> {
    if !valid_key(id) {
        return Err("Projet introuvable.".into());
    }
    let _guard = plans_lock().lock().unwrap_or_else(|p| p.into_inner());
    let mut all: BTreeMap<String, Plan> = json_read(&plans_path());
    all.remove(id);
    json_write(&plans_path(), &all)?;
    Ok(all.into_values().collect())
}

/// « Créer le projet réseau » depuis un modèle de mini-serveur.
pub fn lab_plan(key: &str) -> R<(Vec<Plan>, String)> {
    let lab = SERVER_LABS.iter().find(|l| l.key == key).ok_or("Choisis un modèle de labo.")?;
    let machine = PlanMachine {
        name: lab.name.into(),
        host: String::new(),
        role: lab.role.into(),
        item: lab.items.first().map(|s| s.to_string()).unwrap_or_default(),
        ports: lab.ports.iter().map(|p| i64::from(*p)).collect(),
    };
    plan_save(None, lab.name, &[machine])
}

pub fn labs_payload() -> Value {
    serde_json::json!({
        "vm": vm_labs(),
        "servers": SERVER_LABS,
        "platforms": PLATFORMS.iter().map(|(k, v)| serde_json::json!({"key": k, "name": v})).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labs_json_charge() {
        let labs = vm_labs();
        assert_eq!(labs.len(), 3);
        assert!(labs["web-lab"].reference.as_deref().is_some_and(|r| r.starts_with("https://")));
        assert!(lab_option(Some(&LabOption { id: "smart-home".into(), platform: "vmware".into() })).is_some());
        assert!(lab_option(Some(&LabOption { id: "smart-home".into(), platform: "hyperv".into() })).is_none());
        assert!(lab_option(Some(&LabOption { id: "inconnu".into(), platform: "vmware".into() })).is_none());
    }

    #[test]
    fn cles_de_plan() {
        assert!(valid_key(&new_id()));
        assert!(!valid_key("../../etc"));
        assert!(!valid_key(&"G".repeat(32)));
    }
}
