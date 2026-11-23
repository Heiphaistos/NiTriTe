//! Thèmes du menu Ventoy : galerie, thèmes inclus, import, aperçu, plan et application.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : ventoy_design.py (partie thèmes), usb79_themes.js.

use super::design::{
    apply_theme_settings, changes, commit_config, config_bytes, exists, file_bytes, list_backups, load_config,
    menu_inventory, menu_plan, parse_config, put_if_changed, read_backup, relative, restore_values, safe_path,
    sha256_hex_list, stable_json, volume_for, ASSETS, CONFIG_LIMIT, LIMIT,
};
use super::disks::UsbDisk;
use super::{blocking, has_suffix, is_reparse, sha256_bytes, workshop_home, MediaGuard, Res, TempDir, MEDIA_SUFFIXES, MIB};
use base64::Engine;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

pub type Files = BTreeMap<String, Vec<u8>>;

const POP_PAGE: &str = "https://github.com/odiegoduarte/ventoy-themes/releases/tag/0.10";
const POP_ARCHIVE: &str = "https://github.com/odiegoduarte/ventoy-themes/releases/download/0.10/ventoy.zip";
/// Aucune empreinte publiée en amont : valeur TokaISO, confirmée en hachant l'asset de la release 0.10.
const POP_SHA256: &str = "b93dfe7dac590d4221626abc434ceccb039d959bead37a36df9d7301d8128a8a";

/// (identifiant, nom, fond, texte, accent)
pub const STYLES: &[(&str, &str, &str, &str, &str)] = &[
    ("citrus", "Citrus", "#173032", "#fff8dc", "#e5ef73"),
    ("retro", "Terminal rétro", "#151128", "#fde9eb", "#ef85bf"),
    ("clair", "Clair", "#e9edf5", "#223047", "#056884"),
    ("contraste", "Contraste élevé", "#101010", "#ffffff", "#ffdb00"),
    ("aurore", "Aurore", "#18213a", "#f4f3ff", "#d2a4ff"),
    ("glacier", "Glacier", "#102b49", "#e6f8ff", "#78edf7"),
    ("foret", "Forêt boréale", "#172d22", "#f3faeb", "#a1e6a1"),
    ("sable", "Sable chaud", "#fff0d6", "#463323", "#854513"),
];
/// (identifiant, taille d'icône, hauteur, espacement, espace icône)
pub const LAYOUTS: &[(&str, u32, u32, u32, u32)] = &[("icons", 24, 38, 4, 7), ("compact", 14, 25, 1, 5), ("text", 0, 28, 2, 0)];
pub const VIDEO_MODES: &[(&str, &str)] = &[("4:3", "1024x768,800x600,auto"), ("16:9", "1920x1080,1280x720,1024x768,800x600,auto")];
const MEDIA_ICONS: &[(&str, &str)] = &[
    ("vtoyiso", "disc"), ("vtoywim", "chip"), ("vtoyimg", "drive"), ("vtoyefi", "chip"),
    ("vtoyvhd", "drive"), ("vtoyvtoy", "drive"), ("vtoydir", "folder"), ("vtoyret", "back"),
];

fn icon_pixels(kind: &str) -> [&'static str; 8] {
    match kind {
        "disc" => ["..####..", ".######.", "###..###", "##....##", "##....##", "###..###", ".######.", "..####.."],
        "chip" => ["..#..#..", ".######.", "##....##", ".#.##.#.", ".#.##.#.", "##....##", ".######.", "..#..#.."],
        "drive" => ["........", ".######.", "##....##", "#......#", "########", "#.#....#", ".######.", "........"],
        "folder" => ["........", ".###....", ".######.", "########", "########", "########", ".######.", "........"],
        _ => ["........", "....#...", "...##...", "..####..", ".##..##.", "##....##", "......##", "........"],
    }
}

fn style(id: &str) -> Option<&'static (&'static str, &'static str, &'static str, &'static str, &'static str)> {
    STYLES.iter().find(|s| s.0 == id)
}

fn layout(id: &str) -> Option<&'static (&'static str, u32, u32, u32, u32)> {
    LAYOUTS.iter().find(|l| l.0 == id)
}

