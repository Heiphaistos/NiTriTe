//! Modèles de clé Ventoy : exporter une organisation (images référencées par SHA-256,
//! thème, noms, mode du menu) et la recréer sur une autre clé.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : usb_templates.py.

use super::design::{
    apply_theme_settings, commit_config, exists, file_bytes, load_config, menu_inventory, put_if_changed, relative,
    relative_value, safe_path, set_menu_mode, sha256_hex_list, stable_json, volume_for, ASSETS, LIMIT,
};
use super::disks::UsbDisk;
use super::profiles::file_state;
use super::themes::{read_style, Files};
use super::{blocking, file_digest, file_input, has_suffix, is_reparse, remove_quiet, sha256_bytes, MediaGuard, Res, FAT32_MAX, MEDIA_SUFFIXES, MIB};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_FILES: usize = 400;
const MANIFEST_LIMIT: u64 = 512 * 1024;
const THEME_ASSETS: usize = 700;
/// Format TokaISO conservé : les modèles restent échangeables entre les deux applications.
const FORMAT: &str = "tokaiso-ventoy-template";

fn image_path(value: &Value) -> Res<String> {
    let path = relative_value(value)?;
    let first = path.split('/').next().unwrap_or("").to_lowercase();
    if matches!(first.as_str(), "ventoy" | "$recycle.bin" | "system volume information") || !has_suffix(&path, MEDIA_SUFFIXES) {
        return Err("Un chemin d’image du modèle USB est invalide.".into());
    }
    Ok(path)
}

fn theme_path(value: &str) -> Res<String> {
    let path = relative(value)?;
    if !path.to_lowercase().starts_with("ventoy/") || path.split('/').count() < 3 || !has_suffix(&path, ASSETS) {
        return Err("Un fichier du thème du modèle USB est invalide.".into());
    }
    Ok(path)
}

fn theme_files(config: &Value) -> Res<(Vec<String>, Vec<String>)> {
    let entry = match config.get("theme") {
        None => Map::new(),
        Some(Value::Object(m)) => m.clone(),
        Some(_) => return Ok((vec![], vec![])),
    };
    let files = match entry.get("file") {
        None => vec![],
        Some(Value::String(s)) => vec![json!(s)],
        Some(Value::Array(a)) if a.len() <= 12 => a.clone(),
        Some(_) => return Err("Le modèle contient trop de variantes de thème ou de polices.".into()),
    };
    let fonts = match entry.get("fonts") {
        None => vec![],
        Some(Value::Array(a)) if a.len() <= 32 => a.clone(),
        Some(_) => return Err("Le modèle contient trop de variantes de thème ou de polices.".into()),
    };
    let convert = |rows: Vec<Value>| -> Res<Vec<String>> {
        rows.iter().map(|v| theme_path(v.as_str().ok_or("Chemin Ventoy invalide.")?)).collect()
    };
    Ok((convert(files)?, convert(fonts)?))
}

