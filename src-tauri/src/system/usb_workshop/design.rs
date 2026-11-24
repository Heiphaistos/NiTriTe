//! Modifications réversibles de ventoy.json : chemins sûrs, sauvegardes horodatées,
//! restauration, organisation du menu.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : ventoy_design.py (partie configuration).

use super::disks::{pick_usb, UsbDisk};
use super::{has_suffix, is_reparse, remove_quiet, sha256_bytes, Res, MEDIA_SUFFIXES};
use chrono::{Datelike, Timelike};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const LIMIT: usize = 32 * 1024 * 1024;
pub const CONFIG_LIMIT: usize = 1024 * 1024;
pub const ASSETS: &[&str] = &[".txt", ".png", ".jpg", ".jpeg", ".tga", ".bmp", ".pf2"];
pub const GROUPS: &[&str] = &["Windows", "Linux", "Secours", "Serveurs", "Autres"];
const FIRMWARE_SUFFIXES: &[&str] = &["_legacy", "_uefi", "_ia32", "_aa64", "_mips"];
/// Dossier des sauvegardes : nom TokaISO conservé pour relire les clés déjà préparées.
const BACKUPS: &str = "ventoy/tokaiso-backups";
const CHANGED: &str = "La configuration de la clé a changé. Regénère l’aperçu.";

/// Nom réservé de Windows (PureWindowsPath.is_reserved).
pub fn is_reserved(part: &str) -> bool {
    let base = part.split('.').next().unwrap_or("");
    let base = base.split(':').next().unwrap_or("").trim_end_matches(' ').to_uppercase();
    if matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$") {
        return true;
    }
    let mut chars = base.chars();
    let prefix: String = chars.by_ref().take(3).collect();
    let rest: Vec<char> = chars.collect();
    (prefix == "COM" || prefix == "LPT") && rest.len() == 1 && matches!(rest[0], '1'..='9' | '¹' | '²' | '³')
}

/// Chemin relatif Ventoy validé, séparateurs `/`, sans `/` initial.
pub fn relative(name: &str) -> Res<String> {
    let err = || "Chemin Ventoy invalide.".to_string();
    if name.is_empty() || name.chars().count() > 700 {
        return Err(err());
    }
    let normalized = name.replace('\\', "/");
    let trimmed = normalized.strip_prefix('/').unwrap_or(&normalized);
    let parts: Vec<&str> = trimmed.split('/').collect();
    let bad = |part: &&str| {
        part.is_empty()
            || *part == "."
            || *part == ".."
            || part.ends_with('.')
            || part.ends_with(' ')
            || part.chars().count() > 200
            || is_reserved(part)
            || part.chars().any(|c| (c as u32) < 32 || ":*?\"<>|".contains(c))
    };
    if parts.len() > 16 || parts.iter().any(bad) {
        return Err(err());
    }
    Ok(parts.join("/"))
}

/// Valeur JSON attendue comme chemin relatif.
pub fn relative_value(value: &Value) -> Res<String> {
    relative(value.as_str().ok_or("Chemin Ventoy invalide.")?)
}

/// Résout un chemin relatif sous `root` sans suivre de jonction ni de lien.
pub fn safe_path(root: &Path, name: &str) -> Res<PathBuf> {
    if !root.is_absolute() || !root.is_dir() || is_reparse(root) {
        return Err("Dossier Ventoy introuvable ou redirigé.".into());
    }
    let mut target = root.to_path_buf();
    for part in relative(name)?.split('/') {
        target.push(part);
        match std::fs::symlink_metadata(&target) {
            Ok(_) if is_reparse(&target) => return Err("Un dossier ou un fichier Ventoy est redirigé.".into()),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Chemin Ventoy illisible : {e}")),
        }
    }
    Ok(target)
}

pub fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

pub fn volume_for(identity: &str) -> Res<(UsbDisk, PathBuf)> {
    let device = pick_usb(identity)?;
    let root = device.ventoy_root()?;
    Ok((device, root))
}

