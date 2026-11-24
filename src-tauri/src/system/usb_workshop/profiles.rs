//! Profils Ventoy : comparer une liste d'images enregistrée à une clé et copier seulement les écarts.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : ventoy_profiles.py.

use super::design::{exists, stable_json};
use super::disks::pick_usb;
use super::media::media_paths;
use super::{blocking, file_digest, is_reparse, json_read, json_write, remove_quiet, sha256_bytes, stamp, workshop_home, MediaGuard, Res, FAT32_MAX, MEDIA_SUFFIXES};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Emitter;

const EVENT: &str = "usb-workshop-profile-progress";
static STORE: Mutex<()> = Mutex::new(());

/// État d'un fichier (taille, date, SHA-256), `None` s'il est absent.
pub fn file_state(path: &Path) -> Res<Option<Value>> {
    let link = "Le profil Ventoy contient un lien de fichier incompatible.";
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Fichier illisible : {e}")),
    };
    if meta.file_type().is_symlink() || is_reparse(path) {
        return Err(link.into());
    }
    if !meta.is_file() {
        return Err("Un emplacement du profil Ventoy est occupé par un dossier.".into());
    }
    let before = stamp(path)?;
    let digest = file_digest(path)?;
    let after = stamp(path)?;
    if before != after {
        return Err("Un fichier a changé pendant la comparaison. Recommence la comparaison.".into());
    }
    Ok(Some(json!({"size": after.0, "mtime": u64::try_from(after.1).unwrap_or(u64::MAX), "sha256": digest})))
}

fn incompatible_folder(target: &Path) -> bool {
    exists(target) && (!target.is_dir() || is_reparse(target))
}

pub fn compare(paths: &[PathBuf], target: &Path, mut advance: impl FnMut(usize)) -> Res<Vec<Value>> {
    if incompatible_folder(target) {
        return Err("Le dossier TokaISO sur la clé est incompatible.".into());
    }
    let mut rows = vec![];
    let mut names = std::collections::HashSet::new();
    for path in paths {
        let name = super::file_name(path);
        if !names.insert(name.to_lowercase()) {
            return Err("Le profil contient deux fichiers portant le même nom.".into());
        }
        let source = file_state(path)?.ok_or("Un fichier d’origine est introuvable. Mets à jour le profil.")?;
        let existing = file_state(&target.join(&name))?;
        let status = match &existing {
            None => "add",
            Some(e) if e["sha256"] == source["sha256"] => "same",
            Some(_) => "replace",
        };
        rows.push(json!({"name": name, "status": status, "source": source, "existing": existing}));
        advance(rows.len());
    }
    Ok(rows)
}

pub fn apply_change(source: &Path, target: &Path, row: &Value) -> Res<()> {
    if !target.exists() {
        std::fs::create_dir(target).map_err(|e| format!("Création du dossier TokaISO : {e}"))?;
    }
    if is_reparse(target) {
        return Err("Le dossier TokaISO sur la clé est incompatible.".into());
    }
    let destination = target.join(super::file_name(source));
    let stage = target.join(format!(".tokaiso-{}.part", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        let mut input = std::fs::File::open(source).map_err(|e| format!("Lecture de l’image : {e}"))?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)
            .map_err(|e| format!("Écriture sur la clé : {e}"))?;
        std::io::copy(&mut input, &mut output).map_err(|e| format!("Écriture sur la clé : {e}"))?;
        drop(output);
        if Value::String(file_digest(&stage)?) != row["source"]["sha256"] {
            return Err("La copie ne correspond pas au fichier comparé.".to_string());
        }
        if file_state(&destination)?.unwrap_or(Value::Null) != row["existing"] {
            return Err("Un fichier de la clé a changé. Recommence la comparaison.".to_string());
        }
        if row["existing"].is_null() && exists(&destination) {
            return Err("Un fichier de la clé a changé. Recommence la comparaison.".to_string());
        }
        std::fs::rename(&stage, &destination).map_err(|e| format!("Écriture sur la clé : {e}"))
    })();
    remove_quiet(&stage);
    result
}

