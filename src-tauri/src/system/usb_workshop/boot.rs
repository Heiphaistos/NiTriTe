//! Persistance Linux sur une clé Ventoy (modèles officiels ventoy/backend v5.0).
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : usb_boot.py (partie persistance).

use super::design::{commit_config, load_config};
use super::disks::{self, UsbDisk};
use super::ventoy::download_bounded;
use super::{app_home, ensure_dir, file_digest, file_name, is_reparse, remove_quiet, Res, TempDir, MIB};
use crate::maintenance::commands::execute_system_command;
use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub const BACKEND_URL: &str = "https://github.com/ventoy/backend/releases/download/v5.0/images.zip";
/// Aucune empreinte publiée pour images.zip : valeur figée de TokaISO 7.9, recalculée sur l'asset
/// officiel de la release ventoy/backend v5.0 (identique). Chaque archive interne est en plus
/// vérifiée contre les SHA-256 publiés dans les notes de cette release (BACKENDS).
pub const BACKEND_SHA256: &str = "d9b40a28281762241a2ec5b12bcd46f4f51fd64a0806d23f2df90933914c8437";
pub const BACKEND_RELEASE: &str = "https://github.com/ventoy/backend/releases/tag/v5.0";

/// Profils de persistance : étiquette ext4 attendue par chaque distribution.
pub const PRESETS: &[(&str, &str)] = &[
    ("casper-rw", "Ubuntu Live, Linux Mint, Zorin OS et elementary OS"),
    ("MX-Persist", "MX Linux"),
    ("persistence", "Debian Live, Kali Live et Clonezilla Live"),
    ("vtoycow", "Arch Linux"),
];

/// Taille (Mio) → (descripteur, SHA-256 publiés dans l'ordre de PRESETS).
pub const BACKENDS: &[(u64, &str, [&str; 4])] = &[
    (512, "512MB", [
        "97b1265b0183a1f8cb7d5acffdf6de20a77f1bd3295a7b2e368c3eb5346cbfad",
        "1f943b11bb4b88b0271a59df6bfb0075c4892c903977879f3ca1682c3e5587fe",
        "5eb556d188508cd8fef0739c95a6184b3448fdda551bed697b18900c09f57a64",
        "36091f96eabb9e2adea4e0bfc178d9eb43b605b1fd0cb801c13188e0817d14d5",
    ]),
    (1024, "1GB", [
        "d80e15d4760aa7020643567a573aadc011a2376609102a9a7928c1cc46e76d11",
        "79bbab335a002f36413b8af350af08ff40e7047f9f2a61e08ff8c0248f96377c",
        "13896365dbc092aaf173f99cc0641637c97187541fba72ba5519cc30af000dc8",
        "81615dd761b8cae2a7388d2c9a54c542b8af5e19d29f20357d25250927dc3463",
    ]),
    (2048, "2GB", [
        "f3af593a264f4f87f4e461e409557046de83c79a323908f37df04c524c31352a",
        "ad33722b6d284acd77b2d5606f4166018024aadcf732467fe69f15deb1b21160",
        "76f4708567086cd6b1b643cbbecc43e6e91bd50840b136ad9184f0263c889462",
        "8dc3b6b18dce2456bdecba82011056e7fa5023c089b6ee28dff1a83a5d6123fb",
    ]),
];

const RESERVED: &[&str] = &["con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9"];

fn reserved(part: &str) -> bool {
    RESERVED.contains(&part.split('.').next().unwrap_or("").to_lowercase().as_str())
}

/// Clé Ventoy et racine de sa partition de données.
pub fn select_volume(id: &str) -> Res<(UsbDisk, PathBuf)> {
    let device = disks::pick_usb(id)?;
    let root = device.ventoy_root()?;
    Ok((device, root))
}

fn same_volume(device: &UsbDisk) -> Res<PathBuf> {
    let (current, volume) = select_volume(&device.id)?;
    if current.number != device.number || current.drive != device.drive {
        return Err("La clé a changé. Sélectionne-la de nouveau.".into());
    }
    Ok(volume)
}