pub fn file_bytes(path: &Path, limit: usize) -> Res<Vec<u8>> {
    if exists(path) && is_reparse(path) {
        return Err("Un dossier ou un fichier Ventoy est redirigé.".into());
    }
    let file = std::fs::File::open(path).map_err(|e| format!("Lecture Ventoy impossible : {e}"))?;
    let mut data = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|e| format!("Lecture Ventoy impossible : {e}"))?;
    if data.len() > limit {
        return Err("Le fichier Ventoy dépasse la taille autorisée.".into());
    }
    Ok(data)
}

pub fn config_bytes(data: &Value) -> Res<Vec<u8>> {
    let mut raw = serde_json::to_vec_pretty(data).map_err(|_| "Le fichier ventoy.json est invalide.".to_string())?;
    raw.push(b'\n');
    if raw.len() > CONFIG_LIMIT {
        return Err("Le fichier ventoy.json est trop volumineux.".into());
    }
    Ok(raw)
}

pub fn parse_config(raw: &[u8]) -> Res<Value> {
    let raw = raw.strip_prefix(b"\xef\xbb\xbf").unwrap_or(raw);
    match serde_json::from_slice::<Value>(raw) {
        Ok(value) if value.is_object() => {
            config_bytes(&value)?;
            Ok(value)
        }
        _ => Err("Le fichier ventoy.json est invalide.".into()),
    }
}

pub fn load_config(volume: &Path) -> Res<Value> {
    let path = safe_path(volume, "ventoy/ventoy.json")?;
    if !exists(&path) {
        return Ok(json!({}));
    }
    parse_config(&file_bytes(&path, CONFIG_LIMIT)?)
}

/// JSON canonique (clés triées, compact) : la Map serde_json est un BTreeMap dans ce projet.
pub fn stable_json(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_default()
}

/// `[[nom, sha256], ...]` trié par nom : empreinte d'un ensemble de fichiers.
pub fn sha256_hex_list(files: &std::collections::BTreeMap<String, Vec<u8>>) -> Value {
    Value::Array(files.iter().map(|(k, v)| json!([k, sha256_bytes(v)])).collect())
}

/// Écrit un fichier par renommage atomique ; refuse d'écraser sauf `replace`.
pub fn put_if_changed(root: &Path, name: &str, data: &[u8], replace: bool) -> Res<()> {
    let path = safe_path(root, name)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Création du dossier Ventoy : {e}"))?;
    }
    let path = safe_path(root, name)?;
    if exists(&path) {
        if !path.is_file() {
            return Err("Un dossier occupe le nom du fichier demandé.".into());
        }
        if file_bytes(&path, data.len().max(CONFIG_LIMIT))? == data {
            return Ok(());
        }
        if !replace {
            return Err("Un autre fichier occupe le nom demandé.".into());
        }
    }
    let stage = path.with_file_name(format!(".tokaiso-{}.part", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)
            .map_err(|e| format!("Écriture Ventoy impossible : {e}"))?;
        out.write_all(data).and_then(|_| out.sync_all()).map_err(|e| format!("Écriture Ventoy impossible : {e}"))?;
        drop(out);
        let final_path = safe_path(root, name)?;
        if !replace && exists(&final_path) {
            return Err("Un autre fichier occupe le nom demandé.".to_string());
        }
        std::fs::rename(&stage, &final_path).map_err(|e| format!("Écriture Ventoy impossible : {e}"))
    })();
    remove_quiet(&stage);
    result
}

fn move_file(root: &Path, source: &str, destination: &str) -> Res<()> {
    let before = safe_path(root, source)?;
    let after = safe_path(root, destination)?;
    if !before.is_file() || exists(&after) {
        return Err("Un fichier à déplacer est absent ou sa destination existe déjà.".into());
    }
    if let Some(parent) = after.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Création du dossier Ventoy : {e}"))?;
    }
    std::fs::rename(safe_path(root, source)?, safe_path(root, destination)?)
        .map_err(|e| format!("Déplacement impossible : {e}"))
}

