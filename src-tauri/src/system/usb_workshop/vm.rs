//! Démarrage d'une copie isolée de la clé dans VMware Workstation ou VirtualBox
//! (la clé n'est que lue ; la VM démarre sur un disque virtuel local).
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : usb_boot.py (partie VM), vmworkbench.tools/save_vmx.

use super::disks::{self, UsbDisk};
use super::media::RAW_DISK_CSHARP;
use super::{ensure_dir, is_reparse, json_read, json_write, workshop_home, Res, GIB};
use crate::maintenance::commands::execute_system_command;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Emitter;

const MAX_SNAPSHOT: u64 = 128 * GIB;
const RAW_NAME: &str = "Cle USB-flat.vmdk";

static REGISTRY: Mutex<()> = Mutex::new(());
static CONTROLS: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn snapshot_script() -> String {
    format!(
        "{}\nAdd-Type -TypeDefinition @'\n{RAW_DISK_CSHARP}\n'@\n{}",
        r#"$destination = [IO.Path]::GetFullPath([string]$config.destination)
$directory = [IO.DirectoryInfo]::new([IO.Path]::GetDirectoryName($destination))
for($parent=$directory; $parent; $parent=$parent.Parent){
    if((-not $parent.Exists) -or ($parent.Attributes -band [IO.FileAttributes]::ReparsePoint)){
        exit 4
    }
}"#,
        r#"$device = Assert-TokaDisk
if($device.LogicalSectorSize -ne 512){ exit 5 }
try {
    $reader = [TokaISORawDisk]::new([int]$device.Number)
    try {
        $output = [IO.FileStream]::new($destination,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
        try {
            $reader.CopyTo($output,[long]$config.size,512)
            $output.Flush($true)
        } finally { $output.Dispose() }
    } finally { $reader.Dispose() }
} catch { exit 6 }
exit 0
"#
    )
}

/// Hyperviseurs installés (VBoxManage.exe, vmrun.exe) à leurs emplacements standard.
fn tools() -> (Option<PathBuf>, Option<PathBuf>) {
    let roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)"].iter().filter_map(std::env::var_os).map(PathBuf::from).collect();
    let mut vbox: Vec<PathBuf> = roots.iter().map(|r| r.join(r"Oracle\VirtualBox\VBoxManage.exe")).collect();
    if let Some(msi) = std::env::var_os("VBOX_MSI_INSTALL_PATH") {
        vbox.insert(0, PathBuf::from(msi).join("VBoxManage.exe"));
    }
    let vmrun = roots.iter().map(|r| r.join(r"VMware\VMware Workstation\vmrun.exe")).find(|p| p.is_file());
    (vbox.into_iter().find(|p| p.is_file()), vmrun)
}

fn tool_for(platform: &str) -> Res<PathBuf> {
    let (vbox, vmrun) = tools();
    match platform {
        "virtualbox" => vbox,
        "vmware" => vmrun,
        _ => None,
    }
    .ok_or_else(|| "Cet hyperviseur est introuvable sur ce PC.".to_string())
}

/// checked : commande d'hyperviseur avec délai ; un refus remonte la fin de sa sortie.
fn checked(tool: &Path, args: &[&str], timeout: u64) -> Res<String> {
    let out = execute_system_command(&tool.to_string_lossy(), args, timeout)
        .map_err(|_| "L’hyperviseur n’a pas terminé dans le délai prévu.".to_string())?;
    if !out.success {
        let detail = if out.stderr.trim().is_empty() { out.stdout.trim() } else { out.stderr.trim() };
        let tail: String = detail.chars().rev().take(550).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(format!("L’hyperviseur a refusé l’opération. {tail}").trim_end().to_string());
    }
    Ok(out.stdout)
}

/// save_vmx : fichier .vmx `clé = "valeur"` (écriture atomique).
fn save_vmx(path: &Path, values: &[(&str, String)]) -> Res<()> {
    let text: String = values.iter().map(|(k, v)| format!("{k} = \"{v}\"\n")).collect();
    let stage = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    std::fs::write(&stage, text).map_err(|e| e.to_string())?;
    std::fs::rename(&stage, path).map_err(|e| {
        super::remove_quiet(&stage);
        e.to_string()
    })
}

/// vmdk_descriptor : disque VMware « monolithicFlat » pointant sur la copie brute.
pub fn vmdk_descriptor(extent: &str, size: u64) -> Res<String> {
    if size == 0 || size % 512 != 0 || extent != RAW_NAME {
        return Err("La taille de l’image disque USB est invalide.".into());
    }
    let sectors = size / 512;
    let cid = &uuid::Uuid::new_v4().simple().to_string()[..8];
    Ok(format!(
        "# Disk DescriptorFile\nversion=1\nencoding=\"UTF-8\"\nCID={cid}\nparentCID=ffffffff\ncreateType=\"monolithicFlat\"\n\n\
         # Extent description\nRW {sectors} FLAT \"{extent}\" 0\n\n# The Disk Data Base\nddb.adapterType = \"lsilogic\"\n\
         ddb.geometry.cylinders = \"{}\"\nddb.geometry.heads = \"255\"\nddb.geometry.sectors = \"63\"\n",
        (sectors / (255 * 63)).max(1)
    ))
}