/// Réglages portables du modèle, limités aux images listées (`files` en minuscules).
fn settings_for(config: &Value, files: &HashSet<String>) -> Res<Value> {
    let (choices, fonts) = theme_files(config)?;
    let mut result = Map::new();
    if !choices.is_empty() {
        let original = &config["theme"];
        let mut theme = Map::new();
        theme.insert(
            "file".into(),
            if choices.len() == 1 { json!(format!("/{}", choices[0])) } else { json!(choices.iter().map(|p| format!("/{p}")).collect::<Vec<_>>()) },
        );
        if !fonts.is_empty() {
            theme.insert("fonts".into(), json!(fonts.iter().map(|p| format!("/{p}")).collect::<Vec<_>>()));
        }
        if let Some(fit) = original.get("resolution_fit").and_then(Value::as_str).filter(|f| *f == "0" || *f == "1") {
            theme.insert("resolution_fit".into(), json!(fit));
        }
        let gfx = regex::Regex::new(r"^\d{3,4}x\d{3,4}(?:x\d{1,2})?$").map_err(|e| e.to_string())?;
        if let Some(mode) = original.get("gfxmode").and_then(Value::as_str).filter(|m| gfx.is_match(m)) {
            theme.insert("gfxmode".into(), json!(mode));
        }
        result.insert("theme".into(), Value::Object(theme));
    }
    let mut filtered = vec![];
    for row in config.get("menu_alias").and_then(Value::as_array).into_iter().flatten() {
        let Some(row) = row.as_object() else { continue };
        let field = if row.contains_key("image") { "image" } else { "dir" };
        let Some(label) = row.get("alias").and_then(Value::as_str) else { continue };
        if label.is_empty() || label.chars().count() > 120 || label.chars().any(|c| (c as u32) < 32) {
            continue;
        }
        let Ok(path) = relative_value(row.get(field).unwrap_or(&Value::Null)) else { continue };
        let folded = path.to_lowercase();
        if files.contains(&folded) || (field == "dir" && files.iter().any(|n| n.starts_with(&format!("{folded}/")))) {
            filtered.push(json!({ field: format!("/{path}"), "alias": label }));
        }
    }
    if !filtered.is_empty() {
        filtered.truncate(MAX_FILES);
        result.insert("menu_alias".into(), Value::Array(filtered));
    }
    let mode = config
        .get("control")
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().rev().find_map(|r| r.get("VTOY_DEFAULT_MENU_MODE").and_then(Value::as_str).filter(|m| *m == "0" || *m == "1")));
    if let Some(mode) = mode {
        result.insert("control".into(), json!([{"VTOY_DEFAULT_MENU_MODE": mode}]));
    }
    Ok(Value::Object(result))
}

fn package_assets(root: &Path, config: &Value) -> Res<Files> {
    let (themes, fonts) = theme_files(config)?;
    let mut assets = Files::new();
    let mut total = 0usize;
    let mut read = |name: &str, assets: &mut Files| -> Res<()> {
        if assets.contains_key(name) {
            return Ok(());
        }
        if assets.len() >= THEME_ASSETS {
            return Err("Le thème contient trop de fichiers pour le modèle.".into());
        }
        let data = file_bytes(&safe_path(root, &theme_path(name)?)?, LIMIT - total)?;
        total += data.len();
        assets.insert(name.to_string(), data);
        Ok(())
    };
    let mut folders: Vec<String> = themes
        .iter()
        .map(|theme| {
            let parts: Vec<&str> = theme.split('/').collect();
            let length = if parts.len() >= 4 && parts[1].to_lowercase() == "themes" { 3 } else { 2 };
            parts[..length.min(parts.len())].join("/")
        })
        .collect();
    folders.sort();
    folders.dedup();
    for base in folders {
        let walker = walkdir::WalkDir::new(safe_path(root, &base)?)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !e.file_type().is_dir() || !is_reparse(e.path()));
        for entry in walker {
            let entry = entry.map_err(|e| format!("Lecture du thème : {e}"))?;
            if !entry.file_type().is_file() || !has_suffix(&entry.file_name().to_string_lossy(), ASSETS) {
                continue;
            }
            let rel = entry.path().strip_prefix(root).map_err(|_| "Chemin Ventoy invalide.")?;
            let rel: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            read(&rel.join("/"), &mut assets)?;
        }
    }
    for file in themes.iter().chain(fonts.iter()) {
        read(file, &mut assets)?;
    }
    Ok(assets)
}

pub struct Template {
    pub name: String,
    pub files: Vec<Value>,
    pub settings: Value,
    pub assets: Files,
    pub digest: String,
}

