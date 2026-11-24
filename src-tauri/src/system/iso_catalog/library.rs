//! Port de TokaISO 7.9 (MIT, Cédrick Roy)
//! Inspection en lecture seule des médias (ISO 9660, El Torito, WIM/ESD) et
//! index persistant des fichiers locaux vérifiés (library.py).

use super::common::*;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const EXTENSIONS: [(&str, &str); 18] = [
    (".iso", "ISO"), (".img", "IMG"), (".dmg", "DMG"), (".wim", "WIM"), (".esd", "ESD"),
    (".img.gz", "IMG.GZ"), (".img.xz", "IMG.XZ"), (".img.bz2", "IMG.BZ2"), (".img.zst", "IMG.ZST"),
    (".iso.bz2", "ISO.BZ2"), (".iso.gz", "ISO.GZ"), (".iso.xz", "ISO.XZ"),
    (".ova", "OVA"), (".vhdx", "VHDX"), (".qcow2", "QCOW2"), (".zip", "ZIP"), (".7z", "7Z"), (".rar", "RAR"),
];
const MAX_INDEXED: usize = 500;

pub fn media_type(path: &Path) -> &'static str {
    let lower = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    // Les doubles extensions (.img.xz) passent avant les simples.
    EXTENSIONS.iter().filter(|(ext, _)| ext.matches('.').count() == 2).chain(EXTENSIONS.iter())
        .find(|(ext, _)| lower.ends_with(ext)).map(|(_, f)| *f).unwrap_or("")
}

fn read_at(stream: &mut File, offset: u64, count: u64, limit: u64) -> Vec<u8> {
    if offset.checked_add(count).is_none_or(|end| end > limit) {
        return Vec::new();
    }
    let mut buf = Vec::new();
    if stream.seek(SeekFrom::Start(offset)).is_ok() {
        let _ = stream.by_ref().take(count).read_to_end(&mut buf);
    }
    buf
}

fn le32(b: &[u8], at: usize) -> u64 {
    b.get(at..at + 4).map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as u64)
}

fn le64(b: &[u8], at: usize) -> u64 {
    b.get(at..at + 8).map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap_or([0; 8])))
}

fn xml_text(block: &str, path: &[&str]) -> Option<String> {
    let mut scope = block;
    for (i, tag) in path.iter().enumerate() {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        let start = scope.find(&open)? + open.len();
        let end = scope[start..].find(&close)? + start;
        if i == path.len() - 1 {
            let raw = scope[start..end].trim();
            let text = raw.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&");
            return Some(text).filter(|t| !t.is_empty());
        }
        scope = &scope[start..end];
    }
    None
}

