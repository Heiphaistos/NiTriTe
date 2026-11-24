//! Choix des images locales et ouverture des outils officiels d'écriture (Rufus, balenaEtcher).
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : usb_workshop.py, library.inspect_iso (détection Windows).

use super::disks;
use super::{file_input, file_name, has_suffix, is_reparse, json_read, json_write, workshop_home, Res};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const VENTOY_FILES: &[&str] = &[".iso", ".img", ".wim", ".efi", ".vhd", ".vhdx"];
const ETCHER_FILES: &[&str] = &[".img", ".img.xz", ".img.gz", ".img.bz2", ".iso"];
const TOOLS: &[&str] = &["rufus", "etcher"];

/// boot_media : image locale d'un format géré par l'atelier, jamais un périphérique ni un lien.
pub fn boot_media(value: &str) -> Res<PathBuf> {
    let path = file_input(value)?;
    let name = file_name(&path);
    if is_reparse(&path) || !(has_suffix(&name, VENTOY_FILES) || has_suffix(&name, ETCHER_FILES)) {
        return Err("Ce format ne peut pas être préparé dans l’atelier USB.".into());
    }
    Ok(path)
}

/// Secteur 0 avec signature 55AA et au moins une partition non vide.
pub fn virtual_mbr(boot: &[u8]) -> bool {
    boot.len() == 512
        && boot[510..512] == [0x55, 0xaa]
        && [446usize, 462, 478, 494].iter().any(|&o| {
            boot[o + 4] != 0 && u32::from_le_bytes([boot[o + 12], boot[o + 13], boot[o + 14], boot[o + 15]]) != 0
        })
}

fn read_at(file: &mut std::fs::File, offset: u64, count: usize, limit: u64) -> Vec<u8> {
    if offset + count as u64 > limit {
        return vec![];
    }
    let mut buf = vec![0u8; count];
    match file.seek(SeekFrom::Start(offset)).and_then(|_| file.read_exact(&mut buf)) {
        Ok(()) => buf,
        Err(_) => vec![],
    }
}

/// Entrées d'un répertoire ISO 9660 (nom en majuscules sans « ;1 » → enregistrement).
fn directory(file: &mut std::fs::File, record: &[u8], limit: u64) -> HashMap<Vec<u8>, Vec<u8>> {
    let mut items = HashMap::new();
    if record.len() < 18 {
        return items;
    }
    let start = u64::from(u32::from_le_bytes([record[2], record[3], record[4], record[5]])) * 2048;
    let length = u32::from_le_bytes([record[10], record[11], record[12], record[13]]) as usize;
    if !(1..=1024 * 1024).contains(&length) {
        return items;
    }
    let data = read_at(file, start, length, limit);
    let mut offset = 0;
    while offset < data.len() {
        let count = data[offset] as usize;
        if count == 0 {
            offset = (offset / 2048 + 1) * 2048;
            continue;
        }
        let entry = &data[offset..(offset + count).min(data.len())];
        if entry.len() >= 34 {
            let name_len = entry[32] as usize;
            let raw = &entry[33..(33 + name_len).min(entry.len())];
            let name = raw.split(|b| *b == b';').next().unwrap_or(raw).to_ascii_uppercase();
            items.insert(name, entry.to_vec());
        }
        offset += count;
    }
    items
}

/// ISO d'installation Windows : BOOTMGR à la racine ou SOURCES\BOOT.WIM/INSTALL.*.
pub fn windows_iso(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let limit = file.metadata().map(|m| m.len()).unwrap_or(0);
    let count = (limit.saturating_sub(32768) / 2048).min(16) as usize * 2048;
    let sectors = read_at(&mut file, 32768, count, limit);
    let Some(root) = sectors.chunks(2048).find(|s| s.len() > 190 && &s[1..6] == b"CD001" && s[0] == 1).map(|s| s[156..190].to_vec())
    else {
        return false;
    };
    let entries = directory(&mut file, &root, limit);
    if entries.contains_key(b"BOOTMGR".as_slice()) {
        return true;
    }
    let Some(sources) = entries.get(b"SOURCES".as_slice()).cloned() else { return false };
    let files = directory(&mut file, &sources, limit);
    [b"BOOT.WIM".as_slice(), b"INSTALL.WIM", b"INSTALL.ESD", b"INSTALL.SWM"].iter().any(|n| files.contains_key(*n))
}