/// Ventoy attend la casse exacte `ventoy/ventoy.json` : corrige une casse différente.
pub fn normalize_config_names(volume: &Path) -> Res<()> {
    for name in ["ventoy", "ventoy/ventoy.json"] {
        let path = safe_path(volume, name)?;
        let (Some(parent), Some(wanted)) = (path.parent(), path.file_name()) else { continue };
        if !exists(&path) {
            continue;
        }
        let wanted = wanted.to_string_lossy().into_owned();
        let entries = std::fs::read_dir(parent).map_err(|e| format!("Lecture de la clé : {e}"))?;
        for entry in entries.flatten() {
            let stored = entry.file_name().to_string_lossy().into_owned();
            if stored != wanted && stored.to_lowercase() == wanted.to_lowercase() {
                std::fs::rename(entry.path(), &path).map_err(|_| "Chemin Ventoy invalide.".to_string())?;
            }
        }
    }
    Ok(())
}

/// Enregistre une configuration vérifiée après sauvegarde de la précédente et des déplacements.
pub fn commit_config(volume: &Path, previous: &Value, updated: &Value, title: &str, moves: &[(String, String)]) -> Res<String> {
    if load_config(volume)? != *previous {
        return Err(CHANGED.into());
    }
    normalize_config_names(volume)?;
    if previous == updated && moves.is_empty() {
        return Ok(String::new());
    }
    let target = config_bytes(updated)?;
    let now = chrono::Utc::now();
    let id = uuid::Uuid::new_v4().simple().to_string();
    let name = format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}-{}.json",
        now.year(), now.month(), now.day(), now.hour(), now.minute(), now.second(), &id[..12]
    );
    let backup = json!({"date": now.to_rfc3339(), "title": title, "config": previous, "applied": updated,
                        "moves": moves.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>()});
    put_if_changed(volume, &format!("{BACKUPS}/{name}"), &stable_json(&backup), false)?;
    let mut completed: Vec<&(String, String)> = vec![];
    let result = (|| {
        for pair in moves {
            move_file(volume, &pair.0, &pair.1)?;
            completed.push(pair);
        }
        if load_config(volume)? != *previous {
            return Err(CHANGED.to_string());
        }
        put_if_changed(volume, "ventoy/ventoy.json", &target, true)
    })();
    if let Err(e) = result {
        for (before, after) in completed.into_iter().rev() {
            if let Err(undo) = move_file(volume, after, before) {
                tracing::warn!("usb_workshop: retour du déplacement {after} impossible : {undo}");
            }
        }
        return Err(e);
    }
    Ok(name)
}

fn backup_name_ok(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() == 33
        && b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'-'
        && b[9..15].iter().all(u8::is_ascii_digit)
        && b[15] == b'-'
        && b[16..28].iter().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
        && &b[28..] == b".json"
}

pub fn read_backup(volume: &Path, name: &str) -> Res<Value> {
    if !backup_name_ok(name) {
        return Err("Sauvegarde Ventoy introuvable.".into());
    }
    let path = safe_path(volume, &format!("{BACKUPS}/{name}"))?;
    let invalid = || "Sauvegarde Ventoy invalide.".to_string();
    let data: Value = serde_json::from_slice(&file_bytes(&path, 2 * CONFIG_LIMIT + 128 * 1024)?).map_err(|_| invalid())?;
    let moves_ok = data.get("moves").and_then(Value::as_array).is_some_and(|m| m.len() <= 100);
    let config = data.get("config").filter(|c| c.is_object()).ok_or_else(invalid)?;
    if !moves_ok {
        return Err(invalid());
    }
    config_bytes(config)?;
    Ok(data)
}

pub fn list_backups(volume: &Path) -> Res<Vec<Value>> {
    let folder = safe_path(volume, BACKUPS)?;
    if !folder.is_dir() {
        return Ok(vec![]);
    }
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .map_err(|e| format!("Lecture des sauvegardes : {e}"))?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .collect();
    names.sort_unstable_by(|a, b| b.cmp(a));
    Ok(names
        .into_iter()
        .take(50)
        .filter_map(|name| {
            let data = read_backup(volume, &name).ok()?;
            let date = data.get("date")?.clone();
            let title = data.get("title")?.clone();
            Some(json!({"id": name, "date": date, "title": title}))
        })
        .collect())
}