/// volume_path : chemin Ventoy « /dossier/fichier » sans `..`, nom réservé ni lien de fichiers.
pub fn volume_path(volume: &Path, relative: &str) -> Res<PathBuf> {
    let invalid = || "Chemin du fichier Ventoy invalide.".to_string();
    if !relative.starts_with('/') || relative.len() > 350 {
        return Err(invalid());
    }
    let parts: Vec<&str> = relative[1..].split('/').collect();
    let bad = |p: &str| {
        p.is_empty() || p == "." || p == ".." || p.ends_with('.') || p.ends_with(' ') || reserved(p)
            || p.chars().any(|c| (c as u32) < 32 || "\\:*?\"<>|".contains(c))
    };
    if parts.iter().any(|p| bad(p)) {
        return Err(invalid());
    }
    let mut path = volume.to_path_buf();
    for part in parts {
        path.push(part);
        if path.exists() && is_reparse(&path) {
            return Err("Le chemin Ventoy contient un lien de fichiers.".into());
        }
    }
    Ok(path)
}

pub fn suggest_label(filename: &str) -> &'static str {
    let name = filename.to_lowercase();
    let starts = |list: &[&str]| list.iter().any(|p| name.starts_with(p));
    if starts(&["ubuntu", "kubuntu", "xubuntu", "lubuntu", "linuxmint", "zorin", "elementary"]) {
        "casper-rw"
    } else if starts(&["mx-", "mxlinux"]) {
        "MX-Persist"
    } else if starts(&["debian-live", "kali", "clonezilla"]) {
        "persistence"
    } else if name.starts_with("archlinux") {
        "vtoycow"
    } else {
        ""
    }
}