pub fn open_template(path: &Path) -> Res<Template> {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(u64::MAX);
    if is_reparse(path) || size > LIMIT as u64 + MANIFEST_LIMIT + MIB {
        return Err("Le modèle USB ZIP est trop volumineux ou redirigé.".into());
    }
    let unreadable = || "Impossible de lire ce modèle USB ZIP.".to_string();
    let file = std::fs::File::open(path).map_err(|_| unreadable())?;
    let mut bundle = zip::ZipArchive::new(file).map_err(|_| unreadable())?;
    let mut names = vec![];
    for i in 0..bundle.len() {
        names.push(bundle.by_index(i).map_err(|_| unreadable())?.name().map_err(|_| unreadable())?.to_string());
    }
    if names.len() > THEME_ASSETS + 1 || names.iter().filter(|n| *n == "manifest.json").count() != 1 {
        return Err("Archive de modèle USB invalide.".into());
    }
    let mut manifest = Value::Null;
    let mut assets = Files::new();
    let mut known = HashSet::new();
    let mut total = 0u64;
    for (index, name) in names.iter().enumerate() {
        let entry = bundle.by_index(index).map_err(|_| unreadable())?;
        if name == "manifest.json" {
            if entry.size() > MANIFEST_LIMIT {
                return Err("Le manifeste du modèle USB est trop volumineux.".into());
            }
            let mut raw = vec![];
            entry.take(MANIFEST_LIMIT + 1).read_to_end(&mut raw).map_err(|_| unreadable())?;
            manifest = serde_json::from_slice(&raw).map_err(|_| unreadable())?;
            continue;
        }
        let link = entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000);
        let Some(inner) = name.strip_prefix("assets/").filter(|_| !link && !entry.is_dir()) else {
            return Err("Un élément du modèle ZIP est incompatible.".into());
        };
        let item = theme_path(inner)?;
        total += entry.size();
        if !known.insert(item.to_lowercase()) || total > LIMIT as u64 {
            return Err("Les fichiers du thème dépassent la limite ou contiennent des doublons.".into());
        }
        let mut data = vec![];
        entry.take(LIMIT as u64 + 1).read_to_end(&mut data).map_err(|_| unreadable())?;
        if data.len() as u64 > LIMIT as u64 {
            return Err("Les fichiers du thème dépassent la limite ou contiennent des doublons.".into());
        }
        assets.insert(item, data);
    }
    if manifest.get("type").and_then(Value::as_str) != Some(FORMAT) || manifest.get("version") != Some(&json!(1)) {
        return Err("Ce fichier n’est pas un modèle USB TokaISO ou NiTriTe.".into());
    }
    let name = manifest.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    let rows = manifest.get("files").and_then(Value::as_array).cloned().unwrap_or_default();
    if name.trim().is_empty() || name.chars().count() > 72 || rows.is_empty() || rows.len() > MAX_FILES {
        return Err("Modèle USB incomplet.".into());
    }
    let invalid = "Une image du modèle USB est invalide.";
    let mut files = vec![];
    let mut paths = HashSet::new();
    for row in &rows {
        let row = row.as_object().ok_or(invalid)?;
        let path = image_path(row.get("path").unwrap_or(&Value::Null))?;
        let size = row.get("size").and_then(Value::as_u64).filter(|s| *s > 0);
        let digest = row.get("sha256").and_then(Value::as_str).filter(|d| d.len() == 64 && d.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)));
        let (Some(size), Some(digest)) = (size, digest) else { return Err(invalid.into()) };
        if !paths.insert(path.to_lowercase()) {
            return Err(invalid.into());
        }
        files.push(json!({"path": path, "size": size, "sha256": digest}));
    }
    let settings = manifest.get("config").cloned().unwrap_or_else(|| json!({}));
    if !settings.is_object() {
        return Err("Configuration du modèle USB invalide.".into());
    }
    let settings = settings_for(&settings, &paths)?;
    let (themes, fonts) = theme_files(&settings)?;
    if themes.iter().chain(fonts.iter()).any(|p| !assets.contains_key(p)) {
        return Err("Il manque un fichier du thème dans le modèle.".into());
    }
    for theme in &themes {
        read_style(&assets, theme)?;
    }
    let digest = sha256_bytes(&stable_json(&json!([name, files, settings, sha256_hex_list(&assets)])));
    Ok(Template { name, files, settings, assets, digest })
}