/// Annule uniquement les réglages modifiés par l'opération sauvegardée (`None` = absent).
pub fn restore_values(current: Option<&Value>, old: Option<&Value>, applied: Option<&Value>) -> Res<Option<Value>> {
    if old == applied {
        return Ok(current.cloned());
    }
    if current == applied || current == old {
        return Ok(old.cloned());
    }
    let empty = Map::new();
    if let (Some(Value::Object(cur)), Some(Value::Object(app))) = (current, applied) {
        let old_map = match old {
            None => Some(&empty),
            Some(Value::Object(o)) => Some(o),
            _ => None,
        };
        if let Some(old_map) = old_map {
            let mut result = cur.clone();
            let keys: HashSet<&String> = old_map.keys().chain(app.keys()).collect();
            for key in keys {
                match restore_values(cur.get(key), old_map.get(key), app.get(key))? {
                    None => {
                        result.remove(key);
                    }
                    Some(v) => {
                        result.insert(key.clone(), v);
                    }
                }
            }
            return Ok(if !result.is_empty() || old.is_some() { Some(Value::Object(result)) } else { None });
        }
    }
    Err("Un réglage de cette sauvegarde a été modifié depuis. Choisis une sauvegarde plus récente.".into())
}

fn replace_paths(value: &Value, renamed: &[(String, String)]) -> Value {
    match value {
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), replace_paths(v, renamed))).collect()),
        Value::Array(items) => Value::Array(items.iter().map(|v| replace_paths(v, renamed)).collect()),
        Value::String(s) => {
            let folded = s.to_lowercase();
            renamed
                .iter()
                .find(|(old, _)| old.to_lowercase() == folded)
                .map(|(_, new)| Value::String(new.clone()))
                .unwrap_or_else(|| value.clone())
        }
        other => other.clone(),
    }
}