/// ventoy_images : ISO de la clé (500 au plus, 20 000 entrées parcourues au plus).
pub fn ventoy_images(volume: &Path) -> Vec<Value> {
    let mut images = vec![];
    let walker = walkdir::WalkDir::new(volume)
        .follow_links(false)
        .sort_by(|a, b| a.file_name().to_ascii_lowercase().cmp(&b.file_name().to_ascii_lowercase()))
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 || !e.file_type().is_dir() {
                return true;
            }
            let name = e.file_name().to_string_lossy().to_lowercase();
            !name.starts_with('.') && !name.starts_with('$') && name != "system volume information" && name != "ventoy"
                && !is_reparse(e.path())
        });
    for (visited, entry) in walker.filter_map(Result::ok).enumerate() {
        if images.len() >= 500 || visited > 20000 {
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_lowercase();
        if !entry.file_type().is_file() || !lower.ends_with(".iso") || lower.ends_with(".vlnk.iso") || is_reparse(entry.path()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(volume) else { continue };
        let relative = format!("/{}", rel.to_string_lossy().replace('\\', "/"));
        images.push(json!({"image": relative, "label": suggest_label(&name)}));
    }
    images
}

/// merge_persistence : ajoute le fichier de persistance à chaque section persistence*.
pub fn merge_persistence(config: &Value, image: &str, backend: &str) -> Res<Value> {
    let invalid = || "La section de persistance Ventoy n’est pas valide.".to_string();
    let mut result = config.clone();
    let object = result.as_object_mut().ok_or_else(invalid)?;
    let mut sections = vec!["persistence".to_string()];
    sections.extend(object.keys().filter(|k| k.starts_with("persistence_")).cloned());
    for section in sections {
        let entries = object.entry(section).or_insert_with(|| json!([]));
        let list = entries.as_array_mut().filter(|l| l.iter().all(Value::is_object)).ok_or_else(invalid)?;
        let matches: Vec<usize> = list
            .iter()
            .enumerate()
            .filter(|(_, row)| row["image"].as_str().unwrap_or("").to_lowercase() == image.to_lowercase())
            .map(|(i, _)| i)
            .collect();
        match matches.as_slice() {
            [] => list.push(json!({"image": image, "backend": backend})),
            [index] => {
                let entry = &mut list[*index];
                let values: Vec<String> = match &entry["backend"] {
                    Value::String(s) => vec![s.clone()],
                    Value::Array(items) => items.iter().map(|v| v.as_str().map(String::from)).collect::<Option<_>>().ok_or_else(invalid)?,
                    _ => return Err(invalid()),
                };
                if !values.iter().any(|v| v == backend) {
                    let mut all = values;
                    all.push(backend.to_string());
                    entry["backend"] = json!(all);
                }
            }
            _ => return Err("La même ISO apparaît plusieurs fois dans la configuration de persistance.".into()),
        }
    }
    Ok(result)
}

fn seven_zip() -> Option<PathBuf> {
    ["ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|root| PathBuf::from(root).join(r"7-Zip\7z.exe"))
        .find(|p| p.is_file())
}

/// Contrôle du superbloc ext4 : signature 0xEF53 et étiquette du profil.
pub fn check_superblock(block: &[u8], label: &str) -> bool {
    block.len() >= 136 && block[56..58] == [0x53, 0xef] && {
        let raw = &block[120..136];
        let end = raw.iter().rposition(|b| *b != 0).map_or(0, |i| i + 1);
        &raw[..end] == label.as_bytes()
    }
}

/// backend_image : télécharge le paquet vérifié, extrait l'archive du profil, vérifie son
/// SHA-256 publié, la décompresse (tar.exe de Windows ou 7-Zip) et contrôle le système de fichiers.
fn backend_image(folder: &Path, label: &str, size: u64) -> Res<PathBuf> {
    let (_, descriptor, hashes) = BACKENDS.iter().find(|(s, ..)| *s == size).ok_or("Taille de persistance inconnue.")?;
    let index = PRESETS.iter().position(|(k, _)| *k == label).ok_or("Profil de persistance inconnu.")?;
    let filename = format!("persistence_ext4_{descriptor}_{label}.dat");
    let cache = ensure_dir(&app_home()?.join("ventoy").join("backend-v5.0"))?;
    let bundle = cache.join("images.zip");
    if !(bundle.is_file() && file_digest(&bundle).is_ok_and(|d| d == BACKEND_SHA256)) {
        remove_quiet(&bundle);
        download_bounded(BACKEND_URL, &bundle, 8 * MIB, BACKEND_SHA256)?;
    }
    let archive = folder.join(format!("{filename}.7z"));
    {
        let unreadable = || "Le paquet de persistance Ventoy est illisible.".to_string();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&bundle).map_err(|_| unreadable())?).map_err(|_| unreadable())?;
        let wanted = file_name(&archive);
        let matches: Vec<usize> = (0..zip.len())
            .filter(|i| zip.by_index(*i).is_ok_and(|e| e.name().is_ok_and(|n| n.rsplit('/').next() == Some(wanted.as_str()))))
            .collect();
        let missing = || "L’image de persistance attendue est absente du paquet Ventoy.".to_string();
        let [only] = matches.as_slice() else { return Err(missing()) };
        let mut entry = zip.by_index(*only).map_err(|_| unreadable())?;
        if entry.size() == 0 || entry.size() > 8 * MIB || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err(missing());
        }
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&archive).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|_| unreadable())?;
    }
    if file_digest(&archive)? != hashes[index] {
        return Err("L’empreinte publiée du modèle de persistance ne correspond pas.".into());
    }
    let tar = disks::system32().join("tar.exe");
    let seven = seven_zip();
    let folder_s = folder.to_string_lossy().to_string();
    let archive_s = archive.to_string_lossy().to_string();
    let run_seven = |seven: &Path| {
        let out = format!("-o{folder_s}");
        execute_system_command(&seven.to_string_lossy(), &["x", "-y", &out, "--", &archive_s], 300)
    };
    let mut ok = if tar.is_file() {
        execute_system_command(&tar.to_string_lossy(), &["-xf", &archive_s, "-C", &folder_s], 300).is_ok_and(|r| r.success)
    } else if let Some(s) = &seven {
        run_seven(s).is_ok_and(|r| r.success)
    } else {
        return Err("La décompression du modèle Ventoy nécessite TAR de Windows ou 7-Zip.".into());
    };
    if !ok && tar.is_file() {
        if let Some(s) = &seven {
            ok = run_seven(s).is_ok_and(|r| r.success);
        }
    }
    let raw = folder.join(&filename);
    if !ok || !raw.is_file() || std::fs::metadata(&raw).map(|m| m.len()).unwrap_or(0) != size * MIB {
        return Err("La décompression du modèle de persistance a échoué.".into());
    }
    let mut block = [0u8; 136];
    let mut file = std::fs::File::open(&raw).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(1024)).and_then(|_| file.read_exact(&mut block)).map_err(|e| e.to_string())?;
    if !check_superblock(&block, label) {
        return Err("Le système de fichiers du modèle Ventoy ne correspond pas au profil.".into());
    }
    Ok(raw)
}