/// Lit la table XML d'une image WIM/ESD : index, édition, architecture, langues.
pub fn parse_wim_xml(text: &str) -> Vec<Value> {
    let upper = text.to_uppercase();
    if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") {
        return Vec::new();
    }
    let Ok(image) = regex::Regex::new(r#"(?s)<IMAGE\s+INDEX="(\d+)"\s*>(.*?)</IMAGE>"#) else { return Vec::new() };
    image.captures_iter(text).take(100).map(|cap| {
        let block = &cap[2];
        let display = xml_text(block, &["DISPLAYNAME"]);
        let name = xml_text(block, &["NAME"]);
        let langs = xml_text(block, &["WINDOWS", "LANGUAGES"]).map(|l| {
            l.split("<LANGUAGE>").skip(1).filter_map(|p| p.split("</LANGUAGE>").next()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>()
        }).unwrap_or_default();
        let arch = match xml_text(block, &["WINDOWS", "ARCH"]).as_deref() { Some("0") => "x86", Some("5") => "ARM", Some("9") => "x64", Some("12") => "ARM64", _ => "" };
        json!({
            "index": &cap[1],
            "edition": xml_text(block, &["WINDOWS", "EDITIONID"]).or(display.clone()).or(name.clone()).unwrap_or_else(|| "Édition inconnue".into()),
            "label": display.or(name).unwrap_or_default(),
            "architecture": arch,
            "languages": langs,
        })
    }).collect()
}

fn wim_images(stream: &mut File, offset: u64, length: u64) -> Vec<Value> {
    let header = read_at(stream, offset, 96, offset + length);
    if header.len() < 96 || !header.starts_with(b"MSWIM\0\0\0") {
        return Vec::new();
    }
    let size_and_flags = le64(&header, 72);
    let size = size_and_flags & ((1 << 56) - 1);
    let flags = size_and_flags >> 56;
    let xml_offset = le64(&header, 80);
    if size == 0 || size > 4 * 1024 * 1024 || xml_offset.saturating_add(size) > length || flags & 4 != 0 {
        return Vec::new();
    }
    let xml = read_at(stream, offset + xml_offset, size, offset + length);
    let units: Vec<u16> = xml.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    match String::from_utf16(&units) {
        Ok(text) => parse_wim_xml(text.trim_matches(|c| c == '\u{feff}' || c == '\0' || c == ' ' || c == '\r' || c == '\n')),
        Err(_) => Vec::new(),
    }
}

fn directory(stream: &mut File, record: &[u8], limit: u64) -> HashMap<Vec<u8>, Vec<u8>> {
    let mut items = HashMap::new();
    if record.len() < 18 {
        return items;
    }
    let start = le32(record, 2) * 2048;
    let length = le32(record, 10);
    if length == 0 || length > 1024 * 1024 {
        return items;
    }
    let data = read_at(stream, start, length, limit);
    let mut offset = 0usize;
    while offset < data.len() {
        let count = data[offset] as usize;
        if count == 0 {
            offset = (offset / 2048 + 1) * 2048;
            continue;
        }
        let entry = &data[offset..(offset + count).min(data.len())];
        if entry.len() >= 34 {
            let name_len = entry[32] as usize;
            let raw = entry.get(33..33 + name_len).unwrap_or(&[]);
            let name: Vec<u8> = raw.split(|b| *b == b';').next().unwrap_or(&[]).to_ascii_uppercase();
            items.insert(name, entry.to_vec());
        }
        offset += count;
    }
    items
}

pub fn inspect_iso(stream: &mut File, limit: u64) -> Value {
    let descriptor_bytes = (limit.saturating_sub(32768) / 2048).min(16) * 2048;
    let sectors = read_at(stream, 32768, descriptor_bytes, limit);
    let mut root: Vec<u8> = Vec::new();
    let mut boot_catalog = 0u64;
    for sector in sectors.chunks(2048) {
        if sector.get(1..6) != Some(b"CD001") {
            continue;
        }
        if sector[0] == 1 {
            root = sector.get(156..190).unwrap_or(&[]).to_vec();
        } else if sector[0] == 0 && sector.get(7..39).is_some_and(|s| {
            let end = s.iter().rposition(|b| *b != 0 && *b != b' ').map_or(0, |p| p + 1);
            &s[..end] == b"EL TORITO SPECIFICATION"
        }) {
            boot_catalog = le32(sector, 71) * 2048;
        }
    }
    let nsr = sectors.windows(5).any(|w| w == b"NSR02" || w == b"NSR03");
    let format = if !root.is_empty() { "ISO 9660" } else if nsr { "UDF" } else { "ISO à confirmer" };
    let mut result = json!({"format": format, "boot": [], "windows": [], "contents": [], "detail": ""});
    if boot_catalog > 0 {
        let entries = read_at(stream, boot_catalog, 2048, limit);
        if entries.len() == 2048 && entries[0] == 1 && entries[30..32] == [0x55, 0xaa] {
            let mut modes = std::collections::HashSet::new();
            let mut platform = entries[1];
            for entry in entries.chunks(32).skip(1) {
                if entry[0] == 0x90 || entry[0] == 0x91 {
                    platform = entry[1];
                } else if entry[0] == 0x88 && le32(entry, 8) > 0 {
                    modes.insert(platform);
                }
            }
            let boot: Vec<&str> = [(0u8, "BIOS"), (0xef, "UEFI")].iter().filter(|(c, _)| modes.contains(c)).map(|(_, n)| *n).collect();
            result["boot"] = json!(boot);
        }
    }
    if root.is_empty() {
        result["detail"] = json!("Catalogue ISO 9660 indisponible. Contenu et éditions non lisibles par cet inspecteur.");
        return result;
    }
    let entries = directory(stream, &root, limit);
    let sources_dir = entries.get(&b"SOURCES"[..]).cloned().unwrap_or_default();
    let source_files = directory(stream, &sources_dir, limit);
    let mut contents = Vec::new();
    for filename in ["BOOT.WIM", "INSTALL.WIM", "INSTALL.ESD", "INSTALL.SWM"] {
        let Some(record) = source_files.get(filename.as_bytes()) else { continue };
        contents.push(format!("SOURCES/{filename}"));
        if filename == "INSTALL.WIM" || filename == "INSTALL.ESD" {
            let start = le32(record, 2) * 2048;
            let length = le32(record, 10).min(limit.saturating_sub(start));
            result["windows"] = json!(wim_images(stream, start, length));
        }
    }
    if entries.contains_key(&b"EFI"[..]) {
        contents.push("Dossier EFI".into());
    }
    if entries.contains_key(&b"BOOTMGR"[..]) {
        contents.push("BOOTMGR".into());
    }
    result["contents"] = json!(contents);
    result
}

pub fn inspect(path: &Path) -> Res<Value> {
    let fmt = media_type(path);
    let limit = path.metadata().map_err(|e| e.to_string())?.len();
    let mut stream = File::open(path).map_err(|e| format!("Lecture impossible : {e}"))?;
    let mut details = json!({"format": fmt, "boot": [], "windows": [], "contents": [], "detail": ""});
    match fmt {
        "ISO" => return Ok(inspect_iso(&mut stream, limit)),
        "WIM" | "ESD" => {
            let images = wim_images(&mut stream, 0, limit);
            if images.is_empty() {
                details["detail"] = json!("Métadonnées Windows absentes ou non lisibles.");
            }
            details["windows"] = json!(images);
        }
        "IMG" => {
            let sample = read_at(&mut stream, 0, 4104.min(limit), limit);
            let mut contents = Vec::new();
            if sample.get(512..520) == Some(b"EFI PART") || sample.get(4096..4104) == Some(b"EFI PART") {
                contents.push("Table GPT");
            }
            if sample.get(510..512) == Some(&[0x55, 0xaa]) {
                contents.push("Signature MBR");
            }
            details["contents"] = json!(contents);
        }
        "DMG" => {
            if limit >= 512 && read_at(&mut stream, limit - 512, 4, limit) == b"koly" {
                details["contents"] = json!(["Conteneur Apple UDIF"]);
            }
        }
        "ZIP" => {
            // Seul le répertoire central est lu, jamais les données des fichiers.
            let tail_len = limit.min(65557);
            let tail = read_at(&mut stream, limit - tail_len, tail_len, limit);
            let central = tail.windows(4).rposition(|w| w == b"PK\x05\x06").map(|o| le32(&tail, o + 12)).unwrap_or(0);
            let names: Vec<String> = if central > 0 && central <= 8 * 1024 * 1024 {
                match zip::ZipArchive::new(stream) {
                    Ok(archive) => archive.file_names().filter_map(Result::ok).filter(|n| {
                        let l = n.to_lowercase();
                        [".iso", ".img", ".wim", ".esd"].iter().any(|e| l.ends_with(e))
                    }).take(30).map(|n| n.to_string()).collect(),
                    Err(_) => {
                        details["detail"] = json!("Index ZIP non lisible.");
                        Vec::new()
                    }
                }
            } else { Vec::new() };
            if !names.is_empty() {
                details["detail"] = json!("Extrais l’image depuis le ZIP avant de la copier sur ton support.");
            }
            details["contents"] = json!(names);
        }
        f if f.contains('.') || f == "7Z" || f == "RAR" => {
            details["detail"] = json!("Archive compressée. Extrais le média avant de l’inspecter.");
        }
        _ => {}
    }
    Ok(details)
}

pub fn is_reparse(path: &Path) -> bool {
    match path.symlink_metadata() {
        #[cfg(windows)]
        Ok(meta) => {
            use std::os::windows::fs::MetadataExt;
            meta.file_attributes() & 0x400 != 0 || meta.file_type().is_symlink()
        }
        #[cfg(not(windows))]
        Ok(meta) => meta.file_type().is_symlink(),
        Err(_) => true,
    }
}

fn mtime_ns(meta: &std::fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos() as u64).unwrap_or(0)
}