fn key(row: &Value) -> (String, u64) {
    (row["sha256"].as_str().unwrap_or("").to_string(), row["size"].as_u64().unwrap_or(0))
}

/// Copies locales des images du modèle, retrouvées par (SHA-256, taille) parmi les images
/// choisies (`sources`) et le dossier de téléchargements de NiTriTe.
fn candidates(files: &[&Value], payload: &Value) -> Res<HashMap<(String, u64), PathBuf>> {
    let sources = payload.get("sources").and_then(Value::as_array).cloned().unwrap_or_default();
    if sources.len() > 24 {
        return Err("Sélectionne au plus 24 images.".into());
    }
    let mut pool: Vec<PathBuf> = sources.iter().map(|s| file_input(s.as_str().unwrap_or(""))).collect::<Res<_>>()?;
    if let Ok(entries) = std::fs::read_dir(crate::utils::paths::downloads_dir()) {
        pool.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_file()));
    }
    let mut hashes: HashMap<PathBuf, String> = HashMap::new();
    let mut matches = HashMap::new();
    for row in files {
        let wanted = key(row);
        if matches.contains_key(&wanted) {
            continue;
        }
        for path in &pool {
            if is_reparse(path) || std::fs::metadata(path).map(|m| m.len()).ok() != Some(wanted.1) {
                continue;
            }
            if !hashes.contains_key(path) {
                hashes.insert(path.clone(), file_digest(path)?);
            }
            if hashes.get(path) == Some(&wanted.0) {
                matches.insert(wanted.clone(), path.clone());
                break;
            }
        }
    }
    Ok(matches)
}

fn copy_media(source: &Path, volume: &Path, row: &Value) -> Res<()> {
    let relative_path = row["path"].as_str().unwrap_or("");
    let destination = safe_path(volume, relative_path)?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Création du dossier sur la clé : {e}"))?;
    }
    let destination = safe_path(volume, relative_path)?;
    let stage = destination.with_file_name(format!(".tokaiso-{}.part", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        let mut input = std::fs::File::open(source).map_err(|e| format!("Lecture de l’image : {e}"))?;
        let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(&stage).map_err(|e| format!("Écriture sur la clé : {e}"))?;
        std::io::copy(&mut input, &mut output).and_then(|_| output.flush()).and_then(|_| output.sync_all()).map_err(|e| format!("Écriture sur la clé : {e}"))?;
        drop(output);
        let size = std::fs::metadata(&stage).map(|m| m.len()).unwrap_or(0);
        if Some(size) != row["size"].as_u64() || Some(file_digest(&stage)?.as_str()) != row["sha256"].as_str() {
            return Err("La copie ne correspond pas à l’empreinte du modèle.".to_string());
        }
        if exists(&safe_path(volume, relative_path)?) {
            return Err("Un fichier est apparu sur la clé. Réouvre le modèle avant de réessayer.".to_string());
        }
        std::fs::rename(&stage, safe_path(volume, relative_path)?).map_err(|e| format!("Écriture sur la clé : {e}"))
    })();
    remove_quiet(&stage);
    result
}