fn store_path() -> Res<PathBuf> {
    Ok(workshop_home()?.join("ventoy-profiles.json"))
}

fn load() -> Res<Map<String, Value>> {
    Ok(match json_read(&store_path()?, json!({})) {
        Value::Object(map) => map,
        _ => Map::new(),
    })
}

fn listing(profiles: &Map<String, Value>) -> Value {
    json!({"profiles": profiles.values().cloned().collect::<Vec<_>>()})
}

fn find(key: &Value) -> Res<Value> {
    let _lock = STORE.lock().unwrap_or_else(|e| e.into_inner());
    key.as_str()
        .and_then(|k| load().ok()?.get(k).cloned())
        .ok_or_else(|| "Choisis un profil Ventoy.".to_string())
}

fn emit(app: &tauri::AppHandle, phase: &str, done: usize, total: usize) {
    if let Err(e) = app.emit(EVENT, json!({"phase": phase, "done": done, "total": total})) {
        tracing::warn!("usb_workshop: progression du profil non transmise : {e}");
    }
}

fn sync(app: &tauri::AppHandle, profile: &Value, payload: &Value, apply: bool) -> Res<Value> {
    let device = pick_usb(payload.get("id").and_then(Value::as_str).unwrap_or(""))?;
    let root = device.ventoy_root()?;
    let paths = media_paths(&profile["paths"], MEDIA_SUFFIXES)?;
    let target = root.join("TokaISO");
    emit(app, "compare", 0, paths.len());
    let rows = compare(&paths, &target, |done| emit(app, "compare", done, paths.len()))?;
    let changes: Vec<(&PathBuf, &Value)> = paths.iter().zip(rows.iter()).filter(|(_, r)| r["status"] != "same").collect();
    let size = |v: &Value| v["size"].as_u64().unwrap_or(0) as i128;
    // Les remplacements sont préparés à côté du fichier existant avant le renommage atomique.
    let needed = changes
        .iter()
        .enumerate()
        .map(|(index, (_, row))| {
            changes[..index].iter().map(|(_, r)| size(&r["source"]) - size(&r["existing"])).sum::<i128>() + size(&row["source"])
        })
        .max()
        .unwrap_or(0);
    let free = device.free as i128;
    let fat32 = device.format == "FAT32" && changes.iter().any(|(_, r)| r["source"]["size"].as_u64().unwrap_or(0) > FAT32_MAX);
    let token = sha256_bytes(&stable_json(&json!([profile, device.id, device.drive, rows])));
    if !apply {
        return Ok(json!({"rows": rows, "free": device.free, "needed": needed.max(0) as u64, "fat32": fat32, "token": token,
                         "can_apply": !changes.is_empty() && !fat32 && free >= needed}));
    }
    if payload.get("token").and_then(Value::as_str) != Some(token.as_str()) {
        return Err("La clé ou les fichiers ont changé. Recommence la comparaison.".into());
    }
    if fat32 || free < needed {
        return Err("L’espace libre ou le format de la clé ne permet pas cette copie.".into());
    }
    if changes.iter().any(|(_, r)| r["status"] == "replace") && payload.get("replace") != Some(&json!(true)) {
        return Err("Confirme les fichiers à remplacer sur la clé.".into());
    }
    emit(app, "copy", 0, changes.len());
    for (index, (path, row)) in changes.iter().enumerate() {
        if pick_usb(&device.id)?.drive != device.drive {
            return Err("La clé a changé. Sélectionne-la de nouveau.".into());
        }
        apply_change(path, &target, row)?;
        emit(app, "copy", index + 1, changes.len());
    }
    Ok(json!({"updated": changes.len()}))
}

// ── Commandes Tauri ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn usb_ws_profile_list() -> Result<Value, String> {
    blocking(|| {
        let _lock = STORE.lock().unwrap_or_else(|e| e.into_inner());
        Ok(listing(&load()?))
    })
    .await
}