pub fn images(id: &str) -> Res<Value> {
    let (_, volume) = select_volume(id)?;
    let presets: serde_json::Map<String, Value> = PRESETS.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
    let sizes: Vec<u64> = BACKENDS.iter().map(|(s, ..)| *s).collect();
    Ok(json!({"images": ventoy_images(&volume), "presets": presets, "sizes": sizes}))
}

fn safe_stem(path: &Path) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mut out = String::new();
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(40).collect();
    if trimmed.is_empty() { "linux".into() } else { trimmed }
}

/// UsbBoot.persistence : copie vérifiée du modèle puis ventoy.json modifié avec sauvegarde.
pub fn persistence(payload: &Value) -> Res<Value> {
    let _guard = super::MediaGuard::acquire()?;
    let (device, volume) = select_volume(payload["id"].as_str().unwrap_or(""))?;
    let image = payload["image"].as_str().unwrap_or("");
    let path = volume_path(&volume, image)?;
    let lower = file_name(&path).to_lowercase();
    if !lower.ends_with(".iso") || lower.ends_with(".vlnk.iso") || !path.is_file() {
        return Err("Choisis une ISO présente sur la clé Ventoy.".into());
    }
    let label = payload["label"].as_str().unwrap_or("");
    let size = payload["size"].as_u64().unwrap_or(0);
    if !PRESETS.iter().any(|(k, _)| *k == label) || !BACKENDS.iter().any(|(s, ..)| *s == size) {
        return Err("Choisis le type de Linux et la taille de sa persistance.".into());
    }
    let before = load_config(&volume)?;
    let prefix = "/ventoy/tokaiso-persistence";
    let relative = format!("{prefix}/{}-{size}m-{}.dat", safe_stem(&path), &uuid::Uuid::new_v4().simple().to_string()[..12]);
    let after = merge_persistence(&before, image, &relative)?;
    if device.free < size * MIB + MIB {
        return Err("Espace libre insuffisant sur la clé USB.".into());
    }
    let temp = TempDir::new("persistance-")?;
    let raw = backend_image(&temp.0, label, size)?;
    let digest = file_digest(&raw)?;
    same_volume(&device)?;
    if load_config(&volume)? != before || !volume_path(&volume, image)?.is_file() {
        return Err("La configuration ou l’ISO a changé. Recommence l’opération.".into());
    }
    std::fs::create_dir_all(volume_path(&volume, prefix)?).map_err(|e| format!("Écriture sur la clé : {e}"))?;
    let destination = volume_path(&volume, &relative)?;
    let stage = destination.with_extension("part");
    let (mut staged, mut published) = (false, false);
    let outcome = (|| -> Res<String> {
        let mut source = std::fs::File::open(&raw).map_err(|e| e.to_string())?;
        let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(&stage).map_err(|e| format!("Écriture sur la clé : {e}"))?;
        staged = true;
        std::io::copy(&mut source, &mut output).map_err(|e| format!("Écriture sur la clé : {e}"))?;
        output.sync_all().map_err(|e| format!("Écriture sur la clé : {e}"))?;
        drop(output);
        same_volume(&device)?;
        if file_digest(&stage)? != digest {
            return Err("La copie sur la clé ne correspond pas au modèle de persistance.".into());
        }
        std::fs::rename(&stage, &destination).map_err(|e| format!("Écriture sur la clé : {e}"))?;
        published = true;
        commit_config(&volume, &before, &after, "Persistance Ventoy", &[])
    })();
    if outcome.is_err() && same_volume(&device).is_ok() {
        if staged && !published {
            remove_quiet(&stage);
        }
        // Ne jamais retirer un fichier référencé par la configuration enregistrée.
        if published && load_config(&volume).map(|c| !c["persistence"].to_string().contains(&relative)).unwrap_or(false) {
            remove_quiet(&destination);
        }
    }
    let backup = outcome?;
    Ok(json!({"image": image, "backend": relative, "size": size, "label": label, "backup": backup}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_distribution_names() {
        assert_eq!(suggest_label("ubuntu-24.04-desktop-amd64.iso"), "casper-rw");
        assert_eq!(suggest_label("MX-23_x64.iso"), "MX-Persist");
        assert_eq!(suggest_label("kali-linux.iso"), "persistence");
        assert_eq!(suggest_label("archlinux-2024.iso"), "vtoycow");
        assert_eq!(suggest_label("win11.iso"), "");
    }

    #[test]
    fn volume_paths_reject_traversal_and_reserved_names() {
        let root = std::env::temp_dir();
        assert!(volume_path(&root, "/linux/ubuntu.iso").is_ok());
        for bad in ["linux/x.iso", "/../x.iso", "/a//b.iso", "/con.iso", "/a/b.", "/a:b.iso"] {
            assert!(volume_path(&root, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn persistence_merge_appends_and_deduplicates() {
        let config = json!({"persistence": [{"image": "/A.iso", "backend": "/p1.dat"}], "persistence_uefi": []});
        let merged = merge_persistence(&config, "/a.iso", "/p2.dat").expect("fusion");
        assert_eq!(merged["persistence"][0]["backend"], json!(["/p1.dat", "/p2.dat"]));
        assert_eq!(merged["persistence_uefi"][0]["backend"], "/p2.dat");
        let again = merge_persistence(&merged, "/a.iso", "/p2.dat").expect("fusion");
        assert_eq!(again, merged);
        let dup = json!({"persistence": [{"image": "/a.iso"}, {"image": "/A.ISO"}]});
        assert!(merge_persistence(&dup, "/a.iso", "/p.dat").is_err());
        assert!(merge_persistence(&json!({"persistence": {}}), "/a.iso", "/p.dat").is_err());
    }

    #[test]
    fn ext4_superblock_check() {
        let mut block = vec![0u8; 136];
        block[56] = 0x53;
        block[57] = 0xef;
        block[120..129].copy_from_slice(b"casper-rw");
        assert!(check_superblock(&block, "casper-rw"));
        assert!(!check_superblock(&block, "vtoycow"));
        block[56] = 0;
        assert!(!check_superblock(&block, "casper-rw"));
    }

    #[test]
    fn stems_are_sanitized() {
        assert_eq!(safe_stem(Path::new("Ubuntu 24.04 (LTS).iso")), "ubuntu-24-04-lts");
        assert_eq!(safe_stem(Path::new("###.iso")), "linux");
    }

    #[test]
    fn every_backend_has_four_hashes() {
        assert_eq!(PRESETS.len(), 4);
        assert!(BACKENDS.iter().all(|(_, _, h)| h.iter().all(|x| x.len() == 64)));
    }
}