fn merge_settings(before: &Value, settings: &Value) -> Res<Value> {
    let mut result = before.clone();
    let map = result.as_object_mut().ok_or("Le fichier ventoy.json est invalide.")?;
    if let Some(theme) = settings.get("theme").and_then(Value::as_object) {
        apply_theme_settings(map, theme)?;
    }
    if let Some(aliases) = settings.get("menu_alias").and_then(Value::as_array) {
        let identity = |row: &Value| {
            let text = |k: &str| row.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
            text("image").or_else(|| text("dir")).unwrap_or("").to_lowercase()
        };
        let keys: HashSet<String> = aliases.iter().map(identity).collect();
        let mut merged: Vec<Value> = before
            .get("menu_alias")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter(|r| !r.is_object() || !keys.contains(&identity(r))).cloned().collect())
            .unwrap_or_default();
        merged.extend(aliases.iter().cloned());
        map.insert("menu_alias".into(), Value::Array(merged));
    }
    if let Some(mode) = settings.pointer("/control/0/VTOY_DEFAULT_MENU_MODE").and_then(Value::as_str) {
        set_menu_mode(map, mode)?;
    }
    Ok(result)
}

fn preview(payload: &Value) -> Res<Value> {
    let template = open_template(&file_input(payload.get("path").and_then(Value::as_str).unwrap_or(""))?)?;
    let id = payload.get("id").and_then(Value::as_str).unwrap_or("");
    let volume = if id.is_empty() { None } else { Some(volume_for(id)?.1) };
    let mut present = vec![];
    for row in &template.files {
        let on_usb = match &volume {
            Some(v) => {
                let path = safe_path(v, row["path"].as_str().unwrap_or(""))?;
                path.is_file() && std::fs::metadata(&path).map(|m| m.len()).ok() == row["size"].as_u64()
            }
            None => false,
        };
        present.push(on_usb);
    }
    let missing: Vec<&Value> = template.files.iter().zip(&present).filter(|(_, p)| !**p).map(|(r, _)| r).collect();
    let matches = candidates(&missing, payload)?;
    let rows: Vec<Value> = template
        .files
        .iter()
        .zip(&present)
        .map(|(row, on_usb)| {
            let mut out = row.clone();
            let name = row["path"].as_str().unwrap_or("").rsplit('/').next().unwrap_or("").to_string();
            out["name"] = json!(name);
            out["present"] = json!(on_usb);
            out["available"] = json!(*on_usb || matches.contains_key(&key(row)));
            out
        })
        .collect();
    let ready = rows.iter().all(|r| r["available"] == true);
    Ok(json!({"name": template.name, "digest": template.digest, "files": rows, "ready": ready}))
}

fn export(payload: &Value, device: &UsbDisk, volume: &Path) -> Res<Value> {
    let name = payload.get("name").and_then(Value::as_str).unwrap_or("");
    if name.trim().is_empty() || name.chars().count() > 72 {
        return Err("Donne un nom au modèle USB.".into());
    }
    let target = PathBuf::from(payload.get("target").and_then(Value::as_str).unwrap_or(""));
    let parent_ok = target.parent().is_some_and(Path::is_dir);
    let zip_name = target.extension().is_some_and(|e| e.to_string_lossy().to_lowercase() == "zip");
    if !target.is_absolute() || !parent_ok || !zip_name || (exists(&target) && (!target.is_file() || is_reparse(&target))) {
        return Err("Choisis un fichier ZIP de destination.".into());
    }
    let config = load_config(volume)?;
    let mut files = vec![];
    for image in menu_inventory(volume, &config) {
        let path = image_path(&image["path"])?;
        let state = file_state(&safe_path(volume, &path)?)?.filter(|s| s["size"].as_u64().unwrap_or(0) > 0);
        let state = state.ok_or("Une image de la clé a changé. Actualise la sélection.")?;
        files.push(json!({"path": path, "size": state["size"], "sha256": state["sha256"]}));
    }
    if files.is_empty() {
        return Err("Ajoute des images à la clé avant d’enregistrer son modèle.".into());
    }
    let names: HashSet<String> = files.iter().map(|r| r["path"].as_str().unwrap_or("").to_lowercase()).collect();
    let settings = settings_for(&config, &names)?;
    let assets = package_assets(volume, &settings)?;
    let (_, current) = volume_for(&device.id)?;
    if current != volume || load_config(volume)? != config {
        return Err("La clé ou sa configuration a changé.".into());
    }
    let manifest = json!({"type": FORMAT, "version": 1, "name": name.trim(), "files": files, "config": settings});
    let stage = target.with_file_name(format!(".tokaiso-{}.zip", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&stage).map_err(|e| format!("Écriture du modèle : {e}"))?;
        let mut bundle = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let fail = |e: &dyn std::fmt::Display| format!("Écriture du modèle : {e}");
        bundle.start_file("manifest.json", options).map_err(|e| fail(&e))?;
        bundle.write_all(&stable_json(&manifest)).map_err(|e| fail(&e))?;
        for (asset, data) in &assets {
            bundle.start_file(format!("assets/{asset}"), options).map_err(|e| fail(&e))?;
            bundle.write_all(data).map_err(|e| fail(&e))?;
        }
        bundle.finish().map_err(|e| fail(&e))?;
        open_template(&stage)?;
        std::fs::rename(&stage, &target).map_err(|e| fail(&e))
    })();
    remove_quiet(&stage);
    result?;
    Ok(json!({"count": files.len()}))
}