/// Espace libre du volume qui contient `path` (point de montage le plus long).
fn free_space(path: &Path) -> Option<u64> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|d| path.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.available_space())
}

fn root() -> Res<PathBuf> {
    ensure_dir(&workshop_home()?.join("usb-tests"))
}

fn registry_path() -> Res<PathBuf> {
    Ok(workshop_home()?.join("usb-tests.json"))
}

fn machines() -> Res<Map<String, Value>> {
    Ok(json_read(&registry_path()?, json!({})).as_object().cloned().unwrap_or_default())
}

fn save_machines(update: impl FnOnce(&mut Map<String, Value>)) -> Res<()> {
    let _lock = REGISTRY.lock().map_err(|_| "Registre des copies USB indisponible.".to_string())?;
    let mut all = machines()?;
    update(&mut all);
    json_write(&registry_path()?, &Value::Object(all))
}

pub fn list() -> Res<Value> {
    let (vbox, vmrun) = tools();
    let list: Vec<Value> = machines()?.into_values().collect();
    Ok(json!({"machines": list, "hypervisors": {"vmware": vmrun.is_some(), "virtualbox": vbox.is_some()}}))
}

/// Copie lecture seule de la clé ; la progression est émise sur « usb-workshop-boot-progress ».
fn snapshot_read(app: &tauri::AppHandle, device: &UsbDisk, destination: &Path) -> Res<()> {
    let running = Arc::new(AtomicBool::new(true));
    let watcher = {
        let (running, app, path, size) = (running.clone(), app.clone(), destination.to_path_buf(), device.size);
        std::thread::spawn(move || {
            while running.load(Ordering::Acquire) {
                let written = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0).min(size);
                if let Err(e) = app.emit("usb-workshop-boot-progress", json!({"phase": "read", "written": written, "size": size})) {
                    tracing::warn!("usb_workshop: progression non émise : {e}");
                }
                std::thread::sleep(std::time::Duration::from_millis(1400));
            }
        })
    };
    let mut config = device.identity();
    config["destination"] = json!(destination.to_string_lossy());
    let result = disks::elevated_usb(&config, &snapshot_script(), 10500);
    running.store(false, Ordering::Release);
    if watcher.join().is_err() {
        tracing::warn!("usb_workshop: suivi de progression interrompu");
    }
    let code = result?;
    if code == 5 {
        return Err("Le test en VM prend en charge les clés à secteurs logiques de 512 octets.".into());
    }
    if code != 0 || std::fs::metadata(destination).map(|m| m.len()).unwrap_or(0) != device.size {
        return Err("La lecture complète de la clé USB a échoué.".into());
    }
    Ok(())
}