/// image_methods : Ventoy, Rufus et/ou balenaEtcher selon le format et le contenu.
pub fn image_methods(path: &Path) -> Vec<&'static str> {
    let name = file_name(path).to_lowercase();
    let iso = name.ends_with(".iso");
    let windows = iso && windows_iso(path);
    let mbr = || {
        let mut boot = vec![0u8; 512];
        std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut boot)).is_ok() && virtual_mbr(&boot)
    };
    let etcher = has_suffix(&name, ETCHER_FILES) && (!iso || (!windows && mbr()));
    let mut methods = vec![];
    if has_suffix(&name, VENTOY_FILES) {
        methods.push("ventoy");
    }
    if name.ends_with(".iso") || name.ends_with(".img") {
        methods.push("rufus");
    }
    if etcher {
        methods.push("etcher");
    }
    methods
}

pub fn images(payload: &Value) -> Res<Value> {
    let requested = payload["paths"].as_array().cloned().unwrap_or_default();
    if requested.len() > 24 {
        return Err("Sélectionne au plus 24 images.".into());
    }
    let mut paths: Vec<PathBuf> = vec![];
    for value in &requested {
        let path = boot_media(value.as_str().ok_or("Choisis une image locale valide.")?)?;
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    let images: Vec<Value> = paths
        .iter()
        .map(|p| {
            json!({"path": p.to_string_lossy(), "name": file_name(p),
                   "size": std::fs::metadata(p).map(|m| m.len()).unwrap_or(0), "methods": image_methods(p)})
        })
        .collect();
    Ok(json!({"images": images}))
}

fn writer_paths(tool: &str) -> Vec<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut paths = vec![];
    let (locations, command): (Vec<(&str, &str)>, &str) = if tool == "rufus" {
        (vec![("ProgramFiles", r"Rufus\rufus.exe"), ("LOCALAPPDATA", r"Programs\Rufus\rufus.exe")], "rufus.exe")
    } else {
        let rel = [r"balenaEtcher\balenaEtcher.exe", r"balenaEtcher\balena-etcher.exe", r"balena-etcher\balenaEtcher.exe",
            r"balena-etcher\balena-etcher.exe", r"Programs\balenaEtcher\balenaEtcher.exe", r"Programs\balena-etcher\balenaEtcher.exe",
            r"Programs\balena-etcher\balena-etcher.exe"];
        if let Some(local) = env("LOCALAPPDATA") {
            for folder in ["balena_etcher", "balenaEtcher", "balena-etcher"] {
                let target = local.join(folder);
                let mut apps: Vec<PathBuf> = std::fs::read_dir(&target)
                    .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| file_name(p).starts_with("app-")).collect())
                    .unwrap_or_default();
                apps.sort();
                apps.reverse();
                for app in apps {
                    paths.push(app.join("balenaEtcher.exe"));
                    paths.push(app.join("balena-etcher.exe"));
                }
                paths.push(target.join("balenaEtcher.exe"));
            }
        }
        let locations = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"].iter().flat_map(|n| rel.iter().map(move |r| (*n, *r))).collect();
        (locations, "balenaEtcher.exe")
    };
    paths.extend(locations.iter().filter_map(|(name, rel)| env(name).map(|root| root.join(rel))));
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path).map(|dir| dir.join(command)).filter(|p| p.is_absolute() && p.is_file()).take(1));
    }
    paths
}

fn config_path() -> Res<PathBuf> {
    Ok(workshop_home()?.join("usb-writers.json"))
}

