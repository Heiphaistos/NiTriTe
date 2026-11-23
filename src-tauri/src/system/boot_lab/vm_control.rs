//! Démarrage, observation, arrêt et archivage des VM préparées par NiTriTe.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : vmworkbench.py (action, control, start_vmware, open_vmware,
//! close_test, test_history, archive_test, confirm_test, eject_install_iso, status, save_state).

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::hypervisors::{call, ensure_test_vm, observe_vmware, save_vmx, tools, vmware_memory_locked, vmware_running, Tools};
use super::vm::{forget, save, store, tests_root, Machine};
use super::{file_input, home, json_read, json_write, now_iso, run, R};

#[derive(Debug, Serialize)]
pub struct ControlResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<Machine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed: Option<bool>,
    pub removed: bool,
}

fn done(machine: Machine) -> ControlResult {
    ControlResult { machine: Some(machine), launch: None, confirmed: None, removed: false }
}

fn history_path() -> PathBuf {
    home().join("iso-test-history.json")
}

/// Verrou par VM : une seule opération à la fois.
pub fn action(action: &str, id: &str) -> R<ControlResult> {
    let machine = {
        let mut s = store();
        let machine = s.machines.get(id).cloned().ok_or("Machine virtuelle introuvable.")?;
        if !s.controls.insert(id.to_string()) {
            return Err("Une opération est déjà en cours pour cette VM.".into());
        }
        machine
    };
    let result = control(action, machine, &tools());
    store().controls.remove(id);
    result
}

fn control(action: &str, machine: Machine, available: &Tools) -> R<ControlResult> {
    if action == "vm-test-confirm" {
        if !machine.test {
            return Err("Sélectionne une VM d’essai créée par NiTriTe.".into());
        }
        let result = Machine { menu_at: Some(now_iso()), ..machine };
        save(&result)?;
        return Ok(done(result));
    }
    let tool = available.tool(&machine.platform).cloned().ok_or("Cet hyperviseur est introuvable sur ce PC.")?;
    if action == "vm-eject-iso" {
        return eject_install_iso(machine, &tool);
    }
    let vmware = machine.platform == "vmware";
    if vmware && (action == "vm-start" || action == "vm-console") && !Path::new(&machine.machine).is_file() {
        return Err("Le fichier de configuration de cette VM VMware est introuvable.".into());
    }
    match action {
        "vm-console" if !vmware => {
            call(&tool, &["startvm", &machine.machine, "--type", "gui"], 35)?;
            Ok(done(status(machine, &tool, None)?))
        }
        "vm-console" => {
            let running = observe_vmware(&tool, &machine.machine, 0.0, true);
            open_vmware(machine, &tool, running, false)
        }
        "vm-stop-test" | "vm-discard-test" => close_test(action, machine, &tool),
        "vm-start" if vmware => start_vmware(machine, &tool),
        "vm-start" => {
            call(&tool, &["startvm", &machine.machine, "--type", "headless"], 70)?;
            Ok(done(status(machine, &tool, None)?))
        }
        "vm-status" => Ok(done(status(machine, &tool, None)?)),
        _ => Err("Commande de VM inconnue.".into()),
    }
}

fn start_vmware(machine: Machine, tool: &Path) -> R<ControlResult> {
    let identity = machine.machine.clone();
    if !observe_vmware(tool, &identity, 0.0, true) {
        disable_test_floppy(&machine)?;
        // Workstation peut signaler la VM allumée malgré un échec tardif de la commande.
        let _ = call(tool, &["-T", "ws", "start", &identity, "nogui"], 42);
        if !observe_vmware(tool, &identity, 4.0, true) {
            return open_vmware(machine, tool, false, true);
        }
    }
    Ok(done(status(machine, tool, Some(true))?))
}

fn open_vmware(machine: Machine, tool: &Path, running: bool, fallback: bool) -> R<ControlResult> {
    let gui = tool.with_file_name("vmware.exe");
    if !gui.is_file() {
        return Err("La console VMware Workstation est introuvable.".into());
    }
    let identity = machine.machine.clone();
    if !running {
        disable_test_floppy(&machine)?;
    }
    let mut command = std::process::Command::new(&gui);
    if !running {
        command.arg("-x");
    }
    command.arg(&identity).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    if let Some(dir) = gui.parent() {
        command.current_dir(dir);
    }
    // La console VMware est une application de l'utilisateur : elle reste ouverte après NiTriTe.
    let mut child = command.spawn().map_err(|_| "Impossible d’ouvrir la console VMware Workstation.".to_string())?;
    let confirmed = running || observe_vmware(tool, &identity, 10.0, true);
    if !confirmed {
        if let Ok(Some(code)) = child.try_wait() {
            if !code.success() {
                return Err("VMware Workstation a refusé d’ouvrir la console de cette VM.".into());
            }
        }
    }
    let machine = if confirmed {
        status(machine, tool, Some(true))?
    } else {
        let pending = Machine { console_pending: machine.console_pending || machine.test, ..machine };
        save_state(pending, "unknown")?
    };
    Ok(ControlResult {
        machine: Some(machine),
        launch: Some(if fallback { "console-fallback" } else { "console" }.into()),
        confirmed: Some(confirmed),
        removed: false,
    })
}