/// UsbBoot.create : copie la clé puis prépare une VM sans réseau ni USB qui démarre dessus.
pub fn create(app: &tauri::AppHandle, payload: &Value) -> Res<Value> {
    let _guard = super::MediaGuard::acquire()?;
    let device = disks::pick_usb(payload["id"].as_str().unwrap_or(""))?;
    let platform = payload["platform"].as_str().unwrap_or("");
    let firmware = payload["firmware"].as_str().unwrap_or("");
    if !matches!(platform, "virtualbox" | "vmware") || !matches!(firmware, "bios" | "efi") {
        return Err("Choisis l’hyperviseur et le mode de démarrage.".into());
    }
    let tool = tool_for(platform)?;
    let size = device.size;
    if !(512..=MAX_SNAPSHOT).contains(&size) || size % 512 != 0 {
        return Err("La copie pour un test en VM est limitée aux clés de 128 Gio ou moins.".into());
    }
    if device.logical_sector != 0 && device.logical_sector != 512 {
        return Err("Le test en VM prend en charge les clés à secteurs logiques de 512 octets.".into());
    }
    let root = root()?;
    let reserve = if platform == "virtualbox" { size + GIB } else { GIB };
    if free_space(&root).is_some_and(|free| free < size + reserve) {
        return Err("Espace insuffisant pour préparer la machine virtuelle.".into());
    }
    let identifier = uuid::Uuid::new_v4().simple().to_string();
    let name = format!("TokaISO-USB-{}", &identifier[..8]);
    let folder = root.join(&name);
    std::fs::create_dir(&folder).map_err(|e| format!("Dossier de la copie : {e}"))?;
    let raw = folder.join(RAW_NAME);
    let mut registered = false;
    let outcome = (|| -> Res<Value> {
        snapshot_read(app, &device, &raw)?;
        let machine = if platform == "vmware" {
            let disk = folder.join("Cle USB.vmdk");
            std::fs::write(&disk, vmdk_descriptor(RAW_NAME, size)?).map_err(|e| e.to_string())?;
            let vmx = folder.join("Test USB.vmx");
            let values: Vec<(&str, String)> = [
                (".encoding", "UTF-8"), ("config.version", "8"), ("virtualHW.version", "16"), ("guestOS", "other-64"),
                ("displayName", name.as_str()), ("firmware", firmware), ("memsize", "2048"), ("numvcpus", "2"),
                ("bios.bootOrder", "hdd"), ("floppy0.present", "FALSE"), ("usb.present", "FALSE"),
                ("ethernet0.present", "FALSE"), ("sata0.present", "TRUE"), ("sata0:0.present", "TRUE"),
                ("sata0:0.fileName", "Cle USB.vmdk"), ("sharedFolder.maxNum", "0"), ("isolation.tools.hgfs.disable", "TRUE"),
            ]
            .iter()
            .map(|(k, v)| (*k, v.to_string()))
            .collect();
            save_vmx(&vmx, &values)?;
            vmx.to_string_lossy().into_owned()
        } else {
            if let Err(e) = app.emit("usb-workshop-boot-progress", json!({"phase": "convert", "written": size, "size": size})) {
                tracing::warn!("usb_workshop: progression non émise : {e}");
            }
            let disk = folder.join("Cle USB.vdi");
            let (raw_s, disk_s, folder_s) = (raw.to_string_lossy().to_string(), disk.to_string_lossy().to_string(), folder.to_string_lossy().to_string());
            checked(&tool, &["convertfromraw", &raw_s, &disk_s, "--format", "VDI"], 3600)?;
            std::fs::remove_file(&raw).map_err(|e| e.to_string())?;
            checked(&tool, &["createvm", "--name", &name, "--ostype", "Other_64", "--basefolder", &folder_s, "--register"], 30)?;
            registered = true;
            checked(&tool, &["modifyvm", &name, "--memory", "2048", "--cpus", "2", "--firmware", firmware, "--nic1", "none",
                "--usb-ohci", "off", "--usb-ehci", "off", "--usb-xhci", "off",
                "--boot1", "disk", "--boot2", "none", "--boot3", "none", "--boot4", "none"], 30)?;
            checked(&tool, &["storagectl", &name, "--name", "SATA", "--add", "sata", "--controller", "IntelAhci"], 30)?;
            checked(&tool, &["storageattach", &name, "--storagectl", "SATA", "--port", "0", "--device", "0", "--type", "hdd", "--medium", &disk_s], 30)?;
            name.clone()
        };
        let row = json!({"id": identifier, "name": name, "platform": platform, "folder": folder.to_string_lossy(),
            "machine": machine, "file": if device.name.is_empty() { "Clé USB".to_string() } else { device.name.clone() },
            "state": "imported", "usb_test": true, "size": size, "firmware": firmware, "date": chrono::Utc::now().to_rfc3339()});
        save_machines(|all| {
            all.insert(identifier.clone(), row.clone());
        })?;
        Ok(json!({"machine": row}))
    })();
    if outcome.is_err() {
        if registered && checked(&tool, &["unregistervm", &name], 25).is_err() {
            return Err("La VM incomplète est encore inscrite dans VirtualBox. Retire-la avant de réessayer.".into());
        }
        if folder.parent() == Some(root.as_path()) && !is_reparse(&folder) {
            if let Err(e) = std::fs::remove_dir_all(&folder) {
                tracing::warn!("usb_workshop: copie incomplète non supprimée : {e}");
            }
        }
    }
    outcome
}

fn vmware_running(tool: &Path, vmx: &str) -> bool {
    checked(tool, &["-T", "ws", "list"], 4).is_ok_and(|out| out.lines().any(|l| l.trim().eq_ignore_ascii_case(vmx)))
}

fn powered_off(info: &str) -> bool {
    info.lines().any(|l| l == "VMState=\"poweroff\"" || l == "VMState=\"aborted\"")
}

/// Contrôle d'une copie de test : `start` (ouvre la console), `stop`, `discard`.
pub fn control(action: &str, identifier: &str) -> Res<Value> {
    let row = machines()?.get(identifier).cloned().filter(|r| r["usb_test"] == true);
    let valid_id = identifier.len() == 32 && identifier.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
    let Some(row) = row.filter(|_| valid_id) else { return Err("Copie de test USB introuvable.".into()) };
    {
        let mut controls = CONTROLS.lock().map_err(|_| "Contrôle indisponible.".to_string())?;
        if !controls.get_or_insert_with(HashSet::new).insert(identifier.to_string()) {
            return Err("Une opération est déjà en cours pour cette VM.".into());
        }
    }
    let result = finish(action, &row);
    if let Ok(mut controls) = CONTROLS.lock() {
        controls.get_or_insert_with(HashSet::new).remove(identifier);
    }
    result
}