fn video_mode(ratio: &str) -> Option<&'static str> {
    VIDEO_MODES.iter().find(|m| m.0 == ratio).map(|m| m.1)
}

fn rgb(color: &str) -> [u8; 3] {
    let v = u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0);
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

mod png;
pub use png::crc32;
use png::png;

fn backdrop(style_id: &str, ratio: &str) -> Vec<u8> {
    let (width, height) = if ratio == "4:3" { (640u32, 480u32) } else { (640, 360) };
    let Some(&(_, _, bg, _, accent)) = style(style_id) else { return vec![] };
    let (base, highlight) = (rgb(bg), rgb(accent));
    let mut data = Vec::with_capacity((height * (width * 3 + 1)) as usize);
    for y in 0..height {
        data.push(0);
        for x in 0..width {
            let stripe = (width - 30..width - 23).contains(&x) || (height - 28..height - 23).contains(&y);
            let mut glow = if style_id != "contraste" {
                ((x as f64 / width as f64 + y as f64 / height as f64 - 1.0) * 0.35).max(0.0)
            } else {
                0.0
            };
            if style_id == "retro" && y % 5 == 0 {
                glow += 0.07;
            }
            if stripe {
                data.extend_from_slice(&highlight);
            } else {
                for c in 0..3 {
                    let (b, a) = (base[c] as f64, highlight[c] as f64);
                    data.push((b + (a - b) * glow) as u8);
                }
            }
        }
    }
    png(width, height, 2, &data)
}