fn test_folder(machine: &Machine) -> PathBuf {
    tests_root().join(format!("{}{}", super::hypervisors::TEST_PREFIX, machine.id.get(..8).unwrap_or("")))
}

fn disable_test_floppy(machine: &Machine) -> R<()> {
    if !machine.test {
        return Ok(());
    }
    let identity = PathBuf::from(&machine.machine);
    let expected = test_folder(machine).join("Test ISO.vmx");
    let same = std::fs::canonicalize(&identity).ok() == std::fs::canonicalize(&expected).ok() && identity.is_file();
    if !same || PathBuf::from(format!("{}.lck", machine.machine)).exists() {
        return Ok(());
    }
    let config = std::fs::read_to_string(&identity).unwrap_or_default();
    let has = config.lines().any(|l| {
        let l = l.trim().to_lowercase();
        l.replace(' ', "") == "floppy0.present=\"false\""
    });
    if !has {
        save_vmx(&identity, &[("floppy0.present", "FALSE".into())])?;
    }
    Ok(())
}

fn close_test(action: &str, machine: Machine, tool: &Path) -> R<ControlResult> {
    let folder = std::fs::canonicalize(&machine.folder).unwrap_or_else(|_| PathBuf::from(&machine.folder));
    let expected = std::fs::canonicalize(test_folder(&machine)).unwrap_or_else(|_| test_folder(&machine));
    if !machine.test || folder != expected || ensure_test_vm(&machine.name).is_err() {
        return Err("Cette action est réservée aux VM d’essai créées par NiTriTe.".into());
    }
    let identity = machine.machine.clone();
    let vbox = machine.platform == "virtualbox";
    let current = status(machine, tool, None)?;
    if current.state == "unknown" {
        return Err("VMware ne confirme pas l’état de ce test. Vérifie sa console avant de réessayer.".into());
    }
    if current.state != "imported" {
        if action != "vm-stop-test" {
            return Err("Arrête la VM avant de retirer le test.".into());
        }
        if vbox {
            call(tool, &["controlvm", &identity, "poweroff"], 35)?;
            return Ok(done(status(current, tool, None)?));
        }
        let _ = call(tool, &["-T", "ws", "stop", &identity, "hard"], 22);
        if !observe_vmware(tool, &identity, 7.0, false) {
            return Err("L’arrêt de la VM d’essai n’a pas encore été confirmé.".into());
        }
        return Ok(done(save_state(current, "imported")?));
    }
    if action == "vm-stop-test" {
        return Ok(done(current));
    }
    if !vbox && PathBuf::from(format!("{identity}.lck")).exists() {
        return Err("Ferme l’onglet de la VM d’essai dans VMware avant de retirer le test.".into());
    }
    archive_test(&current)?;
    if vbox {
        ensure_test_vm(&identity)?;
        call(tool, &["unregistervm", &identity], 25)?;
    }
    if folder.is_dir() {
        std::fs::remove_dir_all(&folder).map_err(|e| format!("Suppression du dossier d’essai impossible : {e}"))?;
    }
    forget(&current.id)?;
    Ok(ControlResult { machine: None, launch: None, confirmed: None, removed: true })
}

fn summary(m: &Machine) -> Value {
    json!({
        "id": m.id, "name": m.name, "platform": m.platform, "file": m.file, "iso": m.iso, "date": m.date,
        "boot": m.boot, "firmware": m.firmware, "state": m.state, "powered_at": m.powered_at, "menu_at": m.menu_at,
    })
}

fn archive_test(machine: &Machine) -> R<()> {
    let mut archived: BTreeMap<String, Value> = json_read(&history_path());
    let mut row = summary(machine);
    row["removed_at"] = json!(now_iso());
    archived.insert(machine.id.clone(), row);
    json_write(&history_path(), &archived)
}

