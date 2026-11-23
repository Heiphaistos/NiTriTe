//! Machines virtuelles préparées par NiTriTe : registre, import d'images, VM installable, essai ISO.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : vmworkbench.py (VmWorkbench.prepare, prepare_install_iso,
//! virtualbox, vmware, install_*_iso) et media_tools.py (iso_test).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use super::hypervisors::{bridged_interfaces, call, checked, ensure_test_vm, save_vmx, Tools, TEST_PREFIX};
use super::iso_inspect::inspect;
use super::vm_archive::{check_ova, image_in, is_archive, raw_suffixes, unpack};
use super::{check_space, file_input, home, is_reparse, json_read, json_write, new_id, now_iso, suffix, volume, R, MIB};

const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Machine {
    pub id: String,
    pub name: String,
    pub item: String,
    pub platform: String,
    pub folder: String,
    /// Nom VirtualBox ou chemin .vmx.
    pub machine: String,
    pub file: String,
    pub state: String,
    pub date: String,
    pub ram: u32,
    pub cpu: u32,
    pub firmware: String,
    pub network: String,
    pub adapter: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iso: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk: Option<u32>,
    pub install_iso: bool,
    pub iso_attached: bool,
    pub disk_exceeds_free: bool,
    pub test: bool,
    pub boot: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub powered_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_at: Option<String>,
    pub console_pending: bool,
}

#[derive(Default)]
pub struct Store {
    pub machines: BTreeMap<String, Machine>,
    pub busy: bool,
    pub controls: HashSet<String>,
}

pub fn store() -> MutexGuard<'static, Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE
        .get_or_init(|| Mutex::new(Store { machines: json_read(&machines_path()), ..Default::default() }))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

fn machines_path() -> PathBuf {
    home().join("machines.json")
}

pub fn tests_root() -> PathBuf {
    home().join("iso-tests")
}

pub fn machines_root() -> PathBuf {
    crate::utils::paths::downloads_dir().join("Machines NiTriTe")
}

/// Enregistre (ou remplace) une machine dans machines.json.
pub fn save(machine: &Machine) -> R<()> {
    let mut s = store();
    let mut updated = s.machines.clone();
    updated.insert(machine.id.clone(), machine.clone());
    json_write(&machines_path(), &updated)?;
    s.machines = updated;
    Ok(())
}

pub fn forget(id: &str) -> R<()> {
    let mut s = store();
    let mut updated = s.machines.clone();
    updated.remove(id);
    json_write(&machines_path(), &updated)?;
    s.machines = updated;
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct VmRequest {
    pub platform: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub name: String,
    pub ram: i64,
    pub cpu: i64,
    pub firmware: String,
    pub network: String,
    #[serde(default)]
    pub adapter: String,
    #[serde(default)]
    pub disk: i64,
}

struct Settings {
    ram: u32,
    cpu: u32,
    firmware: String,
    network: String,
    adapter: String,
}

fn settings(req: &VmRequest, tools: &Tools, firmwares: &[&str]) -> R<Settings> {
    if !(512..=65536).contains(&req.ram)
        || !(1..=16).contains(&req.cpu)
        || !firmwares.contains(&req.firmware.as_str())
        || !["nat", "off", "bridged"].contains(&req.network.as_str())
    {
        return Err("Vérifie la mémoire, les processeurs et le réseau de la VM.".into());
    }
    let mut adapter = String::new();
    if req.network == "bridged" && req.platform == "virtualbox" {
        if !bridged_interfaces(tools.virtualbox.as_ref()).contains(&req.adapter) {
            return Err("Choisis la carte réseau de VirtualBox pour le réseau en pont.".into());
        }
        adapter = req.adapter.clone();
    }
    Ok(Settings { ram: req.ram as u32, cpu: req.cpu as u32, firmware: req.firmware.clone(), network: req.network.clone(), adapter })
}

fn machine_name(raw: &str) -> String {
    let name: String = raw.trim().chars().take(56).collect();
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() { "NiTriTe VM".into() } else { name }
}

fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || c == '_' || " .-".contains(c) { c } else { '-' }).collect();
    let s = s.trim_matches([' ', '.', '-']).to_string();
    if s.is_empty() { "NiTriTe VM".into() } else { s }
}