fn apply(payload: &Value, device: &UsbDisk, volume: &Path) -> Res<Value> {
    let template = open_template(&file_input(payload.get("path").and_then(Value::as_str).unwrap_or(""))?)?;
    if payload.get("digest").and_then(Value::as_str) != Some(template.digest.as_str()) {
        return Err("Le modèle USB a changé. Réouvre le ZIP.".into());
    }
    let mut additions = vec![];
    let (mut same, mut needed) = (0usize, 0u64);
    let mut missing = vec![];
    for row in &template.files {
        let existing = file_state(&safe_path(volume, row["path"].as_str().unwrap_or(""))?)?;
        match existing {
            Some(e) if key(&e) == key(row) => same += 1,
            Some(_) => return Err("Un fichier différent occupe déjà un emplacement du modèle sur la clé.".into()),
            None => missing.push(row),
        }
    }
    let copies = candidates(&missing, payload)?;
    for row in missing {
        let source = copies
            .get(&key(row))
            .ok_or("Certaines images du modèle sont introuvables dans les téléchargements ou la bibliothèque.")?;
        needed += row["size"].as_u64().unwrap_or(0);
        additions.push((source.clone(), row));
    }
    for (path, data) in &template.assets {
        let target = safe_path(volume, path)?;
        if exists(&target) {
            if file_bytes(&target, data.len().max(MIB as usize))? != *data {
                return Err("Un fichier de thème différent occupe déjà un emplacement du modèle sur la clé.".into());
            }
        } else {
            needed += data.len() as u64;
        }
    }
    if device.format == "FAT32" && additions.iter().any(|(_, r)| r["size"].as_u64().unwrap_or(0) > FAT32_MAX) {
        return Err("Un fichier dépasserait la limite FAT32 de 4 Gio.".into());
    }
    if device.free < needed {
        return Err("Espace libre insuffisant sur la clé USB.".into());
    }
    let current = load_config(volume)?;
    let same_key = || -> Res<()> {
        if volume_for(&device.id)?.1 != volume {
            return Err("La clé a changé. Sélectionne-la de nouveau.".into());
        }
        Ok(())
    };
    for (source, row) in &additions {
        same_key()?;
        copy_media(source, volume, row)?;
    }
    same_key()?;
    for (path, data) in &template.assets {
        put_if_changed(volume, path, data, false)?;
    }
    let updated = merge_settings(&current, &template.settings)?;
    let backup = commit_config(volume, &current, &updated, &format!("Modèle USB · {}", template.name), &[])?;
    Ok(json!({"copied": additions.len(), "same": same, "backup": backup}))
}

// ── Commandes Tauri ───────────────────────────────────────────────────────────

