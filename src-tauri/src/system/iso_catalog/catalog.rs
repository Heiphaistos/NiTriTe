//! Port de TokaISO 7.9 (MIT, Cédrick Roy)
//! Catalogue embarqué, révisions validées et catalogue personnel (catalog_store.py).

use super::common::*;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

const BASE_JSON: &str = include_str!("data/catalog.json");
const MAX_REVISION: u64 = 4 * 1024 * 1024;
const REVISION_TYPE: &str = "tokaiso-catalogue";
pub const PERSONAL_FAMILY: &str = "Mon catalogue";
const OFFLINE_FAMILY: &str = "Utilitaires hors ligne";
const KINDS: [&str; 4] = ["official", "community", "portal", "kit"];

pub fn base_items() -> Res<Vec<Value>> {
    serde_json::from_str(BASE_JSON).map_err(|e| format!("Catalogue intégré illisible : {e}"))
}

fn find<'a>(items: &'a [Value], key: &str) -> Option<&'a Value> {
    items.iter().find(|row| str_of(row, "id") == key)
}

fn valid_id(key: &str) -> bool {
    let b = key.as_bytes();
    !b.is_empty() && b.len() <= 80 && b[0].is_ascii_lowercase() | b[0].is_ascii_digit()
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_' || *c == b'-')
}