/// Images de la clé (hors dossier ventoy), avec leur nom d'affichage actuel.
pub fn menu_inventory(volume: &Path, config: &Value) -> Vec<Value> {
    let labels: std::collections::HashMap<String, String> = config
        .get("menu_alias")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    let image = r.get("image")?.as_str()?;
                    Some((image.to_lowercase(), r.get("alias").and_then(Value::as_str).unwrap_or("").to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut images = vec![];
    let mut visits = 0;
    let walker = walkdir::WalkDir::new(volume).follow_links(false).into_iter().filter_entry(|e| {
        if e.depth() == 0 || !e.file_type().is_dir() {
            return true;
        }
        let name = e.file_name().to_string_lossy().to_lowercase();
        !matches!(name.as_str(), "ventoy" | "$recycle.bin" | "system volume information") && !is_reparse(e.path())
    });
    for entry in walker.flatten() {
        if entry.file_type().is_dir() {
            visits += 1;
            if visits > 260 {
                break;
            }
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !has_suffix(&name, MEDIA_SUFFIXES) || is_reparse(entry.path()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(volume) else { continue };
        let rel: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        let Ok(location) = relative(&rel.join("/")).map(|r| format!("/{r}")) else { continue };
        let alias = labels.get(&location.to_lowercase()).cloned().unwrap_or_default();
        images.push(json!({"path": location, "name": name, "alias": alias}));
        if images.len() >= 400 {
            break;
        }
    }
    images.sort_by_key(|r| r["path"].as_str().unwrap_or("").to_lowercase());
    images
}

pub fn firmware_keys(config: &Map<String, Value>, plugin: &str) -> Vec<String> {
    std::iter::once(plugin.to_string())
        .chain(FIRMWARE_SUFFIXES.iter().map(|s| format!("{plugin}{s}")).filter(|k| config.contains_key(k)))
        .collect()
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

pub fn set_menu_mode(config: &mut Map<String, Value>, value: &str) -> Res<()> {
    const SETTING: &str = "VTOY_DEFAULT_MENU_MODE";
    for plugin in firmware_keys(config, "control") {
        let settings = config.get(&plugin).cloned().unwrap_or_else(|| json!([]));
        let rows = settings.as_array().ok_or("Les options du menu dans ventoy.json sont invalides.")?;
        let mut kept: Vec<Value> = rows
            .iter()
            .map(|row| match row {
                Value::Object(m) => Value::Object(m.iter().filter(|(k, _)| *k != SETTING).map(|(k, v)| (k.clone(), v.clone())).collect()),
                other => other.clone(),
            })
            .filter(truthy)
            .collect();
        kept.push(json!({ SETTING: value }));
        config.insert(plugin, Value::Array(kept));
    }
    Ok(())
}

/// Plan de menu : noms d'affichage, rangement dans /TokaISO/<groupe>/, mode liste/arborescence.
pub fn menu_plan(volume: &Path, config: &Value, edits: &Value, mode: &str) -> Res<(Value, Vec<(String, String)>)> {
    let invalid = || "Paramètres du menu Ventoy invalides.".to_string();
    let edits = edits.as_array().filter(|e| e.len() <= 100).ok_or_else(invalid)?;
    if !matches!(mode, "" | "tree" | "list") {
        return Err(invalid());
    }
    let mut renamed: Vec<(String, String)> = vec![];
    let mut labels: Vec<(String, String, String)> = vec![]; // (clé, source, alias)
    let mut destinations: HashSet<String> = HashSet::new();
    for edit in edits {
        let edit = edit.as_object().ok_or_else(invalid)?;
        let source = format!("/{}", relative_value(edit.get("path").unwrap_or(&Value::Null))?);
        let path = safe_path(volume, &source)?;
        let key = source.to_lowercase();
        if labels.iter().any(|(k, _, _)| *k == key) || !path.is_file() || !has_suffix(&source, MEDIA_SUFFIXES) {
            return Err("Une image du menu est absente ou sélectionnée deux fois.".into());
        }
        let alias = edit.get("alias").unwrap_or(&json!("")).as_str().map(String::from);
        let alias = alias
            .filter(|a| a.chars().count() <= 100 && !a.chars().any(|c| (c as u32) < 32))
            .ok_or("Le nom d’affichage doit contenir au plus 100 caractères, sur une seule ligne.")?;
        let folder = edit.get("folder").unwrap_or(&json!("")).as_str().map(String::from);
        let folder = folder.filter(|f| f.is_empty() || GROUPS.contains(&f.as_str())).ok_or("Choisis un dossier du menu Ventoy.")?;
        labels.push((key.clone(), source.clone(), alias.trim().to_string()));
        if !folder.is_empty() {
            let name = super::file_name(&path);
            let destination = format!("/TokaISO/{folder}/{name}");
            let folded = destination.to_lowercase();
            if folded != key {
                if destinations.contains(&folded) || exists(&safe_path(volume, &destination)?) {
                    return Err("Ce dossier contient déjà une image portant le même nom.".into());
                }
                renamed.push((source.clone(), destination));
                destinations.insert(folded);
            }
        }
    }
    let mut updated = replace_paths(config, &renamed);
    let map = updated.as_object_mut().ok_or("Le fichier ventoy.json est invalide.")?;
    if !labels.is_empty() {
        let old = config.get("menu_alias").cloned().unwrap_or_else(|| json!([]));
        let old = old.as_array().ok_or("Les noms d’affichage du fichier ventoy.json sont invalides.")?;
        let kept: Vec<Value> = old
            .iter()
            .filter(|row| match row.as_object() {
                None => true,
                Some(m) => {
                    let image = m.get("image").map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string()));
                    let image = image.unwrap_or_default().to_lowercase();
                    !labels.iter().any(|(k, _, _)| *k == image)
                }
            })
            .cloned()
            .collect();
        let mut kept = match replace_paths(&Value::Array(kept), &renamed) {
            Value::Array(a) => a,
            _ => vec![],
        };
        for (_, source, alias) in &labels {
            if !alias.is_empty() {
                let image = renamed.iter().find(|(s, _)| s == source).map(|(_, d)| d.clone()).unwrap_or_else(|| source.clone());
                kept.push(json!({"image": image, "alias": alias}));
            }
        }
        map.insert("menu_alias".into(), Value::Array(kept));
    }
    if !mode.is_empty() {
        set_menu_mode(map, if mode == "tree" { "1" } else { "0" })?;
    }
    Ok((updated, renamed))
}

/// Applique le thème choisi dans chaque mode de démarrage déjà configuré.
pub fn apply_theme_settings(config: &mut Map<String, Value>, selection: &Map<String, Value>) -> Res<()> {
    let invalid = "Les options du thème dans ventoy.json sont invalides.";
    let fonts: Vec<Value> = selection.get("fonts").and_then(Value::as_array).cloned().unwrap_or_default();
    for key in firmware_keys(config, "theme") {
        let old = config.get(&key).cloned().unwrap_or_else(|| json!({}));
        let old = old.as_object().ok_or(invalid)?;
        let mut theme: Map<String, Value> = old
            .iter()
            .filter(|(k, _)| *k != "default_file" && *k != "resolution_fit")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (k, v) in selection {
            theme.insert(k.clone(), v.clone());
        }
        theme.insert("display_mode".into(), json!("GUI"));
        if !fonts.is_empty() {
            let previous = old.get("fonts").cloned().unwrap_or_else(|| json!([]));
            let previous = previous.as_array().filter(|p| p.iter().all(Value::is_string)).ok_or(invalid)?;
            let mut merged: Vec<Value> = vec![];
            for font in previous.iter().chain(fonts.iter()) {
                if !merged.contains(font) {
                    merged.push(font.clone());
                }
            }
            theme.insert("fonts".into(), Value::Array(merged));
        }
        config.insert(key, Value::Object(theme));
    }
    Ok(())
}

/// Différences clé par clé de ventoy.json, plus les déplacements prévus.
pub fn changes(before: &Value, after: &Value, moves: &[(String, String)]) -> Vec<Value> {
    let empty = Map::new();
    let b = before.as_object().unwrap_or(&empty);
    let a = after.as_object().unwrap_or(&empty);
    let mut keys: Vec<&String> = b.keys().chain(a.keys()).collect();
    keys.sort();
    keys.dedup();
    let show = |v: Option<&Value>| v.unwrap_or(&Value::Null).to_string().chars().take(600).collect::<String>();
    let mut result: Vec<Value> = keys
        .into_iter()
        .filter(|k| b.get(*k) != a.get(*k))
        .map(|k| json!({"key": k, "before": show(b.get(k)), "after": show(a.get(k))}))
        .collect();
    result.extend(moves.iter().map(|(s, d)| json!({"key": "Déplacer", "before": s, "after": d})));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nitrite-design-{tag}-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).expect("dossier de test");
        dir
    }

    #[test]
    fn relative_rejects_traversal_and_reserved() {
        assert_eq!(relative("\\ventoy\\ventoy.json").ok().as_deref(), Some("ventoy/ventoy.json"));
        for bad in ["", "a/../b", "a//b", "CON", "x/aux.txt", "a:b", "fin.", "COM1.iso"] {
            assert!(relative(bad).is_err(), "{bad}");
        }
        assert!(relative("COM10.iso").is_ok());
    }

    #[test]
    fn safe_path_stays_under_root() {
        let root = temp_root("safe");
        assert!(safe_path(&root, "../x").is_err());
        assert_eq!(safe_path(&root, "a/b.iso").ok(), Some(root.join("a").join("b.iso")));
        assert!(safe_path(Path::new("relatif"), "a").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn restore_values_reverses_only_changed_keys() {
        let old = json!({"theme": {"file": "/a"}, "keep": 1});
        let applied = json!({"theme": {"file": "/b"}, "keep": 1, "added": true});
        let current = json!({"theme": {"file": "/b"}, "keep": 1, "added": true, "other": 2});
        let restored = restore_values(Some(&current), Some(&old), Some(&applied)).expect("restauration");
        assert_eq!(restored, Some(json!({"theme": {"file": "/a"}, "keep": 1, "other": 2})));
        let conflict = json!({"theme": {"file": "/c"}, "keep": 1});
        assert!(restore_values(Some(&conflict), Some(&old), Some(&applied)).is_err());
    }

    #[test]
    fn menu_mode_replaces_previous_setting() {
        let mut config = json!({"control": [{"VTOY_DEFAULT_MENU_MODE": "0", "VTOY_X": "1"}], "control_uefi": []});
        let map = config.as_object_mut().expect("objet");
        set_menu_mode(map, "1").expect("mode");
        assert_eq!(map["control"], json!([{"VTOY_X": "1"}, {"VTOY_DEFAULT_MENU_MODE": "1"}]));
        assert_eq!(map["control_uefi"], json!([{"VTOY_DEFAULT_MENU_MODE": "1"}]));
    }

    #[test]
    fn theme_settings_cover_firmware_overrides() {
        let mut config = json!({"theme_uefi": {"default_file": 1, "fonts": ["/f1"]}});
        let map = config.as_object_mut().expect("objet");
        let selection = json!({"file": "/t/theme.txt", "fonts": ["/f2", "/f1"]});
        apply_theme_settings(map, selection.as_object().expect("objet")).expect("thème");
        assert_eq!(map["theme"]["display_mode"], "GUI");
        assert_eq!(map["theme_uefi"]["fonts"], json!(["/f1", "/f2"]));
        assert!(map["theme_uefi"].get("default_file").is_none());
    }

    #[test]
    fn menu_plan_moves_and_renames() {
        let root = temp_root("plan");
        std::fs::write(root.join("ubuntu.iso"), b"x").expect("iso");
        let config = json!({"menu_alias": [{"image": "/ubuntu.iso", "alias": "Ancien"}], "persistence": [{"image": "/ubuntu.iso"}]});
        let edits = json!([{"path": "/ubuntu.iso", "alias": " Ubuntu ", "folder": "Linux"}]);
        let (after, moves) = menu_plan(&root, &config, &edits, "tree").expect("plan");
        assert_eq!(moves, vec![("/ubuntu.iso".to_string(), "/TokaISO/Linux/ubuntu.iso".to_string())]);
        assert_eq!(after["menu_alias"], json!([{"image": "/TokaISO/Linux/ubuntu.iso", "alias": "Ubuntu"}]));
        assert_eq!(after["persistence"][0]["image"], "/TokaISO/Linux/ubuntu.iso");
        assert_eq!(after["control"], json!([{"VTOY_DEFAULT_MENU_MODE": "1"}]));
        assert!(menu_plan(&root, &config, &json!([{"path": "/absent.iso"}]), "").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn commit_config_writes_backup_and_detects_changes() {
        let root = temp_root("commit");
        let before = load_config(&root).expect("vide");
        assert_eq!(before, json!({}));
        let after = json!({"theme": {"file": "/x"}});
        let name = commit_config(&root, &before, &after, "Essai", &[]).expect("commit");
        assert!(backup_name_ok(&name));
        assert_eq!(load_config(&root).expect("relue"), after);
        assert_eq!(list_backups(&root).expect("liste").len(), 1);
        let backup = read_backup(&root, &name).expect("sauvegarde");
        let restored = restore_values(Some(&after), backup.get("config"), backup.get("applied")).expect("retour");
        assert_eq!(restored, Some(json!({})));
        assert!(commit_config(&root, &before, &after, "Essai", &[]).is_err(), "config changée");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn changes_lists_differences() {
        let diff = changes(&json!({"a": 1, "b": 2}), &json!({"a": 1, "c": 3}), &[("/x".into(), "/y".into())]);
        assert_eq!(diff.len(), 3);
        assert_eq!(diff[2]["key"], "Déplacer");
    }
}
