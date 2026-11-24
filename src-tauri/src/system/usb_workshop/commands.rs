//! Commandes Tauri de l'atelier USB (disques, Ventoy, vérifications, santé, persistance, VM, outils).
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : actions usb-*, usb-media-*, usb-health-*, usb-boot-*, usb-workshop-*.
//! Les commandes des thèmes, modèles et profils sont dans themes.rs, templates.rs et profiles.rs.

use super::{blocking, boot, disks, health, media, vm, writers, Res};
use serde_json::{json, Value};
use std::path::PathBuf;

#[tauri::command]
pub async fn usb_ws_list() -> Res<Value> {
    blocking(|| Ok(json!({"disks": disks::usb_disks()}))).await
}

#[tauri::command]
pub async fn usb_ws_images(payload: Value) -> Res<Value> {
    blocking(move || writers::images(&payload)).await
}

#[tauri::command]
pub async fn usb_ws_writers() -> Res<Value> {
    blocking(writers::writers).await
}

#[tauri::command]
pub async fn usb_ws_set_writer(payload: Value) -> Res<Value> {
    blocking(move || writers::set_writer(&payload)).await
}

#[tauri::command]
pub async fn usb_ws_open_writer(payload: Value) -> Res<Value> {
    blocking(move || writers::open_writer(&payload)).await
}

/// Aperçu « ce qui va se passer » : obligatoire avant usb_ws_media_run.
#[tauri::command]
pub async fn usb_ws_media_preview(payload: Value) -> Res<Value> {
    blocking(move || media::preview_summary(&payload)).await
}

#[tauri::command]
pub async fn usb_ws_media_run(payload: Value) -> Res<Value> {
    blocking(move || media::run(&payload)).await
}

/// Exporte un rapport texte vers le fichier .txt choisi par la boîte de dialogue.
#[tauri::command]
pub async fn usb_ws_save_text(payload: Value) -> Res<Value> {
    blocking(move || {
        let path = PathBuf::from(payload["path"].as_str().unwrap_or(""));
        let text = payload["text"].as_str().ok_or("Rapport vide.")?;
        let parent_ok = path.parent().is_some_and(|p| p.is_dir() && !super::is_reparse(p));
        let txt = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt"));
        if !path.is_absolute() || !parent_ok || !txt || text.len() > 1024 * 1024 || (path.exists() && super::is_reparse(&path)) {
            return Err("Choisis un fichier .txt de destination.".into());
        }
        std::fs::write(&path, text).map_err(|e| format!("Enregistrement du rapport : {e}"))?;
        Ok(json!({"path": path.to_string_lossy()}))
    })
    .await
}

#[tauri::command]
pub async fn usb_ws_health(payload: Value) -> Res<Value> {
    blocking(move || health::run(payload["id"].as_str().unwrap_or(""), payload["kind"].as_str().unwrap_or(""))).await
}

#[tauri::command]
pub async fn usb_ws_boot_images(payload: Value) -> Res<Value> {
    blocking(move || boot::images(payload["id"].as_str().unwrap_or(""))).await
}

#[tauri::command]
pub async fn usb_ws_boot_persistence(payload: Value) -> Res<Value> {
    blocking(move || boot::persistence(&payload)).await
}

#[tauri::command]
pub async fn usb_ws_boot_list() -> Res<Value> {
    blocking(vm::list).await
}

#[tauri::command]
pub async fn usb_ws_boot_create(app: tauri::AppHandle, payload: Value) -> Res<Value> {
    blocking(move || vm::create(&app, &payload)).await
}

#[tauri::command]
pub async fn usb_ws_boot_control(payload: Value) -> Res<Value> {
    blocking(move || vm::control(payload["action"].as_str().unwrap_or(""), payload["machine"].as_str().unwrap_or(""))).await
}