fn device_id(payload: &Value) -> &str {
    payload.get("id").and_then(Value::as_str).unwrap_or("")
}

#[tauri::command]
pub async fn usb_ws_template_preview(payload: Value) -> Result<Value, String> {
    blocking(move || preview(&payload)).await
}

#[tauri::command]
pub async fn usb_ws_template_export(payload: Value) -> Result<Value, String> {
    blocking(move || {
        let _guard = MediaGuard::acquire()?;
        let (device, volume) = volume_for(device_id(&payload))?;
        export(&payload, &device, &volume)
    })
    .await
}

#[tauri::command]
pub async fn usb_ws_template_apply(payload: Value) -> Result<Value, String> {
    blocking(move || {
        let _guard = MediaGuard::acquire()?;
        let (device, volume) = volume_for(device_id(&payload))?;
        apply(&payload, &device, &volume)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).expect("zip");
        let mut bundle = zip::ZipWriter::new(file);
        for (name, data) in entries {
            bundle.start_file(*name, zip::write::SimpleFileOptions::default()).expect("entrée");
            bundle.write_all(data).expect("données");
        }
        bundle.finish().expect("fin");
    }

    #[test]
    fn open_template_validates_archive() {
        let dir = std::env::temp_dir().join(format!("nitrite-template-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).expect("dossier");
        let digest = "a".repeat(64);
        let manifest = json!({"type": FORMAT, "version": 1, "name": "Secours",
            "files": [{"path": "Linux/ubuntu.iso", "size": 10, "sha256": digest}],
            "config": {"control": [{"VTOY_DEFAULT_MENU_MODE": "1"}], "menu_alias": [{"image": "/Linux/ubuntu.iso", "alias": "Ubuntu"}, {"image": "/autre.iso", "alias": "X"}]}})
        .to_string();
        let good = dir.join("ok.zip");
        write_zip(&good, &[("manifest.json", manifest.as_bytes())]);
        let template = open_template(&good).expect("modèle valide");
        assert_eq!(template.settings["menu_alias"], json!([{"image": "/Linux/ubuntu.iso", "alias": "Ubuntu"}]));
        assert_eq!(template.settings["control"], json!([{"VTOY_DEFAULT_MENU_MODE": "1"}]));

        let stray = dir.join("stray.zip");
        write_zip(&stray, &[("manifest.json", manifest.as_bytes()), ("evil.txt", b"x")]);
        assert!(open_template(&stray).is_err());
        let traversal = json!({"type": FORMAT, "version": 1, "name": "X", "files": [{"path": "../x.iso", "size": 1, "sha256": digest}]}).to_string();
        let bad = dir.join("bad.zip");
        write_zip(&bad, &[("manifest.json", traversal.as_bytes())]);
        assert!(open_template(&bad).is_err());
        let missing_theme = json!({"type": FORMAT, "version": 1, "name": "X", "files": [{"path": "a.iso", "size": 1, "sha256": digest}],
            "config": {"theme": {"file": "/ventoy/t/theme.txt"}}}).to_string();
        let theme = dir.join("theme.zip");
        write_zip(&theme, &[("manifest.json", missing_theme.as_bytes())]);
        assert!(open_template(&theme).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_settings_replaces_matching_aliases() {
        let before = json!({"menu_alias": [{"image": "/a.iso", "alias": "Ancien"}, {"image": "/b.iso", "alias": "B"}]});
        let settings = json!({"menu_alias": [{"image": "/A.iso", "alias": "Nouveau"}], "control": [{"VTOY_DEFAULT_MENU_MODE": "0"}]});
        let merged = merge_settings(&before, &settings).expect("fusion");
        assert_eq!(merged["menu_alias"], json!([{"image": "/b.iso", "alias": "B"}, {"image": "/A.iso", "alias": "Nouveau"}]));
        assert_eq!(merged["control"], json!([{"VTOY_DEFAULT_MENU_MODE": "0"}]));
    }
}