pub struct MediaLibrary {
    path: PathBuf,
    entries: Mutex<Map<String, Value>>,
    matches: Mutex<HashMap<String, Vec<String>>>,
    scan: Mutex<()>,
}

impl MediaLibrary {
    pub fn new(home: &Path, items: &[Value]) -> Self {
        let path = home.join("medias.json");
        let entries = json_read(&path).and_then(|v| v.as_object().cloned()).unwrap_or_default();
        let lib = Self { path, entries: Mutex::new(entries), matches: Mutex::new(HashMap::new()), scan: Mutex::new(()) };
        lib.catalogue_changed(items);
        lib
    }

    pub fn catalogue_changed(&self, items: &[Value]) {
        let mut matches: HashMap<String, Vec<String>> = HashMap::new();
        for item in items {
            for source in sources_of(item) {
                let digest = str_of(source, "sha256").to_lowercase();
                if !digest.is_empty() {
                    let names = matches.entry(digest).or_default();
                    let name = str_of(item, "name").to_string();
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
        *self.matches.lock().unwrap_or_else(|p| p.into_inner()) = matches;
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, Map<String, Value>> {
        self.entries.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn save(&self, entries: &Map<String, Value>) -> Res<()> {
        json_write(&self.path, &Value::Object(entries.clone()))
    }

    fn status(row: &Value) -> &'static str {
        match Path::new(str_of(row, "path")).metadata() {
            Ok(m) if !m.is_file() => "absent",
            Ok(m) if m.len() != u64_of(row, "size") || mtime_ns(&m) != u64_of(row, "modified") => "changed",
            Ok(_) => "ready",
            Err(_) => "absent",
        }
    }

    fn known(&self, digest: &str) -> Vec<String> {
        self.matches.lock().unwrap_or_else(|p| p.into_inner()).get(digest).cloned().unwrap_or_default()
    }

    pub fn list(&self) -> Vec<Value> {
        let rows: Vec<Value> = self.entries().values().map(|e| {
            let mut row = e.clone();
            row["state"] = json!(Self::status(e));
            row
        }).collect();
        let mut digest_count: HashMap<String, i64> = HashMap::new();
        for row in rows.iter().filter(|r| str_of(r, "state") == "ready") {
            *digest_count.entry(str_of(row, "sha256").to_string()).or_default() += 1;
        }
        rows.into_iter().map(|mut row| {
            let digest = str_of(&row, "sha256").to_string();
            let ready = str_of(&row, "state") == "ready";
            row["duplicates"] = json!(if ready { digest_count.get(&digest).copied().unwrap_or(1) - 1 } else { 0 });
            row["known"] = json!(self.known(&digest));
            row
        }).collect()
    }

    /// Fichier local indexé dont l'empreinte correspond à la source (avec revérification facultative).
    pub fn find_verified(&self, source: &Value, recheck: bool) -> Option<Value> {
        let digest = str_of(source, "sha256").to_lowercase();
        if !is_hex64(&digest) || matches!(str_of(source, "kind"), "portal" | "kit") {
            return None;
        }
        let size = u64_of(source, "size");
        let candidates: Vec<Value> = self.entries().values()
            .filter(|r| str_of(r, "sha256") == digest && (size == 0 || size == u64_of(r, "size"))).cloned().collect();
        for mut row in candidates {
            let path = PathBuf::from(str_of(&row, "path"));
            if Self::status(&row) != "ready" || is_reparse(&path) {
                continue;
            }
            if recheck && (file_digest(&path, None).ok().flatten().as_deref() != Some(digest.as_str()) || Self::status(&row) != "ready") {
                continue;
            }
            row["state"] = json!("ready");
            row["duplicates"] = json!(0);
            row["known"] = json!(self.known(&digest));
            return Some(row);
        }
        None
    }

    /// Indexe un fichier ou un dossier (500 nouveaux fichiers au plus par passage).
    pub fn add(&self, supplied: &str) -> Res<Value> {
        let _busy = self.scan.try_lock().map_err(|_| "Une indexation est déjà en cours.".to_string())?;
        let path = Path::new(supplied);
        if !path.is_absolute() || supplied.starts_with(r"\\.\") || supplied.starts_with(r"\\?\") || supplied.starts_with(r"\??\") {
            return Err("Sélectionne un fichier ou dossier local.".into());
        }
        let candidates: Box<dyn Iterator<Item = PathBuf>> = if path.is_dir() {
            Box::new(walkdir::WalkDir::new(path).follow_links(false).into_iter()
                .filter_entry(|e| !(e.file_type().is_dir() && e.depth() > 0 && is_reparse(e.path())))
                .filter_map(Result::ok).filter(|e| e.file_type().is_file()).map(|e| e.into_path()))
        } else {
            Box::new(std::iter::once(path.to_path_buf()))
        };
        let mut by_path: HashMap<String, Value> = self.entries().values().map(|e| (str_of(e, "path").to_lowercase(), e.clone())).collect();
        let (mut indexed, mut skipped, mut limited) = (0usize, 0usize, false);
        for candidate in candidates {
            if media_type(&candidate).is_empty() || is_reparse(&candidate) {
                continue;
            }
            let outcome = (|| -> Res<Option<Value>> {
                let real = candidate.canonicalize().map_err(|e| e.to_string())?;
                let real = PathBuf::from(display_path(&real));
                let current = real.metadata().map_err(|e| e.to_string())?;
                if !current.is_file() {
                    return Ok(None);
                }
                let key = real.to_string_lossy().to_lowercase();
                let previous = by_path.get(&key).cloned().unwrap_or(json!({}));
                if u64_of(&previous, "modified") == mtime_ns(&current) && u64_of(&previous, "size") == current.len() {
                    return Ok(None);
                }
                let digest = file_digest(&real, None)?.unwrap_or_default();
                let after = real.metadata().map_err(|e| e.to_string())?;
                if after.len() != current.len() || mtime_ns(&after) != mtime_ns(&current) {
                    return Err("modifié pendant la lecture".into());
                }
                let id = Some(str_of(&previous, "id")).filter(|s| !s.is_empty()).map(str::to_string)
                    .unwrap_or_else(|| format!("local-{}", uuid::Uuid::new_v4().simple()));
                let mut row = json!({"id": id, "path": real.to_string_lossy(), "name": real.file_name().map(|n| n.to_string_lossy().to_string()),
                    "size": current.len(), "modified": mtime_ns(&current), "sha256": digest});
                if let (Some(dst), Value::Object(src)) = (row.as_object_mut(), inspect(&real)?) {
                    dst.extend(src);
                }
                by_path.insert(key, row.clone());
                Ok(Some(row))
            })();
            match outcome {
                Ok(Some(row)) => {
                    if indexed >= MAX_INDEXED {
                        limited = true;
                        break;
                    }
                    self.entries().insert(str_of(&row, "id").to_string(), row);
                    indexed += 1;
                }
                Ok(None) => {}
                Err(_) => skipped += 1,
            }
        }
        if indexed > 0 {
            let snapshot = self.entries().clone();
            self.save(&snapshot)?;
        }
        Ok(json!({"indexed": indexed, "skipped": skipped, "limited": limited, "files": self.list()}))
    }

    pub fn remove(&self, id: &str) -> Res<Value> {
        let snapshot = {
            let mut entries = self.entries();
            entries.remove(id);
            entries.clone()
        };
        self.save(&snapshot)?;
        Ok(json!({"files": self.list()}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_types() {
        assert_eq!(media_type(Path::new("a.ISO")), "ISO");
        assert_eq!(media_type(Path::new("a.img.xz")), "IMG.XZ");
        assert_eq!(media_type(Path::new("a.txt")), "");
    }

    #[test]
    fn wim_xml() {
        let xml = r#"<WIM><IMAGE INDEX="1"><NAME>Windows 11 Pro</NAME><DISPLAYNAME>Windows 11 Professionnel</DISPLAYNAME>
            <WINDOWS><ARCH>9</ARCH><EDITIONID>Professional</EDITIONID><LANGUAGES><LANGUAGE>fr-FR</LANGUAGE><DEFAULT>fr-FR</DEFAULT></LANGUAGES></WINDOWS></IMAGE>
            <IMAGE INDEX="2"><NAME>Famille &amp; co</NAME></IMAGE></WIM>"#;
        let images = parse_wim_xml(xml);
        assert_eq!(images.len(), 2);
        assert_eq!(images[0]["edition"], "Professional");
        assert_eq!(images[0]["architecture"], "x64");
        assert_eq!(images[0]["languages"], json!(["fr-FR"]));
        assert_eq!(images[1]["edition"], "Famille & co");
        assert!(parse_wim_xml("<!DOCTYPE x [<!ENTITY a 'b'>]><IMAGE INDEX=\"1\"></IMAGE>").is_empty());
    }

    /// ISO factice : descripteur primaire + El Torito (BIOS + UEFI) + SOURCES/INSTALL.WIM.
    #[test]
    fn fake_iso() {
        let mut iso = vec![0u8; 2048 * 40];
        let sector = |n: usize| n * 2048;
        let record = |lba: u32, len: u32, name: &[u8], dir: bool| {
            let mut r = vec![0u8; 33 + name.len() + (1 - name.len() % 2)];
            r[0] = r.len() as u8;
            r[2..6].copy_from_slice(&lba.to_le_bytes());
            r[10..14].copy_from_slice(&len.to_le_bytes());
            r[25] = if dir { 2 } else { 0 };
            r[32] = name.len() as u8;
            r[33..33 + name.len()].copy_from_slice(name);
            r
        };
        // PVD au secteur 16, racine au secteur 20.
        iso[sector(16)] = 1;
        iso[sector(16) + 1..sector(16) + 6].copy_from_slice(b"CD001");
        let root = record(20, 2048, b"\0", true);
        iso[sector(16) + 156..sector(16) + 156 + root.len()].copy_from_slice(&root);
        // Boot record au secteur 17 → catalogue au secteur 25.
        iso[sector(17) + 1..sector(17) + 6].copy_from_slice(b"CD001");
        iso[sector(17) + 7..sector(17) + 30].copy_from_slice(b"EL TORITO SPECIFICATION");
        iso[sector(17) + 71..sector(17) + 75].copy_from_slice(&25u32.to_le_bytes());
        let cat = sector(25);
        iso[cat] = 1;
        iso[cat + 30] = 0x55;
        iso[cat + 31] = 0xaa;
        iso[cat + 32] = 0x88;
        iso[cat + 40..cat + 44].copy_from_slice(&30u32.to_le_bytes());
        iso[cat + 64] = 0x91;
        iso[cat + 65] = 0xef;
        iso[cat + 96] = 0x88;
        iso[cat + 104..cat + 108].copy_from_slice(&31u32.to_le_bytes());
        // Racine : SOURCES (secteur 21) et EFI.
        let mut at = sector(20);
        for r in [record(21, 2048, b"SOURCES", true), record(22, 2048, b"EFI", true)] {
            iso[at..at + r.len()].copy_from_slice(&r);
            at += r.len();
        }
        let wim = record(32, 2048 * 3, b"INSTALL.WIM;1", false);
        iso[sector(21)..sector(21) + wim.len()].copy_from_slice(&wim);
        // En-tête WIM + XML UTF-16 au secteur 32.
        let xml: Vec<u8> = "\u{feff}<WIM><IMAGE INDEX=\"1\"><NAME>Pro</NAME><WINDOWS><ARCH>9</ARCH><EDITIONID>Professional</EDITIONID></WINDOWS></IMAGE></WIM>"
            .encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let w = sector(32);
        iso[w..w + 8].copy_from_slice(b"MSWIM\0\0\0");
        iso[w + 72..w + 80].copy_from_slice(&(xml.len() as u64).to_le_bytes());
        iso[w + 80..w + 88].copy_from_slice(&2048u64.to_le_bytes());
        iso[w + 2048..w + 2048 + xml.len()].copy_from_slice(&xml);

        let path = std::env::temp_dir().join(format!("nitrite-fake-{}.iso", uuid::Uuid::new_v4().simple()));
        std::fs::write(&path, &iso).unwrap();
        let info = inspect(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(info["format"], "ISO 9660");
        assert_eq!(info["boot"], json!(["BIOS", "UEFI"]));
        assert_eq!(info["contents"], json!(["SOURCES/INSTALL.WIM", "Dossier EFI"]));
        assert_eq!(info["windows"][0]["edition"], "Professional");
    }
}