fn candidates(tool: &str) -> Res<Vec<PathBuf>> {
    let config = json_read(&config_path()?, json!({}));
    let mut choices: Vec<PathBuf> = config[tool].as_str().map(PathBuf::from).into_iter().collect();
    choices.extend(writer_paths(tool));
    Ok(choices.into_iter().filter(|p| p.is_absolute() && p.is_file()).collect())
}

/// Nom d'exécutable attendu pour l'outil (rufus-4.6p.exe, rufus_x64.exe, balenaEtcher.exe…).
pub fn expected_name(tool: &str, name: &str) -> bool {
    let name = name.to_lowercase();
    if tool == "etcher" {
        return name == "balenaetcher.exe" || name == "balena-etcher.exe";
    }
    let Some(middle) = name.strip_prefix("rufus").and_then(|n| n.strip_suffix(".exe")) else { return false };
    let version = |s: &str| s.chars().all(|c| matches!(c, '-' | '_' | '.') || c.is_ascii_digit());
    let arch = ["_x86", "_x64", "_arm64"].iter().find_map(|a| middle.strip_suffix(a));
    match arch {
        Some(rest) => version(rest),
        None => version(middle.strip_suffix('p').unwrap_or(middle)),
    }
}

/// trusted : nom attendu + signature Authenticode valide de l'éditeur officiel.
fn trusted(tool: &str, path: &Path) -> bool {
    if !path.is_absolute() || !path.is_file() || is_reparse(path) || !expected_name(tool, &file_name(path)) {
        return false;
    }
    let script = format!(
        "{}$s=Get-AuthenticodeSignature -LiteralPath $config.path; @{{status=[string]$s.Status;signer=[string]$s.SignerCertificate.Subject}}|ConvertTo-Json -Compress",
        disks::config_prelude(&json!({"path": path.to_string_lossy()}))
    );
    let Ok(check) = disks::powershell_json(&script, 20) else { return false };
    let signer = check["signer"].as_str().unwrap_or("").to_lowercase();
    let names: &[&str] = if tool == "rufus" { &["akeo", "pete batard", "peter batard"] } else { &["balena", "resin.io"] };
    check["status"] == "Valid" && names.iter().any(|n| signer.contains(n))
}

pub fn writers() -> Res<Value> {
    let mut out = json!({});
    for tool in TOOLS {
        out[*tool] = json!(candidates(tool)?.first().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default());
    }
    Ok(json!({"writers": out}))
}

fn valid_tool(payload: &Value) -> Res<&str> {
    payload["tool"].as_str().filter(|t| TOOLS.contains(t)).ok_or_else(|| "Choisis Rufus ou balenaEtcher.".to_string())
}

/// Mémorise l'exécutable indiqué, seulement s'il est signé par l'éditeur officiel.
pub fn set_writer(payload: &Value) -> Res<Value> {
    let tool = valid_tool(payload)?;
    let path = file_input(payload["path"].as_str().unwrap_or(""))?;
    if !trusted(tool, &path) {
        return Err("Choisis l’exécutable officiel signé de cet outil.".into());
    }
    let file = config_path()?;
    let mut config = json_read(&file, json!({}));
    if !config.is_object() {
        config = json!({});
    }
    config[tool] = json!(path.to_string_lossy());
    json_write(&file, &config)?;
    Ok(json!({"path": path.to_string_lossy()}))
}