/// Dossier neuf pour une VM et nom natif préfixé `NiTriTe-Test-`.
fn install_folder(name: &str) -> R<(String, String, PathBuf)> {
    let id = new_id();
    let slug = slug(name);
    let root = machines_root();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let folder = root.join(format!("{slug}-{}", &id[..8]));
    std::fs::create_dir(&folder).map_err(|e| e.to_string())?;
    let native = format!("{TEST_PREFIX}{}-{}", slug.replace(' ', "-"), &id[..8]);
    Ok((id, native, folder))
}

fn platform_ok(platform: &str, tools: &Tools) -> R<PathBuf> {
    tools.tool(platform).cloned().ok_or_else(|| "Cet hyperviseur est introuvable sur ce PC.".into())
}

fn s(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

fn nic(network: &str) -> String {
    if network == "off" { "none".into() } else { network.into() }
}

/// Import d'une image de VM locale (OVA, OVF, VDI, VMDK ou archive).
pub fn prepare(req: &VmRequest, tools: &Tools) -> R<Machine> {
    platform_ok(&req.platform, tools)?;
    let path = file_input(&req.file).map_err(|_| "Choisis une image VM locale.".to_string())?;
    let config = settings(req, tools, &["efi", "bios", "auto"])?;
    let name = machine_name(&req.name);
    let (id, native, folder) = install_folder(&name)?;
    let stage = folder.join(format!("Extraction-{}", &id[..8]));
    let result = (|| -> R<String> {
        let mut image = path.clone();
        if is_archive(&path) {
            std::fs::create_dir(&stage).map_err(|e| e.to_string())?;
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            unpack(&path, &stage, tools, size)?;
            image = image_in(&stage, &req.platform)?;
        }
        check_ova(&image, &folder)?;
        if req.platform == "virtualbox" {
            import_virtualbox(tools, &image, &folder, &native, &config)
        } else {
            import_vmware(tools, &image, &folder, &native, &config)
        }
    })();
    let _ = std::fs::remove_dir_all(&stage);
    let machine = result.map_err(|e| format!("Préparation de la VM interrompue. {e}"))?;
    let data = Machine {
        id,
        name,
        item: "local".into(),
        platform: req.platform.clone(),
        folder: s(&folder),
        machine,
        file: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        state: "imported".into(),
        date: now_iso(),
        ram: config.ram,
        cpu: config.cpu,
        firmware: config.firmware,
        network: config.network,
        adapter: config.adapter,
        ..Default::default()
    };
    save(&data)?;
    Ok(data)
}

fn import_virtualbox(tools: &Tools, image: &Path, folder: &Path, name: &str, c: &Settings) -> R<String> {
    let tool = platform_ok("virtualbox", tools)?;
    let ext = suffix(image);
    if ext == ".ova" || ext == ".ovf" {
        checked(&tool, &["import".into(), s(image), "--vsys".into(), "0".into(), "--vmname".into(), name.into(), "--basefolder".into(), s(folder)], 600)?;
    } else if raw_suffixes("virtualbox").contains(&ext.as_str()) {
        let disk = folder.join("NiTriTe.vdi");
        let size = std::fs::metadata(image).map(|m| m.len()).unwrap_or(0);
        check_space(folder, &[size], 0, "Espace insuffisant pour préparer la machine virtuelle.")?;
        checked(&tool, &["clonemedium".into(), "disk".into(), s(image), s(&disk), "--format".into(), "VDI".into()], 600)?;
        call(&tool, &["createvm", "--name", name, "--ostype", "Linux_64", "--basefolder", &s(folder), "--register"], 30)?;
        call(&tool, &["storagectl", name, "--name", "SATA", "--add", "sata", "--controller", "IntelAhci"], 30)?;
        call(&tool, &["storageattach", name, "--storagectl", "SATA", "--port", "0", "--device", "0", "--type", "hdd", "--medium", &s(&disk)], 30)?;
    } else {
        return Err("Choisis une image OVA, OVF, VDI ou VMDK pour VirtualBox.".into());
    }
    let mut options: Vec<String> = ["modifyvm", name, "--memory", &c.ram.to_string(), "--cpus", &c.cpu.to_string(), "--nic1", &nic(&c.network)]
        .map(String::from)
        .to_vec();
    if c.network == "bridged" {
        options.extend(["--bridgeadapter1".into(), c.adapter.clone()]);
    }
    for n in 2..=8 {
        options.extend([format!("--nic{n}"), "none".into()]);
    }
    if c.firmware != "auto" {
        options.extend(["--firmware".into(), c.firmware.clone()]);
    }
    checked(&tool, &options, 30)?;
    Ok(name.into())
}

fn vm_net(c: &Settings) -> Vec<(&'static str, String)> {
    vec![
        ("ethernet0.present", if c.network == "off" { "FALSE" } else { "TRUE" }.into()),
        ("ethernet0.connectionType", if c.network == "bridged" { "bridged" } else { "nat" }.into()),
    ]
}

fn import_vmware(tools: &Tools, image: &Path, folder: &Path, name: &str, c: &Settings) -> R<String> {
    let path = folder.join("NiTriTe.vmx");
    let ext = suffix(image);
    if (ext == ".ova" || ext == ".ovf") && tools.ovftool.is_some() {
        let ovf = tools.ovftool.as_ref().ok_or("OVF Tool introuvable.")?;
        checked(ovf, &["--lax".into(), format!("--name={name}"), s(image), s(&path)], 600)?;
    } else if ext == ".vmdk" && tools.diskmanager.is_some() {
        let manager = tools.diskmanager.as_ref().ok_or("Outil de disque VMware introuvable.")?;
        let size = std::fs::metadata(image).map(|m| m.len()).unwrap_or(0);
        check_space(folder, &[size], 0, "Espace insuffisant pour préparer la machine virtuelle.")?;
        let disk = folder.join("NiTriTe.vmdk");
        checked(manager, &["-r".into(), s(image), "-t".into(), "0".into(), s(&disk)], 600)?;
        save_vmx(&path, &[
            (".encoding", "UTF-8".into()), ("config.version", "8".into()), ("virtualHW.version", "19".into()),
            ("guestOS", "other5xlinux-64".into()), ("sata0.present", "TRUE".into()), ("sata0:0.present", "TRUE".into()),
            ("sata0:0.fileName", "NiTriTe.vmdk".into()),
        ])?;
    } else {
        return Err("Choisis une image OVA, OVF ou VMDK et vérifie les outils VMware.".into());
    }
    let mut values: Vec<(&str, String)> = vec![("displayName", name.into()), ("memsize", c.ram.to_string()), ("numvcpus", c.cpu.to_string())];
    values.extend(vm_net(c));
    values.extend([("ethernet0.startConnected", "TRUE".into()), ("sharedFolder.maxNum", "0".into()), ("isolation.tools.hgfs.disable", "TRUE".into())]);
    const EXTRA_NICS: [&str; 9] = ["ethernet1.present", "ethernet2.present", "ethernet3.present", "ethernet4.present", "ethernet5.present", "ethernet6.present", "ethernet7.present", "ethernet8.present", "ethernet9.present"];
    values.extend(EXTRA_NICS.iter().map(|k| (*k, "FALSE".to_string())));
    if c.firmware != "auto" {
        values.extend([("firmware", c.firmware.clone()), ("uefi.secureBoot.enabled", "FALSE".into())]);
    }
    save_vmx(&path, &values)?;
    Ok(s(&path))
}

/// VM vide avec disque virtuel neuf et ISO d'installation attachée.
pub fn prepare_install_iso(req: &VmRequest, tools: &Tools) -> R<Machine> {
    platform_ok(&req.platform, tools)?;
    let iso = file_input(&req.path)?;
    if suffix(&iso) != ".iso" || is_reparse(&iso) {
        return Err("Choisis un fichier ISO local pour installer la VM.".into());
    }
    let report = inspect(&iso)?;
    if report.format != "ISO 9660" && report.format != "UDF" {
        return Err("L’image ISO n’a pas été reconnue.".into());
    }
    let config = settings(req, tools, &["efi", "bios"])?;
    let firmware = if config.firmware == "efi" { "UEFI" } else { "BIOS" };
    if !report.boot.is_empty() && !report.boot.iter().any(|b| b == firmware) {
        return Err("Le mode de démarrage choisi est absent de cette ISO.".into());
    }
    if config.ram % 4 != 0 {
        return Err("Entre une quantité de mémoire vive divisible par 4 Mio.".into());
    }
    if !(4..=2048).contains(&req.disk) {
        return Err("Entre une capacité de disque entre 4 et 2 048 Gio.".into());
    }
    let disk = req.disk as u32;
    let controller = if req.platform == "virtualbox" { tools.virtualbox.clone() } else { tools.diskmanager.clone() };
    let controller = controller.ok_or("L’outil de création de disque VMware est introuvable.")?;
    let name = machine_name(&req.name);
    let (id, native, folder) = install_folder(&name)?;
    let mut registered = false;
    let result = (|| -> R<(String, u64)> {
        let (fs, free) = volume(&folder)?;
        if fs == "FAT32" {
            return Err("Choisis un dossier de téléchargement sur un volume sans limite FAT32 de 4 Gio.".into());
        }
        if free < (u64::from(config.ram) + 32) * MIB {
            return Err("Espace insuffisant pour préparer la machine virtuelle.".into());
        }
        if req.platform == "vmware" {
            return Ok((install_vmware_iso(&controller, &iso, &folder, &native, &config, disk)?, free));
        }
        call(&controller, &["createvm", "--name", &native, "--ostype", "Other_64", "--basefolder", &s(&folder), "--register"], 45)?;
        registered = true;
        install_virtualbox_iso(&controller, &iso, &folder, &native, &config, disk)?;
        Ok((native.clone(), free))
    })();
    let (machine, free) = match result {
        Ok(v) => v,
        Err(e) => {
            // Annulation : uniquement la VM que l'on vient de créer (préfixe NiTriTe-Test-).
            let vbox_file = folder.join(&native).join(format!("{native}.vbox"));
            let unregistered = if (registered || vbox_file.is_file()) && ensure_test_vm(&native).is_ok() {
                call(&controller, &["unregistervm", &native, "--delete"], 45).is_ok()
            } else {
                true
            };
            if unregistered {
                let _ = std::fs::remove_dir_all(&folder);
            }
            return Err(e);
        }
    };
    let data = Machine {
        id,
        name,
        item: "local".into(),
        platform: req.platform.clone(),
        folder: s(&folder),
        machine,
        file: iso.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        iso: Some(s(&iso)),
        state: "imported".into(),
        date: now_iso(),
        disk: Some(disk),
        install_iso: true,
        iso_attached: true,
        disk_exceeds_free: u64::from(disk) * GIB > free,
        ram: config.ram,
        cpu: config.cpu,
        firmware: config.firmware,
        network: config.network,
        adapter: config.adapter,
        ..Default::default()
    };
    save(&data)?;
    Ok(data)
}

fn install_virtualbox_iso(tool: &Path, iso: &Path, folder: &Path, name: &str, c: &Settings, disk: u32) -> R<()> {
    let mut modify: Vec<String> = [
        "modifyvm", name, "--memory", &c.ram.to_string(), "--cpus", &c.cpu.to_string(), "--ioapic", "on", "--firmware",
        &c.firmware, "--boot1", "dvd", "--boot2", "disk", "--boot3", "none", "--boot4", "none", "--nic1", &nic(&c.network),
    ]
    .map(String::from)
    .to_vec();
    if c.network == "bridged" {
        modify.extend(["--bridgeadapter1".into(), c.adapter.clone()]);
    }
    checked(tool, &modify, 45)?;
    let vdi = s(&folder.join("NiTriTe.vdi"));
    call(tool, &["createmedium", "disk", "--filename", &vdi, "--size", &(u64::from(disk) * 1024).to_string(), "--format", "VDI", "--variant", "Standard"], 120)?;
    call(tool, &["storagectl", name, "--name", "SATA", "--add", "sata", "--controller", "IntelAhci", "--portcount", "2", "--bootable", "on"], 30)?;
    call(tool, &["storageattach", name, "--storagectl", "SATA", "--port", "0", "--device", "0", "--type", "hdd", "--medium", &vdi], 30)?;
    call(tool, &["storageattach", name, "--storagectl", "SATA", "--port", "1", "--device", "0", "--type", "dvddrive", "--medium", &s(iso)], 30)?;
    Ok(())
}

fn install_vmware_iso(manager: &Path, iso: &Path, folder: &Path, name: &str, c: &Settings, disk: u32) -> R<String> {
    let vmdk = folder.join("NiTriTe.vmdk");
    call(manager, &["-c", "-s", &format!("{disk}GB"), "-a", "lsilogic", "-t", "0", &s(&vmdk)], 120)?;
    let path = folder.join("NiTriTe.vmx");
    let mut values: Vec<(&str, String)> = vec![
        (".encoding", "UTF-8".into()), ("config.version", "8".into()), ("virtualHW.version", "16".into()),
        ("guestOS", "other-64".into()), ("displayName", name.into()), ("firmware", c.firmware.clone()),
        ("memsize", c.ram.to_string()), ("numvcpus", c.cpu.to_string()), ("bios.bootOrder", "cdrom,hdd".into()),
        ("floppy0.present", "FALSE".into()), ("sata0.present", "TRUE".into()), ("sata0:0.present", "TRUE".into()),
        ("sata0:0.fileName", "NiTriTe.vmdk".into()), ("sata0:1.present", "TRUE".into()),
        ("sata0:1.startConnected", "TRUE".into()), ("sata0:1.deviceType", "cdrom-image".into()), ("sata0:1.fileName", s(iso)),
    ];
    values.extend(vm_net(c));
    values.extend([
        ("ethernet0.virtualDev", "e1000e".into()), ("ethernet0.startConnected", "TRUE".into()),
        ("sharedFolder.maxNum", "0".into()), ("isolation.tools.hgfs.disable", "TRUE".into()),
    ]);
    save_vmx(&path, &values)?;
    Ok(s(&path))
}

/// Essai isolé d'une ISO : VM `NiTriTe-Test-xxxxxxxx`, 2 Gio, 2 vCPU, démarrage DVD, réseau NAT ou coupé.
pub fn iso_test(path: &str, platform: &str, firmware: &str, network: bool) -> R<Machine> {
    let path = file_input(path)?;
    if suffix(&path) != ".iso" || is_reparse(&path) {
        return Err("Le fichier choisi n’est pas un média pris en charge.".into());
    }
    let report = inspect(&path)?;
    if report.format != "ISO 9660" && report.format != "UDF" {
        return Err("L’image ISO n’a pas été reconnue.".into());
    }
    if !["virtualbox", "vmware"].contains(&platform) || !["bios", "efi"].contains(&firmware) {
        return Err("Choisis l’hyperviseur et le mode de démarrage.".into());
    }
    let tools = super::hypervisors::tools();
    let handle = platform_ok(platform, &tools)?;
    let id = new_id();
    let name = format!("{TEST_PREFIX}{}", &id[..8]);
    let folder = tests_root().join(&name);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let iso = s(&path);
    let machine = if platform == "virtualbox" {
        call(&handle, &["createvm", "--name", &name, "--ostype", "Other_64", "--basefolder", &s(&folder), "--register"], 30)?;
        call(&handle, &["modifyvm", &name, "--memory", "2048", "--cpus", "2", "--firmware", firmware, "--nic1", if network { "nat" } else { "none" },
            "--boot1", "dvd", "--boot2", "none", "--boot3", "none", "--boot4", "none"], 30)?;
        call(&handle, &["storagectl", &name, "--name", "SATA", "--add", "sata", "--controller", "IntelAhci"], 30)?;
        call(&handle, &["storageattach", &name, "--storagectl", "SATA", "--port", "0", "--device", "0", "--type", "dvddrive", "--medium", &iso], 30)?;
        name.clone()
    } else {
        let target = folder.join("Test ISO.vmx");
        save_vmx(&target, &[
            (".encoding", "UTF-8".into()), ("config.version", "8".into()), ("virtualHW.version", "16".into()),
            ("guestOS", "other-64".into()), ("displayName", name.clone()), ("firmware", firmware.into()),
            ("memsize", "2048".into()), ("numvcpus", "2".into()), ("bios.bootOrder", "cdrom".into()),
            ("floppy0.present", "FALSE".into()), ("ide1:0.present", "TRUE".into()), ("ide1:0.startConnected", "TRUE".into()),
            ("ide1:0.deviceType", "cdrom-image".into()), ("ide1:0.fileName", iso.clone()),
            ("ethernet0.present", if network { "TRUE" } else { "FALSE" }.into()), ("ethernet0.connectionType", "nat".into()),
            ("ethernet0.virtualDev", "e1000e".into()), ("ethernet0.startConnected", "TRUE".into()),
            ("sharedFolder.maxNum", "0".into()), ("isolation.tools.hgfs.disable", "TRUE".into()),
        ])?;
        s(&target)
    };
    let vm = Machine {
        id,
        name,
        item: "local".into(),
        platform: platform.into(),
        folder: s(&folder),
        machine,
        file: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        state: "imported".into(),
        test: true,
        boot: report.boot,
        iso: Some(iso),
        firmware: firmware.into(),
        network: if network { "nat" } else { "off" }.into(),
        date: now_iso(),
        ..Default::default()
    };
    save(&vm)?;
    Ok(vm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms() {
        assert_eq!(machine_name("  Mon   labo  "), "Mon labo");
        assert_eq!(machine_name(""), "NiTriTe VM");
        assert_eq!(machine_name(&"x".repeat(80)).len(), 56);
        assert_eq!(slug("Home/Assistant: v2."), "Home-Assistant- v2");
        assert_eq!(slug("..."), "NiTriTe VM");
    }

    fn req(ram: i64, cpu: i64, firmware: &str, network: &str) -> VmRequest {
        VmRequest {
            platform: "vmware".into(), file: String::new(), path: String::new(), name: String::new(),
            ram, cpu, firmware: firmware.into(), network: network.into(), adapter: String::new(), disk: 0,
        }
    }

    #[test]
    fn reglages() {
        let t = Tools::default();
        assert!(settings(&req(2048, 2, "efi", "nat"), &t, &["efi", "bios", "auto"]).is_ok());
        assert!(settings(&req(256, 2, "efi", "nat"), &t, &["efi"]).is_err());
        assert!(settings(&req(2048, 17, "efi", "nat"), &t, &["efi"]).is_err());
        assert!(settings(&req(2048, 2, "auto", "nat"), &t, &["efi", "bios"]).is_err());
        assert!(settings(&req(2048, 2, "efi", "host"), &t, &["efi"]).is_err());
    }
}