fn finish(action: &str, row: &Value) -> Res<Value> {
    let root = root()?;
    let id = row["id"].as_str().unwrap_or("");
    let name = format!("TokaISO-USB-{}", id.get(..8).unwrap_or(""));
    let folder = PathBuf::from(row["folder"].as_str().unwrap_or(""));
    if folder.parent() != Some(root.as_path()) || folder.file_name().map(|n| n.to_string_lossy() != name).unwrap_or(true)
        || (folder.exists() && is_reparse(&folder))
    {
        return Err("Le dossier de la copie USB n’est pas reconnu.".into());
    }
    let platform = row["platform"].as_str().unwrap_or("");
    let identity = row["machine"].as_str().unwrap_or("");
    let expected = if platform == "virtualbox" { name.clone() } else { folder.join("Test USB.vmx").to_string_lossy().into_owned() };
    if !matches!(platform, "virtualbox" | "vmware") || identity != expected {
        return Err("La VM de test USB n’est pas reconnue.".into());
    }
    let tool = tool_for(platform)?;
    match action {
        "start" => {
            if platform == "vmware" {
                checked(&tool, &["-T", "ws", "start", identity, "gui"], 90)?;
            } else {
                checked(&tool, &["startvm", identity, "--type", "gui"], 90)?;
            }
            Ok(json!({"started": true}))
        }
        "stop" => {
            if platform == "vmware" {
                if vmware_running(&tool, identity) {
                    checked(&tool, &["-T", "ws", "stop", identity, "hard"], 35)?;
                }
                let stopped = (0..6).any(|i| {
                    if i > 0 {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                    !vmware_running(&tool, identity)
                });
                if !stopped {
                    return Err("L’arrêt de la VM d’essai n’a pas encore été confirmé.".into());
                }
            } else {
                let state = checked(&tool, &["showvminfo", identity, "--machinereadable"], 30)?;
                if !powered_off(&state) {
                    checked(&tool, &["controlvm", identity, "poweroff"], 35)?;
                }
            }
            Ok(json!({"machine": row}))
        }
        "discard" => {
            if platform == "vmware" {
                if vmware_running(&tool, identity) || Path::new(&format!("{identity}.lck")).exists() {
                    return Err("Éteins la VM et ferme son onglet VMware avant de retirer la copie.".into());
                }
            } else {
                match checked(&tool, &["showvminfo", identity, "--machinereadable"], 30) {
                    Ok(state) => {
                        if !powered_off(&state) {
                            return Err("Arrête la VM avant de retirer le test.".into());
                        }
                        checked(&tool, &["unregistervm", identity], 25)?;
                    }
                    Err(e) => {
                        let listing = checked(&tool, &["list", "vms"], 30)?;
                        if listing.lines().any(|l| l.starts_with(&format!("\"{identity}\" "))) {
                            return Err(e);
                        }
                    }
                }
                let vdi = folder.join("Cle USB.vdi").to_string_lossy().into_owned();
                // Déjà fermé par unregistervm ou lors d'une tentative précédente : échec sans conséquence.
                if checked(&tool, &["closemedium", "disk", &vdi], 20).is_err() {
                    tracing::info!("usb_workshop: disque {vdi} déjà fermé");
                }
            }
            if folder.exists() {
                std::fs::remove_dir_all(&folder).map_err(|e| format!("Suppression de la copie : {e}"))?;
            }
            save_machines(|all| {
                all.remove(id);
            })?;
            Ok(json!({"removed": true}))
        }
        _ => Err("Commande de démarrage USB inconnue.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vmdk_descriptor_matches_size() {
        let text = vmdk_descriptor(RAW_NAME, 512 * 255 * 63 * 2).expect("descripteur");
        assert!(text.contains(&format!("RW {} FLAT \"{RAW_NAME}\" 0", 255 * 63 * 2)));
        assert!(text.contains("ddb.geometry.cylinders = \"2\""));
        assert!(vmdk_descriptor(RAW_NAME, 1000).is_err());
        assert!(vmdk_descriptor("autre.vmdk", 512).is_err());
    }

    #[test]
    fn poweroff_detection() {
        assert!(powered_off("name=\"x\"\nVMState=\"poweroff\"\n"));
        assert!(!powered_off("VMState=\"running\""));
    }

    #[test]
    fn snapshot_script_embeds_raw_reader_once() {
        let script = snapshot_script();
        assert_eq!(script.matches("class TokaISORawDisk").count(), 1);
        assert!(script.contains("exit 5"));
    }
}
