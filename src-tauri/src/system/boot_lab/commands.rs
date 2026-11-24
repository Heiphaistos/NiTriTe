//! Commandes Tauri du labo PXE + VM (toutes asynchrones, corps bloquant hors du thread UI).
//! Port de TokaISO 7.9 (MIT, Cédrick Roy).

use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};

use super::diagnostics::{self, Estimate, NetworkRequest};
use super::hypervisors::{self, bridged_interfaces, detected, Detected, HYPERV_URL, VIRTUALBOX_URL, VMWARE_URL};
use super::kit::{self, KitRequest, KitResult};
use super::labs::{self, PlanMachine, Profile};
use super::pxe::{self, Diagnose, ExportRequest, Interface, Project, PxeStatus};
use super::vm::{self, Machine, VmRequest};
use super::vm_control::{self, ControlResult};
use super::{blocking, R};

/// Arrête le serveur PXE quand la fenêtre principale est détruite (fermeture de l'app).
fn stop_on_close(app: &AppHandle) {
    static HOOKED: AtomicBool = AtomicBool::new(false);
    if HOOKED.swap(true, Ordering::SeqCst) {
        return;
    }
    let windows = app.webview_windows();
    for window in windows.values() {
        window.on_window_event(|event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                pxe::stop();
            }
        });
    }
}

// ─── PXE ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn boot_lab_pxe_interfaces() -> R<Vec<Interface>> {
    blocking(|| Ok(pxe::interfaces())).await
}

#[tauri::command]
pub async fn boot_lab_pxe_status() -> R<PxeStatus> {
    blocking(|| Ok(pxe::status())).await
}

#[tauri::command]
pub async fn boot_lab_pxe_export(request: ExportRequest) -> R<Project> {
    blocking(move || pxe::export(&request)).await
}

#[tauri::command]
pub async fn boot_lab_pxe_start(app: AppHandle, id: String, bind: String) -> R<PxeStatus> {
    stop_on_close(&app);
    blocking(move || pxe::start(&id, &bind)).await
}

#[tauri::command]
pub async fn boot_lab_pxe_stop() -> R<PxeStatus> {
    blocking(|| Ok(pxe::stop())).await
}

#[tauri::command]
pub async fn boot_lab_pxe_diagnose() -> R<Diagnose> {
    blocking(|| Ok(pxe::diagnose())).await
}

/// VM iPXE (ISO officielle vérifiée) avec réseau NAT pour tester le menu.
#[tauri::command]
pub async fn boot_lab_pxe_test(platform: String, firmware: String) -> R<Machine> {
    blocking(move || {
        let image = pxe::official(&pxe::IPXE_ISO, &pxe::cache_dir())?;
        vm::iso_test(&image.to_string_lossy(), &platform, &firmware, true)
    })
    .await
}

// ─── VM ───────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct Links {
    pub virtualbox: &'static str,
    pub vmware: &'static str,
    pub hyperv: &'static str,
}

#[derive(Serialize)]
pub struct VmList {
    pub hypervisors: Detected,
    pub installers: Value,
    pub machines: Vec<Machine>,
    pub busy: bool,
    pub interfaces: Vec<String>,
    pub seven_zip: bool,
    pub links: Links,
}

#[tauri::command]
pub async fn boot_lab_vm_list() -> R<VmList> {
    blocking(|| {
        let t = hypervisors::tools();
        let (machines, busy) = {
            let s = vm::store();
            (s.machines.values().cloned().collect(), s.busy)
        };
        Ok(VmList {
            hypervisors: detected(&t),
            installers: json!({"virtualbox": t.virtualbox.is_some(), "vmware": t.vmrun.is_some() && t.diskmanager.is_some()}),
            machines,
            busy,
            interfaces: bridged_interfaces(t.virtualbox.as_ref()),
            seven_zip: t.seven_zip.is_some(),
            links: Links { virtualbox: VIRTUALBOX_URL, vmware: VMWARE_URL, hyperv: HYPERV_URL },
        })
    })
    .await
}

fn with_busy(f: impl FnOnce() -> R<Machine>) -> R<Machine> {
    {
        let mut s = vm::store();
        if s.busy {
            return Err("Une préparation de VM est déjà en cours.".into());
        }
        s.busy = true;
    }
    let result = f();
    vm::store().busy = false;
    result
}

#[tauri::command]
pub async fn boot_lab_vm_import(request: VmRequest) -> R<Machine> {
    blocking(move || with_busy(|| vm::prepare(&request, &hypervisors::tools()))).await
}

#[tauri::command]
pub async fn boot_lab_vm_install_iso(request: VmRequest) -> R<Machine> {
    blocking(move || with_busy(|| vm::prepare_install_iso(&request, &hypervisors::tools()))).await
}

/// vm-start, vm-status, vm-console, vm-stop-test, vm-discard-test, vm-eject-iso, vm-test-confirm.
#[tauri::command]
pub async fn boot_lab_vm_action(action: String, id: String) -> R<ControlResult> {
    blocking(move || vm_control::action(&action, &id)).await
}

#[tauri::command]
pub async fn boot_lab_vm_test_history() -> R<Vec<Value>> {
    blocking(|| Ok(vm_control::test_history())).await
}

#[tauri::command]
pub async fn boot_lab_iso_test(path: String, platform: String, firmware: String, network: bool) -> R<Machine> {
    blocking(move || vm::iso_test(&path, &platform, &firmware, network)).await
}

// ─── Diagnostics ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn boot_lab_network_test(request: NetworkRequest) -> R<Value> {
    blocking(move || diagnostics::network(&request)).await
}

#[tauri::command]
pub async fn boot_lab_drive_report() -> R<Value> {
    blocking(|| Ok(diagnostics::drive_report())).await
}

#[tauri::command]
pub async fn boot_lab_disk_check(drive: String, paths: Vec<String>, sizes: Vec<Estimate>) -> R<Value> {
    blocking(move || diagnostics::disk_check(&drive, &paths, &sizes)).await
}

// ─── Plans, labos, trousses ───────────────────────────────────────────────────

#[derive(Serialize)]
pub struct Plans {
    pub plans: Vec<labs::Plan>,
    pub id: String,
}

#[tauri::command]
pub async fn boot_lab_plans() -> R<Vec<labs::Plan>> {
    blocking(|| Ok(labs::plans())).await
}

#[tauri::command]
pub async fn boot_lab_plan_save(id: Option<String>, name: String, machines: Vec<PlanMachine>) -> R<Plans> {
    blocking(move || labs::plan_save(id.as_deref(), &name, &machines).map(|(plans, id)| Plans { plans, id })).await
}

#[tauri::command]
pub async fn boot_lab_plan_delete(id: String) -> R<Vec<labs::Plan>> {
    blocking(move || labs::plan_delete(&id)).await
}

#[tauri::command]
pub async fn boot_lab_lab_plan(key: String) -> R<Plans> {
    blocking(move || labs::lab_plan(&key).map(|(plans, id)| Plans { plans, id })).await
}

#[tauri::command]
pub async fn boot_lab_labs() -> R<Value> {
    Ok(labs::labs_payload())
}

#[tauri::command]
pub async fn boot_lab_profile() -> R<Profile> {
    blocking(|| Ok(labs::machine_profile())).await
}

#[tauri::command]
pub async fn boot_lab_kit_export(request: KitRequest) -> R<KitResult> {
    blocking(move || kit::export(&request)).await
}