fn valid_filename(name: &str) -> bool {
    const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let numbered = (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4
        && stem.as_bytes()[3].is_ascii_digit() && stem.as_bytes()[3] != b'0';
    !name.chars().any(|c| ":*?\"<>|/\\".contains(c) || (c as u32) < 32) && !RESERVED.contains(&stem.as_str()) && !numbered
}

fn read_limited(path: &Path) -> Res<Vec<u8>> {
    let file = std::fs::File::open(path).map_err(|e| format!("Lecture impossible : {e}"))?;
    let mut raw = Vec::new();
    file.take(MAX_REVISION + 1).read_to_end(&mut raw).map_err(|e| e.to_string())?;
    Ok(raw)
}

pub struct CatalogueStore {
    home: PathBuf,
    base: Vec<Value>,
    /// {"current": {revision, entries:[...], sha256}, "previous": ...}
    state: Value,
    personal: Map<String, Value>,
    stage: Option<Value>,
}

impl CatalogueStore {
    pub fn new(home: &Path) -> Res<Self> {
        let state = json_read(&home.join("catalog-versions.json")).filter(Value::is_object).unwrap_or_else(|| json!({}));
        let personal = json_read(&home.join("personal-catalog.json"))
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        Ok(Self { home: home.to_path_buf(), base: base_items()?, state, personal, stage: None })
    }

    fn families(&self) -> HashSet<String> {
        self.base.iter().map(|row| str_of(row, "family").to_string()).collect()
    }

    /// Validation d'une révision de catalogue (port de `CatalogueStore.validate`).
    fn validate(&self, raw: &[u8]) -> Res<Value> {
        if raw.len() as u64 > MAX_REVISION {
            return Err("La révision dépasse la taille autorisée.".into());
        }
        let data: Value = serde_json::from_slice(raw).map_err(|_| "Révision de catalogue non reconnue.".to_string())?;
        if str_of(&data, "type") != REVISION_TYPE || data.get("version").and_then(Value::as_i64) != Some(1) {
            return Err("Révision de catalogue non reconnue.".into());
        }
        let revision = label(data.get("revision"), 80);
        let rows = data.get("entries").and_then(Value::as_array).filter(|r| r.len() <= 750);
        let (Some(rows), false) = (rows, revision.is_empty()) else {
            return Err("Révision de catalogue invalide.".into());
        };
        let families = self.families();
        let mut cleaned: Vec<Value> = Vec::new();
        let empty = json!({});
        for entry in rows {
            let key = entry.get("id").and_then(Value::as_str).unwrap_or("");
            if !entry.is_object() || !valid_id(key) || key.starts_with("local-") || key.starts_with("personal-")
                || cleaned.iter().any(|row| str_of(row, "id") == key) {
                return Err("Identifiant de fiche invalide ou en double.".into());
            }
            let item = find(&self.base, key).unwrap_or(&empty);
            let pick = |field: &str| entry.get(field).or_else(|| item.get(field));
            let name = label(pick("name"), 140);
            let family = pick("family").and_then(Value::as_str).unwrap_or(OFFLINE_FAMILY).to_string();
            let sources = pick("sources").and_then(Value::as_array).filter(|s| (1..=24).contains(&s.len()));
            let (Some(sources), false, true) = (sources, name.is_empty(), families.contains(&family)) else {
                return Err("Vérifie le nom, la catégorie et les sources de la fiche.".into());
            };
            let mut valid = Vec::new();
            for source in sources {
                let kind = str_of(source, "kind");
                if !source.is_object() || !KINDS.contains(&kind) {
                    return Err("Type de source inconnu.".into());
                }
                let url = address(source.get("url"))?;
                let size = source.get("size").map_or(Some(0), Value::as_u64).filter(|s| *s <= 1 << 50);
                let filename = label(source.get("filename"), 160);
                let Some(size) = size.filter(|_| filename.is_empty() || valid_filename(&filename)) else {
                    return Err("Taille ou nom de fichier invalide.".into());
                };
                if kind == "kit" {
                    let original = sources_of(item).iter().find(|old| str_of(old, "url") == url && str_of(old, "kind") == "kit");
                    valid.push(original.cloned().ok_or("Un kit automatisé doit provenir du catalogue intégré.")?);
                    continue;
                }
                let mut value = Map::new();
                for field in ["label", "arch", "language", "format", "verified", "sample", "note"] {
                    value.insert(field.into(), json!(label(source.get(field), 400)));
                }
                value.insert("url".into(), json!(url));
                value.insert("filename".into(), json!(filename));
                value.insert("kind".into(), json!(kind));
                value.insert("size".into(), json!(size));
                value.insert("sha256".into(), json!(checksum(source.get("sha256"))?));
                if let Some(list) = source.get("platforms").and_then(Value::as_array) {
                    let names: Vec<&str> = ["virtualbox", "vmware"].into_iter().filter(|n| list.iter().any(|v| v == n)).collect();
                    value.insert("platforms".into(), json!(names));
                }
                valid.push(Value::Object(value));
            }
            let mut row = item.as_object().cloned().unwrap_or_default();
            row.insert("id".into(), json!(key));
            row.insert("name".into(), json!(name));
            row.insert("family".into(), json!(family));
            row.insert("key".into(), json!(item.get("key").and_then(Value::as_str).unwrap_or(key)));
            row.insert("image".into(), json!(str_of(item, "image")));
            row.insert("desc".into(), json!(label(pick("desc"), 450)));
            row.insert("sources".into(), Value::Array(valid));
            row.insert("group".into(), json!(label(pick("group"), 30)));
            cleaned.push(Value::Object(row));
        }
        let digest = hex(&<sha2::Sha256 as sha2::Digest>::digest(raw));
        Ok(json!({"revision": revision, "entries": cleaned, "sha256": digest}))
    }

    fn public(&self) -> Vec<Value> {
        let mut rows = self.base.clone();
        if let Some(entries) = self.state.pointer("/current/entries").and_then(Value::as_array) {
            for entry in entries {
                match rows.iter_mut().find(|row| str_of(row, "id") == str_of(entry, "id")) {
                    Some(slot) => *slot = entry.clone(),
                    None => rows.push(entry.clone()),
                }
            }
        }
        rows
    }

    pub fn active(&self) -> Vec<Value> {
        let mut rows = self.public();
        rows.extend(self.personal.values().map(card));
        rows
    }

    /// Aperçu d'une révision locale. Le chargement par URL de TokaISO n'est pas porté
    /// (règle anti-SSRF : aucune URL libre côté frontend hors entrées personnelles).
    pub fn preview(&mut self, payload: &Value) -> Res<Value> {
        self.stage = None;
        let path = file_input(payload.get("path"))?;
        let revision = self.validate(&read_limited(&path)?)?;
        let expected = checksum(payload.get("sha256"))?;
        if !expected.is_empty() && expected != str_of(&revision, "sha256") {
            return Err("L’empreinte de la révision ne correspond pas.".into());
        }
        let before = self.public();
        let mut after = self.base.clone();
        for entry in revision["entries"].as_array().into_iter().flatten() {
            match after.iter_mut().find(|row| str_of(row, "id") == str_of(entry, "id")) {
                Some(slot) => *slot = entry.clone(),
                None => after.push(entry.clone()),
            }
        }
        let mut changes = Vec::new();
        for row in &after {
            let key = str_of(row, "id");
            let old = find(&before, key);
            if old != Some(row) {
                let mut hosts: Vec<String> = sources_of(row).iter()
                    .filter_map(|s| reqwest::Url::parse(str_of(s, "url")).ok()?.host_str().map(str::to_string))
                    .collect::<HashSet<_>>().into_iter().collect();
                hosts.sort();
                changes.push(json!({"name": str_of(row, "name"), "item": key, "added": old.is_none(), "hosts": hosts}));
            }
        }
        for row in &before {
            if find(&after, str_of(row, "id")).is_none() {
                changes.push(json!({"name": str_of(row, "name"), "item": str_of(row, "id"), "removed": true, "hosts": []}));
            }
        }
        let result = json!({"revision": revision["revision"], "sha256": revision["sha256"], "changes": changes, "pinned": !expected.is_empty()});
        self.stage = Some(revision);
        Ok(result)
    }

    pub fn status(&self) -> Value {
        json!({
            "revision": self.state.pointer("/current/revision").and_then(Value::as_str).unwrap_or("7.9"),
            "canRollback": self.state.get("previous").is_some_and(|p| !p.is_null()),
            "personal": self.personal.values().collect::<Vec<_>>(),
        })
    }

    pub fn change(&mut self, apply: bool) -> Res<Value> {
        let current = self.state.get("current").cloned().unwrap_or(Value::Null);
        let state = if apply {
            let stage = self.stage.clone().ok_or("Vérifie la révision avant de l’appliquer.")?;
            json!({"current": stage, "previous": current})
        } else {
            let previous = self.state.get("previous").filter(|p| !p.is_null()).cloned().ok_or("Aucune révision précédente.")?;
            json!({"current": previous, "previous": current})
        };
        json_write(&self.home.join("catalog-versions.json"), &state)?;
        self.state = state;
        self.stage = None;
        Ok(self.status())
    }

    fn save_personal(data: &Value, existing: Option<&str>) -> Res<Value> {
        let name = label(data.get("name"), 100);
        let links = data.get("links").map_or(Some(&[][..]), |l| l.as_array().map(Vec::as_slice));
        let Some(links) = links.filter(|l| l.len() <= 12 && !name.is_empty()) else {
            return Err("Donne un nom et indique au plus 12 liens.".into());
        };
        let mut urls: Vec<String> = Vec::new();
        for link in links {
            let url = address(Some(link))?;
            if !urls.contains(&url) {
                urls.push(url);
            }
        }
        let digest = checksum(data.get("sha256"))?;
        let recorded = match data.get("path").and_then(Value::as_str).filter(|p| !p.is_empty()) {
            Some(p) => Some(file_input(Some(&json!(p)))?),
            None => None,
        };
        let key = existing.map(str::to_string).unwrap_or_else(|| format!("personal-{}", uuid::Uuid::new_v4().simple()));
        let filename = match &recorded {
            Some(p) => p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            None => label(data.get("filename"), 160),
        };
        let sources: Vec<Value> = urls.iter().map(|url| {
            let format = reqwest::Url::parse(url).ok()
                .and_then(|u| Path::new(u.path()).extension().map(|e| e.to_string_lossy().to_uppercase()))
                .unwrap_or_default();
            json!({"kind": "community", "label": "Lien personnel", "url": url, "sha256": digest, "size": 0, "format": format})
        }).collect();
        Ok(json!({"id": key, "name": name, "notes": label(data.get("notes"), 400), "sha256": digest, "links": urls,
                  "path": recorded.as_deref().map(display_path).unwrap_or_default(), "filename": filename, "sources": sources}))
    }

    pub fn personal_export(&self) -> Value {
        let rows: Vec<Value> = self.personal.values().map(|row| {
            json!({"name": row["name"], "notes": row["notes"], "sha256": row["sha256"], "links": row["links"], "filename": row["filename"]})
        }).collect();
        json!({"type": "tokaiso-personal", "version": 1, "entries": rows})
    }

    pub fn personal_action(&mut self, action: &str, payload: &Value) -> Res<Value> {
        let key = payload.get("id").and_then(Value::as_str).filter(|k| !k.is_empty());
        let mut updated = self.personal.clone();
        match action {
            "delete" => {
                if let Some(k) = key {
                    updated.remove(k);
                }
            }
            "save" => {
                if key.is_some_and(|k| !updated.contains_key(k)) {
                    return Err("Fiche personnelle introuvable.".into());
                }
                let row = Self::save_personal(payload, key)?;
                updated.insert(str_of(&row, "id").to_string(), row);
            }
            "import" => {
                let path = file_input(payload.get("path"))?;
                let raw = read_limited(&path)?;
                if raw.len() as u64 > MAX_REVISION {
                    return Err("Le fichier de partage dépasse la taille autorisée.".into());
                }
                let data: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
                let entries = data.get("entries").and_then(Value::as_array).filter(|e| e.len() <= 250);
                let (Some(entries), "tokaiso-personal", Some(1)) = (entries, str_of(&data, "type"), data.get("version").and_then(Value::as_i64)) else {
                    return Err("Fichier de partage TokaISO non reconnu.".into());
                };
                for entry in entries {
                    if !entry.is_object() {
                        return Err("Fiche personnelle invalide.".into());
                    }
                    let mut clean = entry.clone();
                    clean["path"] = json!("");
                    let row = Self::save_personal(&clean, None)?;
                    updated.insert(str_of(&row, "id").to_string(), row);
                }
            }
            _ => return Ok(self.status()),
        }
        if updated.len() > 250 {
            return Err("Le catalogue personnel peut contenir 250 fiches.".into());
        }
        json_write(&self.home.join("personal-catalog.json"), &Value::Object(updated.clone()))?;
        self.personal = updated;
        Ok(self.status())
    }
}

fn card(row: &Value) -> Value {
    let notes = str_of(row, "notes");
    json!({"id": row["id"], "name": row["name"], "desc": if notes.is_empty() { str_of(row, "filename") } else { notes },
           "family": PERSONAL_FAMILY, "key": "local", "image": "", "sources": row["sources"], "personal": true,
           "localFile": str_of(row, "filename")})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> CatalogueStore {
        CatalogueStore { home: std::env::temp_dir(), base: base_items().unwrap(), state: json!({}), personal: Map::new(), stage: None }
    }

    #[test]
    fn embedded_catalogue_shape() {
        let items = base_items().unwrap();
        assert_eq!(items.len(), 402);
        let sources: usize = items.iter().map(|i| sources_of(i).len()).sum();
        assert_eq!(sources, 845);
        let families: HashSet<_> = items.iter().map(|i| str_of(i, "family")).collect();
        assert_eq!(families.len(), 32);
        assert!(items.iter().all(|i| i.get("translations").is_none()));
        assert!(items.iter().flat_map(sources_of).all(|s| KINDS.contains(&str_of(s, "kind"))));
    }

    #[test]
    fn revision_validation() {
        let s = store();
        let ok = br#"{"type":"tokaiso-catalogue","version":1,"revision":"r1","entries":[
            {"id":"0","name":"Windows test","sources":[{"kind":"official","url":"https://example.org/w.iso","size":10}]},
            {"id":"neuf","name":"Neuf","family":"Linux","sources":[{"kind":"official","url":"https://example.org/n.iso"}]}]}"#;
        let rev = s.validate(ok).unwrap();
        assert_eq!(rev["entries"].as_array().unwrap().len(), 2);
        assert_eq!(rev["entries"][0]["family"], "Windows");
        let bad_kind = br#"{"type":"tokaiso-catalogue","version":1,"revision":"r","entries":[{"id":"x","name":"X","family":"Linux","sources":[{"kind":"evil","url":"https://e.org/a"}]}]}"#;
        assert!(s.validate(bad_kind).is_err());
        let http = br#"{"type":"tokaiso-catalogue","version":1,"revision":"r","entries":[{"id":"x","name":"X","family":"Linux","sources":[{"kind":"official","url":"http://e.org/a"}]}]}"#;
        assert!(s.validate(http).is_err());
        let kit = br#"{"type":"tokaiso-catalogue","version":1,"revision":"r","entries":[{"id":"x","name":"X","family":"Linux","sources":[{"kind":"kit","url":"https://e.org/a"}]}]}"#;
        assert!(s.validate(kit).is_err());
        let personal = br#"{"type":"tokaiso-catalogue","version":1,"revision":"r","entries":[{"id":"personal-x","name":"X","family":"Linux","sources":[{"kind":"official","url":"https://e.org/a"}]}]}"#;
        assert!(s.validate(personal).is_err());
        let bad_name = br#"{"type":"tokaiso-catalogue","version":1,"revision":"r","entries":[{"id":"x","name":"X","family":"Linux","sources":[{"kind":"official","url":"https://e.org/a","filename":"CON.iso"}]}]}"#;
        assert!(s.validate(bad_name).is_err());
    }

    #[test]
    fn personal_entries() {
        let row = CatalogueStore::save_personal(&json!({"name":"Mon ISO","links":["https://e.org/a.iso","https://e.org/a.iso"]}), None).unwrap();
        assert_eq!(row["sources"].as_array().unwrap().len(), 1);
        assert_eq!(row["sources"][0]["format"], "ISO");
        assert!(str_of(&row, "id").starts_with("personal-"));
        assert!(CatalogueStore::save_personal(&json!({"name":"X","links":["https://192.168.0.10/a.iso"]}), None).is_err());
        assert!(CatalogueStore::save_personal(&json!({"name":"X","links":["http://e.org/a.iso"]}), None).is_err());
        assert!(CatalogueStore::save_personal(&json!({"name":"","links":[]}), None).is_err());
        assert_eq!(card(&row)["family"], PERSONAL_FAMILY);
    }
}
