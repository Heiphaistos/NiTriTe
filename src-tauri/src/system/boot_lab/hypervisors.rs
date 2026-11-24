//! Détection des hyperviseurs installés et appels de leurs CLI officiels (vmrun, VBoxManage).
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : vmworkbench.py (tools, checked, bridged_interfaces,
//! save_vmx, vmware_running, observe_vmware, vmware_memory_locked).

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{new_id, run, R, TIMEOUT};

/// Préfixe obligatoire des VM créées par NiTriTe : seules celles-ci peuvent être supprimées.
pub const TEST_PREFIX: &str = "NiTriTe-Test-";

pub const VIRTUALBOX_URL: &str = "https://www.virtualbox.org/wiki/Downloads";
pub const VMWARE_URL: &str = "https://www.vmware.com/products/desktop-hypervisor/workstation-and-fusion";
pub const HYPERV_URL: &str =
    "https://learn.microsoft.com/fr-fr/windows-server/virtualization/hyper-v/get-started/install-hyper-v";

#[derive(Debug, Clone, Default)]
pub struct Tools {
    pub virtualbox: Option<PathBuf>,
    pub vmrun: Option<PathBuf>,
    pub ovftool: Option<PathBuf>,
    pub diskmanager: Option<PathBuf>,
    pub seven_zip: Option<PathBuf>,
    pub hyperv: bool,
}

impl Tools {
    pub fn tool(&self, platform: &str) -> Option<&PathBuf> {
        match platform {
            "virtualbox" => self.virtualbox.as_ref(),
            "vmware" => self.vmrun.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Detected {
    pub virtualbox: bool,
    pub vmware: bool,
    /// Hyper-V est signalé mais pas piloté (TokaISO ne prend en charge que VirtualBox et VMware).
    pub hyperv: bool,
}

pub fn detected(t: &Tools) -> Detected {
    Detected { virtualbox: t.virtualbox.is_some(), vmware: t.vmrun.is_some(), hyperv: t.hyperv }
}

fn existing(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

/// Lecture seule : chemins d'installation standard.
pub fn tools() -> Tools {
    let roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .map(PathBuf::from)
        .collect();
    let mut vbox: Vec<PathBuf> = roots.iter().map(|r| r.join(r"Oracle\VirtualBox\VBoxManage.exe")).collect();
    if let Ok(msi) = std::env::var("VBOX_MSI_INSTALL_PATH") {
        if !msi.is_empty() {
            vbox.insert(0, PathBuf::from(msi).join("VBoxManage.exe"));
        }
    }
    let virtualbox = vbox.into_iter().find(|p| p.is_file());
    let vmrun = roots.iter().map(|r| r.join(r"VMware\VMware Workstation\vmrun.exe")).find(|p| p.is_file());
    let vmware_dir = vmrun.as_ref().and_then(|p| p.parent().map(Path::to_path_buf));
    let seven_zip = roots
        .iter()
        .map(|r| r.join(r"7-Zip\7z.exe"))
        .chain(vmware_dir.iter().map(|d| d.join("7za.exe")))
        .find(|p| p.is_file());
    Tools {
        virtualbox,
        ovftool: vmware_dir.as_ref().and_then(|d| existing(d.join(r"OVFTool\ovftool.exe"))),
        diskmanager: vmware_dir.as_ref().and_then(|d| existing(d.join("vmware-vdiskmanager.exe"))),
        vmrun,
        seven_zip,
        hyperv: super::system32().join("vmms.exe").is_file(),
    }
}

/// Appel d'un outil d'hyperviseur : erreur française si refus ou délai dépassé.
pub fn checked(exe: &Path, args: &[String], timeout: u64) -> R<String> {
    let out = run(exe, args, timeout).map_err(|e| {
        if e == TIMEOUT {
            "L’hyperviseur n’a pas terminé dans le délai prévu.".to_string()
        } else {
            format!("L’hyperviseur a refusé l’opération. {e}")
        }
    })?;
    if !out.success {
        let text = if out.stderr.trim().is_empty() { out.stdout.trim() } else { out.stderr.trim() };
        let tail: String = text.chars().rev().take(550).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(format!("L’hyperviseur a refusé l’opération. {tail}").trim_end().to_string());
    }
    Ok(out.stdout)
}

/// Raccourci : arguments en tableau de `&str`.
pub fn call(exe: &Path, args: &[&str], timeout: u64) -> R<String> {
    checked(exe, &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(), timeout)
}

pub fn parse_bridged(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|l| l.strip_prefix("Name:"))
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

pub fn bridged_interfaces(tool: Option<&PathBuf>) -> Vec<String> {
    tool.and_then(|t| call(t, &["list", "bridgedifs"], 4).ok()).map(|o| parse_bridged(&o)).unwrap_or_default()
}

/// Met à jour un .vmx en gardant les autres clés (écriture atomique).
pub fn save_vmx(path: &Path, values: &[(&str, String)]) -> R<()> {
    if values.iter().any(|(_, v)| v.contains(['"', '\n', '\r'])) {
        return Err("Valeur de configuration VMware invalide.".into());
    }
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let keys: Vec<String> = values.iter().map(|(k, _)| k.to_lowercase()).collect();
    let mut lines: Vec<String> = existing
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|l| !keys.contains(&l.split('=').next().unwrap_or("").trim().to_lowercase()))
        .map(String::from)
        .collect();
    lines.extend(values.iter().map(|(k, v)| format!("{k} = \"{v}\"")));
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let stage = path.with_file_name(format!("{name}.{}.tmp", new_id()));
    let result = std::fs::write(&stage, lines.join("\n") + "\n")
        .and_then(|_| std::fs::rename(&stage, path))
        .map_err(|e| format!("Écriture de la configuration VMware impossible : {e}"));
    let _ = std::fs::remove_file(&stage);
    result
}

fn norm(path: &str) -> String {
    path.trim().replace('/', "\\").to_lowercase()
}

pub fn vmware_running(tool: &Path, identity: &str, timeout: u64) -> R<bool> {
    let status = call(tool, &["-T", "ws", "list"], timeout)?;
    let target = norm(identity);
    Ok(status.lines().any(|l| norm(l) == target))
}

pub fn vmware_memory_locked(identity: &str) -> bool {
    Path::new(identity)
        .parent()
        .and_then(|d| std::fs::read_dir(d).ok())
        .map(|entries| entries.flatten().any(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".vmem.lck")))
        .unwrap_or(false)
}

/// Attend jusqu'à `wait` secondes que la VM soit allumée (ou éteinte et déverrouillée).
pub fn observe_vmware(tool: &Path, identity: &str, wait: f64, running: bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs_f64(wait);
    loop {
        if let Ok(on) = vmware_running(tool, identity, 2) {
            if on == running && (running || !vmware_memory_locked(identity)) {
                return true;
            }
        }
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        std::thread::sleep((deadline - now).min(Duration::from_millis(400)));
    }
}

/// Refuse toute suppression d'une VM qui n'a pas été créée par NiTriTe.
pub fn ensure_test_vm(name: &str) -> R<()> {
    let base = Path::new(name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if name.starts_with(TEST_PREFIX) || base.starts_with(TEST_PREFIX) {
        Ok(())
    } else {
        Err("Cette action est réservée aux VM d’essai créées par NiTriTe.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cartes_pont() {
        let out = "Name:            Intel(R) Ethernet\r\nGUID: x\r\n\r\nName:   Wi-Fi 6\r\nName:   \r\n";
        assert_eq!(parse_bridged(out), vec!["Intel(R) Ethernet", "Wi-Fi 6"]);
    }

    #[test]
    fn garde_des_vm() {
        assert!(ensure_test_vm("NiTriTe-Test-1a2b3c4d").is_ok());
        assert!(ensure_test_vm("Ma VM perso").is_err());
        assert!(ensure_test_vm("Windows 11").is_err());
    }

    #[test]
    fn vmx_garde_les_autres_cles() {
        let dir = std::env::temp_dir().join(format!("nitrite-vmx-{}", new_id()));
        std::fs::create_dir_all(&dir).expect("dossier");
        let path = dir.join("t.vmx");
        std::fs::write(&path, "\u{feff}memsize = \"512\"\nguestOS = \"x\"\n").expect("vmx");
        save_vmx(&path, &[("MemSize", "2048".into()), ("floppy0.present", "FALSE".into())]).expect("save");
        let text = std::fs::read_to_string(&path).expect("lecture");
        assert_eq!(text, "guestOS = \"x\"\nMemSize = \"2048\"\nfloppy0.present = \"FALSE\"\n");
        assert!(save_vmx(&path, &[("displayName", "a\"b".into())]).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