/// Ouvre l'outil officiel (Rufus reçoit l'image avec `-g -i`) ; l'écriture se confirme dans l'outil.
pub fn open_writer(payload: &Value) -> Res<Value> {
    let tool = valid_tool(payload)?;
    let image = boot_media(payload["path"].as_str().unwrap_or(""))?;
    if !image_methods(&image).contains(&tool) {
        return Err("Cette image ne convient pas au mode choisi.".into());
    }
    let _guard = super::MediaGuard::acquire()?;
    let drive = disks::pick_usb(payload["id"].as_str().unwrap_or(""))?;
    let image_drive = image.to_string_lossy().get(..2).map(str::to_uppercase).unwrap_or_default();
    let letters: Vec<String> = std::iter::once(drive.drive.to_uppercase()).chain(drive.volumes.iter().map(|v| v.drive.to_uppercase())).collect();
    if disks::is_drive_letter(&image_drive) && letters.contains(&image_drive) {
        return Err("Copie l’image sur un autre disque avant de préparer cette clé.".into());
    }
    let writer = candidates(tool)?
        .into_iter()
        .find(|p| trusted(tool, p))
        .ok_or("L’outil officiel est introuvable. Indique son exécutable ou ajoute-le à la file.")?;
    let folder = writer.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let arguments: Vec<String> = if tool == "rufus" { vec!["-g".into(), "-i".into(), image.to_string_lossy().into_owned()] } else { vec![] };
    let config = json!({"writer": writer.to_string_lossy(), "folder": folder, "arguments": arguments});
    // Start-Process passe par ShellExecute : Rufus (manifeste administrateur) obtient sa demande UAC.
    let script = format!(
        "{}$a=@($config.arguments | ForEach-Object {{ '\"'+$_+'\"' }}); if($a.Count){{Start-Process -FilePath $config.writer -WorkingDirectory $config.folder -ArgumentList $a}}else{{Start-Process -FilePath $config.writer -WorkingDirectory $config.folder}}; '{{}}'",
        disks::config_prelude(&config)
    );
    disks::powershell_json(&script, 30).map_err(|_| "Windows n’a pas pu ouvrir cet outil. Relance-le depuis son dossier.".to_string())?;
    Ok(json!({"tool": tool, "image": image.to_string_lossy(), "preselected": tool == "rufus"}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rufus_and_etcher_names() {
        for ok in ["rufus.exe", "rufus-4.6.exe", "rufus-4.6p.exe", "rufus_x64.exe", "rufus-4.6_arm64.exe"] {
            assert!(expected_name("rufus", ok), "{ok}");
        }
        for bad in ["rufusx.exe", "rufus-4.6.msi", "setup.exe", "rufus-4.6pp.exe"] {
            assert!(!expected_name("rufus", bad), "{bad}");
        }
        assert!(expected_name("etcher", "balenaEtcher.exe"));
        assert!(!expected_name("etcher", "etcher.exe"));
    }

    #[test]
    fn mbr_detection() {
        let mut boot = vec![0u8; 512];
        assert!(!virtual_mbr(&boot));
        boot[510] = 0x55;
        boot[511] = 0xaa;
        assert!(!virtual_mbr(&boot));
        boot[446 + 4] = 0x0c;
        boot[446 + 12] = 1;
        assert!(virtual_mbr(&boot));
    }

    fn record(lba: u32, len: u32, name: &[u8]) -> Vec<u8> {
        let mut r = vec![0u8; 33 + name.len() + (name.len() + 1) % 2];
        r[0] = r.len() as u8;
        r[2..6].copy_from_slice(&lba.to_le_bytes());
        r[10..14].copy_from_slice(&len.to_le_bytes());
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    }

    #[test]
    fn windows_iso_detected_from_iso9660_tree() {
        let mut iso = vec![0u8; 2048 * 24];
        let pvd = 16 * 2048;
        iso[pvd] = 1;
        iso[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        let root = record(20, 2048, b"\0");
        iso[pvd + 156..pvd + 156 + 34].copy_from_slice(&root[..34]);
        let sources = record(21, 2048, b"SOURCES");
        iso[20 * 2048..20 * 2048 + sources.len()].copy_from_slice(&sources);
        let wim = record(22, 10, b"INSTALL.WIM;1");
        iso[21 * 2048..21 * 2048 + wim.len()].copy_from_slice(&wim);
        let path = std::env::temp_dir().join(format!("nitrite-usb-{}.iso", uuid::Uuid::new_v4().simple()));
        std::fs::write(&path, &iso).expect("iso");
        assert!(windows_iso(&path));
        assert_eq!(image_methods(&path), vec!["ventoy", "rufus"]);
        super::super::remove_quiet(&path);
    }
}