/// Essais ISO en cours et retirés, du plus récent au plus ancien.
pub fn test_history() -> Vec<Value> {
    let mut records: BTreeMap<String, Value> = json_read(&history_path());
    for vm in store().machines.values().filter(|m| m.test) {
        records.insert(vm.id.clone(), summary(vm));
    }
    let key = |v: &Value| {
        let date = v["date"].as_str().unwrap_or("");
        if date.is_empty() { v["removed_at"].as_str().unwrap_or("").to_string() } else { date.to_string() }
    };
    let mut rows: Vec<Value> = records.into_values().collect();
    rows.sort_by_key(|v| std::cmp::Reverse(key(v)));
    rows
}

fn vbox_powered_off(info: &str) -> bool {
    info.lines().any(|l| l.starts_with("VMState=\"poweroff\"") || l.starts_with("VMState=\"aborted\""))
}

fn eject_install_iso(machine: Machine, tool: &Path) -> R<ControlResult> {
    if !machine.install_iso {
        return Err("Sélectionne une VM installable créée depuis une ISO.".into());
    }
    if !machine.iso_attached {
        return Ok(done(machine));
    }
    let identity = machine.machine.clone();
    if machine.platform == "virtualbox" {
        let info = call(tool, &["showvminfo", &identity, "--machinereadable"], 30)?;
        if !vbox_powered_off(&info) {
            return Err("Éteins la VM avant de retirer le média d’installation.".into());
        }
        call(tool, &["storageattach", &identity, "--storagectl", "SATA", "--port", "1", "--device", "0", "--type", "dvddrive", "--medium", "emptydrive"], 30)?;
    } else {
        let path = file_input(&identity)?;
        if vmware_running(tool, &identity, 4)? || vmware_memory_locked(&identity) {
            return Err("Éteins la VM avant de retirer le média d’installation.".into());
        }
        if PathBuf::from(format!("{identity}.lck")).exists() {
            return Err("Ferme l’onglet de la VM dans VMware avant de retirer le média d’installation.".into());
        }
        let config = std::fs::read_to_string(&path).unwrap_or_default();
        let suspended = config.lines().any(|l| {
            let (k, v) = l.split_once('=').unwrap_or(("", ""));
            k.trim().eq_ignore_ascii_case("checkpoint.vmState") && !v.trim().trim_matches('"').is_empty()
        });
        if suspended {
            return Err("Éteins la VM avant de retirer le média d’installation.".into());
        }
        save_vmx(&path, &[
            ("sata0:1.present", "FALSE".into()), ("sata0:1.startConnected", "FALSE".into()),
            ("sata0:1.fileName", String::new()), ("bios.bootOrder", "hdd".into()),
        ])?;
    }
    let machine = Machine { iso_attached: false, ..machine };
    save(&machine)?;
    Ok(done(machine))
}

/// État observé : `imported` (éteinte), `running`, `guest` (système invité actif) ou `unknown`.
pub fn status(machine: Machine, tool: &Path, vmware_on: Option<bool>) -> R<Machine> {
    let identity = machine.machine.clone();
    let (running, guest, machine) = if machine.platform == "virtualbox" {
        let info = call(tool, &["showvminfo", &identity, "--machinereadable"], 30)?;
        let running = info.lines().any(|l| l.starts_with("VMState=\"running\""));
        let guest = running
            && call(tool, &["guestproperty", "get", &identity, "/VirtualBox/GuestInfo/OS/Product"], 4)
                .map(|o| o.contains("Value:"))
                .unwrap_or(false);
        (running, guest, machine)
    } else {
        let running = match vmware_on {
            Some(v) => v,
            None => vmware_running(tool, &identity, 4)?,
        };
        if !running && (vmware_memory_locked(&identity) || (machine.console_pending && Path::new(&identity).is_file())) {
            return save_state(machine, "unknown");
        }
        let machine = Machine { console_pending: false, ..machine };
        let args = ["-T", "ws", "checkToolsState", &identity].map(String::from);
        let guest = running
            && run(tool, &args, 3).map(|o| o.success && o.stdout.trim().eq_ignore_ascii_case("running")).unwrap_or(false);
        (running, guest, machine)
    };
    save_state(machine, if guest { "guest" } else if running { "running" } else { "imported" })
}

fn save_state(machine: Machine, state: &str) -> R<Machine> {
    let mut result = Machine { state: state.into(), ..machine };
    if result.test && (state == "running" || state == "guest") && result.powered_at.is_none() {
        result.powered_at = Some(now_iso());
    }
    save(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etat_virtualbox() {
        assert!(vbox_powered_off("name=\"x\"\nVMState=\"poweroff\"\n"));
        assert!(vbox_powered_off("VMState=\"aborted\"\r\n"));
        assert!(!vbox_powered_off("VMState=\"running\"\n"));
    }
}