pub fn icon_png(kind: &str, size: u32, accent: &str) -> Vec<u8> {
    let pixels = icon_pixels(kind);
    let colored = rgb(accent);
    let mut data = vec![];
    for y in 0..size {
        data.push(0);
        let row = pixels[(y * 8 / size) as usize].as_bytes();
        for x in 0..size {
            if row[(x * 8 / size) as usize] == b'#' {
                data.extend_from_slice(&colored);
                data.push(0xff);
            } else {
                data.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    png(size, size, 6, &data)
}

pub fn builtin_theme(style_id: &str, ratio: &str, layout_id: &str) -> Res<(Files, String)> {
    let &(_, _, bg, fg, accent) = style(style_id).ok_or("Thème Ventoy introuvable.")?;
    let &(_, icon, height, spacing, icon_space) = layout(layout_id).ok_or("Choisis une présentation du menu Ventoy.")?;
    let theme = format!(
        r#"desktop-image: "background.png"
desktop-image-scale-method: "stretch"
desktop-color: "{bg}"
title-text: " "
message-color: "{fg}"
+ label {{
  left = 9%
  top = 10%
  width = 82%
  height = 32
  text = "NiTriTe"
  color = "{accent}"
  align = "left"
}}
+ boot_menu {{
  left = 9%
  top = 23%
  width = 82%
  height = 56%
  item_color = "{fg}"
  selected_item_color = "{accent}"
  icon_width = {icon}
  icon_height = {icon}
  item_height = {height}
  item_spacing = {spacing}
  item_padding = 2
  item_icon_space = {icon_space}
}}
+ label {{
  left = 9%
  top = 86%
  width = 82%
  height = 25
  text = "@VTOY_HOTKEY_TIP@"
  color = "{fg}"
  align = "left"
}}
"#
    );
    let mut files = Files::new();
    files.insert("theme.txt".into(), theme.into_bytes());
    files.insert("background.png".into(), backdrop(style_id, ratio));
    if icon > 0 {
        for (name, kind) in MEDIA_ICONS {
            files.insert(format!("icons/{name}.png"), icon_png(kind, icon, accent));
        }
    }
    Ok((files, "theme.txt".into()))
}

// ── Import de thèmes ──────────────────────────────────────────────────────────

fn suffix(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    match base.rfind('.') {
        Some(i) if i > 0 => base[i..].to_lowercase(),
        _ => String::new(),
    }
}

fn add_asset(files: &mut Files, name: &str, data: Vec<u8>, total: usize) -> Res<usize> {
    let path = relative(name)?;
    if files.len() >= 512 || data.len() > 8 * MIB as usize || total + data.len() > LIMIT {
        return Err("Le thème dépasse la taille autorisée.".into());
    }
    if files.keys().any(|k| k.to_lowercase() == path.to_lowercase()) {
        return Err("Le thème contient deux fichiers portant le même nom.".into());
    }
    let bad = "Une image ou une police du thème est invalide.";
    let header: Option<&[u8]> = match suffix(&path).as_str() {
        ".png" => Some(b"\x89PNG\r\n\x1a\n"),
        ".jpg" | ".jpeg" => Some(b"\xff\xd8"),
        ".bmp" => Some(b"BM"),
        ".pf2" => Some(b"FILE"),
        _ => None,
    };
    if header.is_some_and(|h| !data.starts_with(h)) {
        return Err(bad.into());
    }
    if suffix(&path) == ".tga" && (data.len() < 18 || data[1] > 1 || ![1, 2, 3, 9, 10, 11].contains(&data[2])) {
        return Err(bad.into());
    }
    let size = data.len();
    files.insert(path, data);
    Ok(total + size)
}

/// Lit un dossier ou un ZIP de thème ; renvoie les fichiers et le style de chaque theme.txt.
pub fn import_bundle(path: &Path) -> Res<(Files, Map<String, Value>)> {
    let mut files = Files::new();
    let mut total = 0;
    let too_many = "L’archive du thème contient trop de fichiers.";
    if path.is_dir() {
        if is_reparse(path) {
            return Err("Un dossier ou un fichier du thème est redirigé.".into());
        }
        let (mut dirs, mut seen) = (0, 0);
        let walker = walkdir::WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !e.file_type().is_dir() || !is_reparse(e.path()));
        for entry in walker {
            let entry = entry.map_err(|e| format!("Lecture du thème : {e}"))?;
            if entry.file_type().is_dir() {
                dirs += 1;
                if dirs > 260 {
                    return Err(too_many.into());
                }
                continue;
            }
            seen += 1;
            if seen > 1200 {
                return Err(too_many.into());
            }
            if !has_suffix(&entry.file_name().to_string_lossy(), ASSETS) {
                continue;
            }
            let rel = entry.path().strip_prefix(path).map_err(|_| "Chemin Ventoy invalide.")?;
            let rel: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            total = add_asset(&mut files, &rel.join("/"), file_bytes(entry.path(), 8 * MIB as usize)?, total)?;
        }
    } else {
        let unreadable = || "La clé de lecture du ZIP ou son format est invalide.".to_string();
        let file = std::fs::File::open(path).map_err(|_| unreadable())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|_| unreadable())?;
        if archive.len() > 1200 {
            return Err(too_many.into());
        }
        for index in 0..archive.len() {
            let entry = archive.by_index(index).map_err(|_| unreadable())?;
            let raw_name = entry.name().map_err(|_| unreadable())?.to_string();
            let name = relative(raw_name.trim_end_matches('/'))?;
            let link = entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000);
            if link || entry.external_attributes() & 0x400 != 0 {
                return Err("Un dossier ou un fichier du thème est redirigé.".into());
            }
            if entry.is_dir() || !has_suffix(&name, ASSETS) {
                continue;
            }
            if entry.size() > 8 * MIB || total as u64 + entry.size() > LIMIT as u64 {
                return Err("Le thème dépasse la taille autorisée.".into());
            }
            let mut data = vec![];
            entry.take(8 * MIB + 1).read_to_end(&mut data).map_err(|_| unreadable())?;
            total = add_asset(&mut files, &name, data, total)?;
        }
    }
    let themes: Vec<String> = files
        .keys()
        .filter(|n| {
            let base = n.rsplit('/').next().unwrap_or("").to_lowercase();
            base == "theme.txt" || base == "theme_legacy.txt"
        })
        .cloned()
        .collect();
    if themes.is_empty() || themes.len() > 12 {
        return Err("Le thème doit contenir un à douze fichiers theme.txt.".into());
    }
    let mut styles = Map::new();
    for name in themes {
        let value = read_style(&files, &name)?;
        styles.insert(name, value);
    }
    Ok((files, styles))
}