#[tauri::command]
pub async fn usb_ws_profile_save(payload: Value) -> Result<Value, String> {
    blocking(move || {
        let name: String = payload.get("name").and_then(Value::as_str).unwrap_or("").trim().chars().take(72).collect();
        if name.is_empty() {
            return Err("Donne un nom au profil Ventoy.".into());
        }
        let paths = media_paths(payload.get("paths").unwrap_or(&Value::Null), MEDIA_SUFFIXES)?;
        let names: std::collections::HashSet<String> = paths.iter().map(|p| super::file_name(p).to_lowercase()).collect();
        if names.len() != paths.len() {
            return Err("Le profil contient deux fichiers portant le même nom.".into());
        }
        let _lock = STORE.lock().unwrap_or_else(|e| e.into_inner());
        let mut profiles = load()?;
        let key = profiles
            .iter()
            .find(|(_, row)| row["name"].as_str().is_some_and(|n| n.to_lowercase() == name.to_lowercase()))
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
        if !profiles.contains_key(&key) && profiles.len() >= 40 {
            return Err("La limite est de 40 profils Ventoy.".into());
        }
        let list: Vec<String> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        profiles.insert(key.clone(), json!({"id": key, "name": name, "paths": list}));
        json_write(&store_path()?, &Value::Object(profiles.clone()))?;
        let mut result = listing(&profiles);
        result["id"] = json!(key);
        Ok(result)
    })
    .await
}

#[tauri::command]
pub async fn usb_ws_profile_remove(payload: Value) -> Result<Value, String> {
    blocking(move || {
        let _lock = STORE.lock().unwrap_or_else(|e| e.into_inner());
        let mut profiles = load()?;
        let key = payload.get("profile").and_then(Value::as_str).unwrap_or("");
        if profiles.remove(key).is_none() {
            return Err("Choisis un profil Ventoy.".into());
        }
        json_write(&store_path()?, &Value::Object(profiles.clone()))?;
        Ok(listing(&profiles))
    })
    .await
}

#[tauri::command]
pub async fn usb_ws_profile_preview(app: tauri::AppHandle, payload: Value) -> Result<Value, String> {
    blocking(move || {
        let profile = find(payload.get("profile").unwrap_or(&Value::Null))?;
        let _guard = MediaGuard::acquire()?;
        sync(&app, &profile, &payload, false)
    })
    .await
}

#[tauri::command]
pub async fn usb_ws_profile_apply(app: tauri::AppHandle, payload: Value) -> Result<Value, String> {
    blocking(move || {
        let profile = find(payload.get("profile").unwrap_or(&Value::Null))?;
        let _guard = MediaGuard::acquire()?;
        sync(&app, &profile, &payload, true)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_classifies_files() {
        let root = std::env::temp_dir().join(format!("nitrite-profile-{}", uuid::Uuid::new_v4().simple()));
        let (src, dst) = (root.join("src"), root.join("TokaISO"));
        std::fs::create_dir_all(&src).expect("src");
        std::fs::create_dir_all(&dst).expect("dst");
        for (name, data) in [("a.iso", "A"), ("b.iso", "B"), ("c.iso", "C")] {
            std::fs::write(src.join(name), data).expect("source");
        }
        std::fs::write(dst.join("b.iso"), "B").expect("même");
        std::fs::write(dst.join("c.iso"), "ancien").expect("différent");
        let paths: Vec<PathBuf> = ["a.iso", "b.iso", "c.iso"].iter().map(|n| src.join(n)).collect();
        let mut seen = 0;
        let rows = compare(&paths, &dst, |n| seen = n).expect("comparaison");
        let status: Vec<&str> = rows.iter().map(|r| r["status"].as_str().unwrap_or("")).collect();
        assert_eq!(status, ["add", "same", "replace"]);
        assert_eq!(seen, 3);
        apply_change(&paths[2], &dst, &rows[2]).expect("remplacement");
        assert_eq!(std::fs::read_to_string(dst.join("c.iso")).expect("lu"), "C");
        apply_change(&paths[0], &dst, &rows[0]).expect("ajout");
        assert!(apply_change(&paths[0], &dst, &rows[0]).is_err(), "l'ajout existe déjà");
        let dup = vec![src.join("a.iso"), src.join("a.iso")];
        assert!(compare(&dup, &dst, |_| {}).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