/// Correspondance fnmatch avec `*` et `?`.
// ponytail: les classes `[...]` de fnmatch ne sont pas gérées (absentes des thèmes GRUB courants).
fn glob(text: &[char], pattern: &[char]) -> bool {
    match (pattern.first(), text.first()) {
        (None, _) => text.is_empty(),
        (Some('*'), _) => glob(text, &pattern[1..]) || (!text.is_empty() && glob(&text[1..], pattern)),
        (Some('?'), Some(_)) => glob(&text[1..], &pattern[1..]),
        (Some(p), Some(t)) if p == t => glob(&text[1..], &pattern[1..]),
        _ => false,
    }
}

fn normpath(path: &str) -> String {
    let mut parts: Vec<&str> = vec![];
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p| *p != "..") => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() { ".".into() } else { parts.join("/") }
}

fn color_value(value: &str) -> bool {
    let inner = match value.chars().next() {
        Some(q @ ('"' | '\'')) if value.len() >= 2 && value.ends_with(q) => &value[1..value.len() - 1],
        _ => value,
    };
    inner.len() == 7 && inner.starts_with('#') && inner[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Valide un theme.txt (menu GRUB présent, éléments référencés dans le thème) et lit ses couleurs.
pub fn read_style(files: &Files, name: &str) -> Res<Value> {
    let raw = files.get(name).ok_or("Le fichier theme.txt du thème est introuvable.")?;
    if raw.len() > 128 * 1024 {
        return Err("Le fichier theme.txt est trop volumineux.".into());
    }
    let raw = raw.strip_prefix(b"\xef\xbb\xbf").unwrap_or(raw);
    let original = String::from_utf8_lossy(raw);
    let text: String = original.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join("\n");
    let menu = regex::Regex::new(r"\+\s*boot_menu\s*\{").map_err(|e| e.to_string())?;
    if !menu.is_match(&text) || text.contains('\0') {
        return Err("Le fichier theme.txt ne contient pas de menu GRUB valide.".into());
    }
    let property = regex::Regex::new(r#"\b([\w-]+)\s*[:=]\s*(?:"([^"\r\n]*)"|'([^'\r\n]*)')"#).map_err(|e| e.to_string())?;
    let base = name.rsplit_once('/').map(|(b, _)| b).unwrap_or("");
    let mut background = String::new();
    let mut colors = Map::new();
    for caps in property.captures_iter(&text) {
        let key = caps.get(1).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
        let value = caps.get(2).or_else(|| caps.get(3)).map(|m| m.as_str()).unwrap_or("");
        if matches!(key.as_str(), "desktop-color" | "item_color" | "selected_item_color") && color_value(value) {
            colors.insert(key.clone(), json!(value));
        }
        let ext = suffix(value);
        if value.is_empty() || value == "*" || ext == ".txt" || !ASSETS.contains(&ext.as_str()) {
            continue;
        }
        if value.starts_with('/') || value.starts_with('\\') || value.contains(':') {
            return Err("Un élément du thème se trouve hors de son dossier.".into());
        }
        let asset = normpath(&format!("{base}/{}", value.replace('\\', "/")));
        if asset.starts_with("../") || asset == ".." {
            return Err("Un élément du thème se trouve hors de son dossier.".into());
        }
        let pattern: Vec<char> = asset.to_lowercase().chars().collect();
        if !files.keys().any(|e| glob(&e.to_lowercase().chars().collect::<Vec<_>>(), &pattern)) {
            return Err("Une image ou une police référencée dans theme.txt est introuvable.".into());
        }
        if key == "desktop-image" && files.contains_key(&asset) {
            background = asset;
        }
    }
    Ok(json!({"background": background, "colors": colors}))
}

// ── Galerie et cache local ────────────────────────────────────────────────────

fn cache() -> Res<PathBuf> {
    Ok(workshop_home()?.join("ventoy-themes"))
}

fn valid_id(id: &str) -> bool {
    (3..=64).contains(&id.len()) && id.bytes().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase() || c == b'-')
}

fn metadata(identity: &str) -> Res<Value> {
    if !valid_id(identity) {
        return Err("Thème Ventoy introuvable.".into());
    }
    let path = safe_path(&cache()?, &format!("{identity}/manifest.json"))?;
    let data = parse_config(&file_bytes(&path, CONFIG_LIMIT)?)?;
    let ok = data.get("name").is_some_and(Value::is_string)
        && data.get("styles").and_then(Value::as_object).is_some_and(|s| !s.is_empty());
    if !ok {
        return Err("Thème Ventoy introuvable.".into());
    }
    Ok(data)
}

pub fn gallery() -> Res<Value> {
    let mut themes: Vec<Value> = STYLES
        .iter()
        .map(|s| json!({"id": s.0, "name": s.1, "author": "NiTriTe", "origin": "Inclus avec NiTriTe (thèmes TokaISO)", "available": true, "builtin": true}))
        .collect();
    let mut pop = json!({"id": "popos", "name": "Ventoy PopOS Style Theme", "author": "Diego Duarte",
                         "origin": "Communauté · dépôt MIT", "source": POP_PAGE, "available": false});
    let folder = cache()?;
    if folder.is_dir() && !is_reparse(&folder) {
        let mut names: Vec<String> = std::fs::read_dir(&folder)
            .map_err(|e| format!("Lecture des thèmes : {e}"))?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        for name in names.into_iter().take(100) {
            let Ok(meta) = metadata(&name) else { continue };
            let variants = meta["styles"].as_object().map_or(0, Map::len);
            if name == "popos" {
                pop["available"] = json!(true);
                pop["variants"] = json!(variants);
            } else {
                themes.push(json!({"id": name, "name": meta["name"], "author": "Importation locale",
                                   "origin": "Dossier ou ZIP local", "available": true, "variants": variants}));
            }
        }
    }
    themes.insert(0, pop);
    Ok(json!({"themes": themes}))
}

fn save(path: &Path, origin: &str) -> Res<Value> {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(u64::MAX);
    if !path.is_absolute() || is_reparse(path) || (!path.is_dir() && (suffix(&path.to_string_lossy()) != ".zip" || size > LIMIT as u64)) {
        return Err("Choisis un dossier ou un ZIP de thème Ventoy.".into());
    }
    let (files, styles) = import_bundle(path)?;
    let mut digest = Sha256::new();
    for (name, value) in &files {
        digest.update(name.as_bytes());
        digest.update([0u8]);
        digest.update(Sha256::digest(value));
    }
    let digest = super::hex(&digest.finalize());
    let identity = if origin == "popos" { "popos".to_string() } else { format!("local-{}", &digest[..26]) };
    let root = super::ensure_dir(&cache()?)?;
    for (name, value) in &files {
        put_if_changed(&root, &format!("{identity}/assets/{name}"), value, false)?;
    }
    let label = if origin == "popos" {
        "Ventoy PopOS Style Theme".to_string()
    } else {
        path.file_stem().map(|s| s.to_string_lossy().chars().take(80).collect()).unwrap_or_default()
    };
    let manifest = format!("{identity}/manifest.json");
    if !exists(&safe_path(&root, &manifest)?) {
        put_if_changed(&root, &manifest, &config_bytes(&json!({"name": label, "styles": styles, "digest": digest}))?, false)?;
    }
    let mut result = gallery()?;
    result["selected"] = json!(identity);
    Ok(result)
}

/// Fichiers du thème, nom du theme.txt retenu et son style.
pub fn theme(identity: &str, ratio: &str, layout_id: &str) -> Res<(Files, String, Value)> {
    if video_mode(ratio).is_none() {
        return Err("Choisis un thème et un format d’écran.".into());
    }
    if layout(layout_id).is_none() {
        return Err("Choisis une présentation du menu Ventoy.".into());
    }
    if style(identity).is_some() {
        let (files, name) = builtin_theme(identity, ratio, layout_id)?;
        let parsed = read_style(&files, &name)?;
        return Ok((files, name, parsed));
    }
    let data = metadata(identity)?;
    let folder = safe_path(&cache()?, &format!("{identity}/assets"))?;
    let (files, _) = import_bundle(&folder)?;
    let variants: Vec<&String> = data["styles"].as_object().map(|s| s.keys().collect()).unwrap_or_default();
    let legacy = regex::Regex::new(r"(?i)legacy|1024|800|4[-_]3").map_err(|e| e.to_string())?;
    let name = variants
        .iter()
        .find(|n| legacy.is_match(n) == (ratio == "4:3"))
        .or(variants.first())
        .map(|n| n.to_string())
        .unwrap_or_default();
    if !files.contains_key(&name) {
        return Err("Le fichier theme.txt du thème est introuvable.".into());
    }
    let parsed = read_style(&files, &name)?;
    Ok((files, name, parsed))
}

fn text<'a>(payload: &'a Value, key: &str, default: &'a str) -> &'a str {
    payload.get(key).and_then(Value::as_str).unwrap_or(default)
}

fn preview(payload: &Value) -> Res<Value> {
    let identity = payload.get("theme").and_then(Value::as_str).ok_or("Choisis un thème et un format d’écran.")?;
    let (files, name, parsed) = theme(identity, text(payload, "ratio", "16:9"), text(payload, "layout", "icons"))?;
    let background = files.get(parsed["background"].as_str().unwrap_or("")).cloned().unwrap_or_default();
    let kind = if background.starts_with(b"\x89PNG\r\n\x1a\n") { "png" } else if background.starts_with(b"\xff\xd8") { "jpeg" } else { "" };
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut icons = Map::new();
    if style(identity).is_some() {
        for (icon, _) in MEDIA_ICONS {
            if let Some(data) = files.get(&format!("icons/{icon}.png")) {
                icons.insert((*icon).into(), json!(format!("data:image/png;base64,{}", b64.encode(data))));
            }
        }
    }
    let image = if !kind.is_empty() && background.len() < 2 * MIB as usize {
        format!("data:image/{kind};base64,{}", b64.encode(&background))
    } else {
        String::new()
    };
    Ok(json!({"colors": parsed["colors"], "variant": name, "image": image, "icons": icons}))
}

/// Icônes des classes de menu personnalisées (menu_class) pour un thème inclus.
fn add_custom_icons(config: &Value, files: &mut Files, style_id: &str, layout_id: &str) {
    let (Some(&(_, size, ..)), Some(&(_, _, _, _, accent))) = (layout(layout_id), style(style_id)) else { return };
    let Some(map) = config.as_object() else { return };
    if size == 0 {
        return;
    }
    let mut known: Vec<String> = files.keys().map(|k| k.to_lowercase()).collect();
    for plugin in super::design::firmware_keys(map, "menu_class") {
        let Some(rules) = map.get(&plugin).and_then(Value::as_array) else { continue };
        for rule in rules {
            let Some(label) = rule.get("class").and_then(Value::as_str) else { continue };
            let valid = (1..=64).contains(&label.len()) && label.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-');
            let filename = format!("{label}.png");
            if !valid || super::design::is_reserved(&filename) {
                continue;
            }
            let name = format!("icons/{filename}");
            if !known.contains(&name.to_lowercase()) && files.len() < 256 {
                let kind = if rule.get("dir").is_some() { "folder" } else { "disc" };
                files.insert(name.clone(), icon_png(kind, size, accent));
                known.push(name.to_lowercase());
            }
        }
    }
}

/// Plan (aperçu + jeton) ou application d'une modification de thème/menu ou d'une restauration.
fn draft(volume: &Path, device: &UsbDisk, payload: &Value, restore: bool, apply: bool) -> Res<Value> {
    let before = load_config(volume)?;
    let mut files = Files::new();
    let mut folder = String::new();
    let (after, moves, title) = if restore {
        let backup = read_backup(volume, text(payload, "backup", ""))?;
        let applied = backup.get("applied").unwrap_or(&before);
        let after = restore_values(Some(&before), backup.get("config"), Some(applied))?.unwrap_or_else(|| json!({}));
        let invalid = "Sauvegarde Ventoy invalide.";
        let mut moves = vec![];
        for row in backup["moves"].as_array().into_iter().flatten().rev() {
            let pair = row.as_array().filter(|p| p.len() == 2).ok_or(invalid)?;
            let source = relative(pair[1].as_str().ok_or(invalid)?)?;
            let destination = relative(pair[0].as_str().ok_or(invalid)?)?;
            if !has_suffix(&source, MEDIA_SUFFIXES) || !has_suffix(&destination, MEDIA_SUFFIXES) {
                return Err(invalid.into());
            }
            if !safe_path(volume, &source)?.is_file() || exists(&safe_path(volume, &destination)?) {
                return Err("Les images ont changé depuis cette sauvegarde.".into());
            }
            moves.push((format!("/{source}"), format!("/{destination}")));
        }
        (after, moves, "Restauration Ventoy")
    } else {
        let edits = payload.get("edits").cloned().unwrap_or_else(|| json!([]));
        let (mut after, moves) = menu_plan(volume, &before, &edits, text(payload, "mode", ""))?;
        let identity = text(payload, "theme", "");
        if !identity.is_empty() {
            let ratio = text(payload, "ratio", "16:9");
            let layout_id = text(payload, "layout", "icons");
            let (theme_files, name, _) = theme(identity, ratio, layout_id)?;
            files = theme_files;
            if style(identity).is_some() {
                add_custom_icons(&after, &mut files, identity, layout_id);
            }
            let revision = sha256_bytes(&stable_json(&sha256_hex_list(&files)));
            folder = format!("ventoy/themes/tokaiso-{identity}-{}-{}", ratio.replace(':', "x"), &revision[..12]);
            let fonts: Vec<Value> = files
                .iter()
                .filter(|(item, data)| item.to_lowercase().ends_with(".pf2") && data.starts_with(b"FILE"))
                .map(|(item, _)| json!(format!("/{folder}/{item}")))
                .collect();
            let mut selection = Map::new();
            selection.insert("file".into(), json!(format!("/{folder}/{name}")));
            selection.insert("gfxmode".into(), json!(video_mode(ratio).unwrap_or("auto")));
            if !fonts.is_empty() {
                selection.insert("fonts".into(), Value::Array(fonts));
            }
            let map = after.as_object_mut().ok_or("Le fichier ventoy.json est invalide.")?;
            apply_theme_settings(map, &selection)?;
        }
        (after, moves, "Thème et menu Ventoy")
    };
    let mut to_move = vec![];
    for (source, destination) in &moves {
        let (size, mtime) = super::stamp(&safe_path(volume, source)?)?;
        to_move.push(json!([source, destination, size, mtime.to_string()]));
    }
    let token = sha256_bytes(&stable_json(&json!([device.id, device.drive, before, after, to_move, sha256_hex_list(&files)])));
    let diff = changes(&before, &after, &moves);
    if !apply {
        return Ok(json!({"changes": diff, "token": token}));
    }
    if text(payload, "token", "") != token {
        return Err("La configuration de la clé a changé. Regénère l’aperçu.".into());
    }
    let confirm_device = || -> Res<()> {
        if volume_for(&device.id)?.0.drive != device.drive {
            return Err("La clé a changé. Sélectionne-la de nouveau.".into());
        }
        Ok(())
    };
    confirm_device()?;
    for (name, value) in &files {
        put_if_changed(volume, &format!("{folder}/{name}"), value, false)?;
    }
    if !files.is_empty() {
        confirm_device()?;
    }
    let backup = commit_config(volume, &before, &after, title, &moves)?;
    Ok(json!({"backup": backup, "changes": diff, "backups": list_backups(volume)?}))
}

// ── Commandes Tauri ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn usb_ws_design_gallery() -> Result<Value, String> {
    blocking(gallery).await
}

#[tauri::command]
pub async fn usb_ws_design_preview(payload: Value) -> Result<Value, String> {
    blocking(move || preview(&payload)).await
}

#[tauri::command]
pub async fn usb_ws_design_download() -> Result<Value, String> {
    blocking(|| {
        if cache()?.join("popos").join("manifest.json").is_file() {
            let mut result = gallery()?;
            result["selected"] = json!("popos");
            return Ok(result);
        }
        let temp = TempDir::new("tokaiso-theme-")?;
        let archive = temp.0.join("popos.zip");
        super::ventoy::download_bounded(POP_ARCHIVE, &archive, 8 * MIB, POP_SHA256)?;
        save(&archive, "popos")
    })
    .await
}

#[tauri::command]
pub async fn usb_ws_design_import(payload: Value) -> Result<Value, String> {
    blocking(move || {
        let source = payload.get("path").and_then(Value::as_str).ok_or("Choisis un dossier ou un ZIP de thème Ventoy.")?;
        save(Path::new(source), "local")
    })
    .await
}

#[tauri::command]
pub async fn usb_ws_design_menu(payload: Value) -> Result<Value, String> {
    blocking(move || {
        let (_, volume) = volume_for(text(&payload, "device", ""))?;
        Ok(json!({"images": menu_inventory(&volume, &load_config(&volume)?), "backups": list_backups(&volume)?}))
    })
    .await
}

#[tauri::command]
pub async fn usb_ws_design_plan(payload: Value) -> Result<Value, String> {
    blocking(move || {
        let (device, volume) = volume_for(text(&payload, "device", ""))?;
        let restore = payload.get("restore").and_then(Value::as_bool).unwrap_or(false);
        draft(&volume, &device, &payload, restore, false)
    })
    .await
}

#[tauri::command]
pub async fn usb_ws_design_apply(payload: Value) -> Result<Value, String> {
    blocking(move || {
        let (device, volume) = volume_for(text(&payload, "device", ""))?;
        let _guard = MediaGuard::acquire()?;
        let restore = payload.get("restore").and_then(Value::as_bool).unwrap_or(false);
        draft(&volume, &device, &payload, restore, true)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_theme_is_valid_for_every_style() {
        for s in STYLES {
            for l in LAYOUTS {
                let (files, name) = builtin_theme(s.0, "16:9", l.0).expect("thème");
                let parsed = read_style(&files, &name).expect("style");
                assert_eq!(parsed["background"], "background.png");
                assert_eq!(parsed["colors"]["desktop-color"], s.2);
            }
        }
    }

    #[test]
    fn read_style_rejects_bad_themes() {
        let mut files = Files::new();
        files.insert("t/theme.txt".into(), b"desktop-image: \"bg.png\"\n".to_vec());
        assert!(read_style(&files, "t/theme.txt").is_err(), "sans boot_menu");
        files.insert("t/theme.txt".into(), b"+ boot_menu {}\ndesktop-image: \"../bg.png\"".to_vec());
        assert!(read_style(&files, "t/theme.txt").is_err(), "hors dossier");
        files.insert("t/theme.txt".into(), b"+ boot_menu {}\nmenu_pixmap_style = \"menu_*.png\"".to_vec());
        assert!(read_style(&files, "t/theme.txt").is_err(), "image absente");
        files.insert("t/menu_c.png".into(), b"\x89PNG\r\n\x1a\n".to_vec());
        assert!(read_style(&files, "t/theme.txt").is_ok());
    }

    #[test]
    fn add_asset_checks_headers() {
        let mut files = Files::new();
        assert!(add_asset(&mut files, "a.png", b"pas un png".to_vec(), 0).is_err());
        assert!(add_asset(&mut files, "a.pf2", b"FILE....".to_vec(), 0).is_ok());
        assert!(add_asset(&mut files, "A.PF2", b"FILE....".to_vec(), 0).is_err(), "doublon");
    }

    #[test]
    fn glob_and_normpath() {
        let chars = |s: &str| s.chars().collect::<Vec<_>>();
        assert!(glob(&chars("t/menu_nw.png"), &chars("t/menu_*.png")));
        assert!(!glob(&chars("t/select.png"), &chars("t/menu_*.png")));
        assert_eq!(normpath("a/./b/../c.png"), "a/c.png");
        assert_eq!(normpath("../x"), "../x");
    }

    #[test]
    fn custom_icons_follow_menu_classes() {
        let config = json!({"menu_class": [{"key": "win", "class": "windows"}, {"dir": "/Linux", "class": "linux"}, {"class": "CON"}]});
        let (mut files, _) = builtin_theme("citrus", "16:9", "icons").expect("thème");
        add_custom_icons(&config, &mut files, "citrus", "icons");
        assert!(files.contains_key("icons/windows.png"));
        assert!(files.contains_key("icons/linux.png"));
        assert!(!files.contains_key("icons/CON.png"));
    }
}
