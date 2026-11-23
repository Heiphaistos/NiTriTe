//! Dépaquetage des images de VM (ZIP, 7z, tar.gz/xz, .xz, OVA) avec contrôle des chemins et de la place.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : vmworkbench.py (archive_path, unpack, image_in, check_ova).

use regex::Regex;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::hypervisors::{checked, Tools};
use super::{check_space, lower_name, suffix, R};

pub const ARCHIVES: [&str; 6] = [".zip", ".7z", ".tar.gz", ".tar.xz", ".vdi.xz", ".vmdk.xz"];
const MAX_MEMBERS: usize = 2048;
const BAD: &str = "L’archive VM contient des éléments incompatibles.";
const NO_SPACE: &str = "Espace insuffisant pour préparer la machine virtuelle.";

pub fn raw_suffixes(platform: &str) -> &'static [&'static str] {
    if platform == "virtualbox" { &[".vdi", ".vmdk"] } else { &[".vmdk"] }
}

pub fn is_archive(path: &Path) -> bool {
    let name = lower_name(path);
    ARCHIVES.iter().any(|s| name.ends_with(s))
}

fn reserved(part: &str) -> bool {
    let base = part.split('.').next().unwrap_or("").split(':').next().unwrap_or("").trim_end_matches(' ').to_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base[3..].chars().all(|c| ('1'..='9').contains(&c)))
}

/// Chemin d'un membre d'archive sous `base`, refusé s'il pourrait sortir du dossier ou est invalide sous Windows.
pub fn archive_path(base: &Path, name: &str) -> R<PathBuf> {
    let mut original = name.replace('\\', "/");
    while let Some(rest) = original.strip_prefix("./") {
        original = rest.to_string();
    }
    let trimmed = original.trim_end_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    let bad = trimmed.is_empty()
        || parts.iter().any(|p| {
            p.is_empty()
                || *p == "."
                || *p == ".."
                || p.ends_with('.')
                || p.ends_with(' ')
                || p.chars().any(|c| (c as u32) < 32 || ":*?\"<>|".contains(c))
                || reserved(p)
        });
    if bad {
        return Err("Un chemin de l’archive VM est incompatible.".into());
    }
    Ok(parts.iter().fold(base.to_path_buf(), |acc, p| acc.join(p)))
}

/// Membres d'un tar non compressé (OVA) : (nom, taille) ; refuse liens et types spéciaux.
pub fn tar_members<S: Read + Seek>(stream: &mut S) -> R<Vec<(String, u64)>> {
    let octal = |field: &[u8]| -> R<u64> {
        let text: String = field.iter().take_while(|b| **b != 0).map(|b| *b as char).collect();
        let text = text.trim();
        if text.is_empty() { Ok(0) } else { u64::from_str_radix(text, 8).map_err(|_| BAD.to_string()) }
    };
    let cstr = |field: &[u8]| String::from_utf8_lossy(&field[..field.iter().position(|b| *b == 0).unwrap_or(field.len())]).to_string();
    let mut members = Vec::new();
    let mut long_name: Option<String> = None;
    let mut header = [0u8; 512];
    let mut offset = 0u64;
    loop {
        stream.seek(SeekFrom::Start(offset)).map_err(|_| BAD.to_string())?;
        if stream.read_exact(&mut header).is_err() || header.iter().all(|b| *b == 0) {
            break;
        }
        let size = octal(&header[124..136])?;
        let kind = header[156];
        let data = offset + 512;
        offset = data + size.div_ceil(512) * 512;
        match kind {
            b'L' | b'x' => {
                let mut buf = vec![0u8; size.min(64 * 1024) as usize];
                stream.seek(SeekFrom::Start(data)).map_err(|_| BAD.to_string())?;
                stream.read_exact(&mut buf).map_err(|_| BAD.to_string())?;
                long_name = if kind == b'L' {
                    Some(cstr(&buf))
                } else {
                    String::from_utf8_lossy(&buf)
                        .lines()
                        .find_map(|l| l.split_once(" path=").map(|(_, v)| v.to_string()))
                };
                continue;
            }
            b'g' => continue,
            b'0' | 0 | b'5' => {}
            _ => return Err(BAD.into()),
        }
        let prefix = if &header[257..262] == b"ustar" { cstr(&header[345..500]) } else { String::new() };
        let base = cstr(&header[0..100]);
        let name = long_name.take().unwrap_or(if prefix.is_empty() { base } else { format!("{prefix}/{base}") });
        if kind != b'5' && !name.ends_with('/') {
            if members.len() >= MAX_MEMBERS {
                return Err(BAD.into());
            }
            members.push((name, size));
        }
    }
    Ok(members)
}

/// Liste `bsdtar -tv` : (type, taille, nom).
pub fn parse_tar_listing(listing: &str) -> R<Vec<(char, u64, String)>> {
    static LINE: OnceLock<Option<Regex>> = OnceLock::new();
    let re = LINE
        .get_or_init(|| Regex::new(r"^(\S)\S{9}\s+\d+\s+\S+\s+\S+\s+(\d+)\s+\S+\s+\S+\s+\S+\s(.+)$").ok())
        .as_ref()
        .ok_or_else(|| BAD.to_string())?;
    listing
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let c = re.captures(l.trim_end()).ok_or_else(|| BAD.to_string())?;
            let kind = c[1].chars().next().unwrap_or('?');
            let size = c[2].parse::<u64>().map_err(|_| BAD.to_string())?;
            Ok((kind, size, c[3].to_string()))
        })
        .collect()
}

/// Liste `7z l -slt -ba` : tailles, refus des liens.
pub fn parse_7z_listing(listing: &str, folder: &Path) -> R<Vec<u64>> {
    let mut sizes = Vec::new();
    let normalized = listing.replace("\r\n", "\n");
    for block in normalized.trim().split("\n\n").filter(|b| !b.trim().is_empty()) {
        let fields: HashMap<&str, &str> = block.lines().filter_map(|l| l.split_once(" = ")).collect();
        let attrs = fields.get("Attributes").copied().unwrap_or("");
        let link_attr = attrs.split_whitespace().any(|w| w.len() == 10 && w.starts_with('l'));
        let Some(path) = fields.get("Path") else { return Err(BAD.into()) };
        if fields.contains_key("Hard Link") || fields.contains_key("Symbolic Link") || link_attr {
            return Err(BAD.into());
        }
        archive_path(folder, path)?;
        let size = fields.get("Size").copied().unwrap_or("0");
        if size.is_empty() || !size.bytes().all(|b| b.is_ascii_digit()) || sizes.len() >= MAX_MEMBERS {
            return Err(BAD.into());
        }
        sizes.push(size.parse::<u64>().map_err(|_| BAD.to_string())?);
    }
    Ok(sizes)
}

fn unzip(path: &Path, folder: &Path, reserve: u64) -> R<()> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Archive ZIP illisible : {e}"))?;
    if zip.len() > MAX_MEMBERS {
        return Err(BAD.into());
    }
    let mut plan = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let link = entry.unix_mode().map(|m| m & 0o170000 == 0o120000).unwrap_or(false);
        if link || entry.is_symlink() {
            return Err(BAD.into());
        }
        if !entry.is_dir() {
            let name = entry.name().map_err(|_| BAD.to_string())?.to_string();
            plan.push((i, archive_path(folder, &name)?, entry.size()));
        }
    }
    check_space(folder, &plan.iter().map(|p| p.2).collect::<Vec<_>>(), reserve, NO_SPACE)?;
    for (i, dst, size) in plan {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&dst).map_err(|e| e.to_string())?;
        let written = std::io::copy(&mut entry.take(size + 1), &mut out).map_err(|e| e.to_string())?;
        if written != size {
            return Err(BAD.into());
        }
    }
    Ok(())
}

fn untar(path: &Path, folder: &Path, reserve: u64) -> R<()> {
    let tar = super::system32().join("tar.exe");
    if !tar.is_file() {
        return Err("Décompresse ce fichier, puis choisis le disque ou l’image OVF.".into());
    }
    let source = path.to_string_lossy().to_string();
    let listing = checked(&tar, &["-tvf".into(), source.clone()], 120)?;
    let rows = parse_tar_listing(&listing)?;
    if rows.len() > MAX_MEMBERS || rows.iter().any(|(k, _, _)| *k != '-' && *k != 'd') {
        return Err(BAD.into());
    }
    for (_, _, name) in rows.iter().filter(|r| r.0 == '-') {
        archive_path(folder, name)?;
    }
    let sizes: Vec<u64> = rows.iter().filter(|r| r.0 == '-').map(|r| r.1).collect();
    check_space(folder, &sizes, reserve, NO_SPACE)?;
    checked(&tar, &["-xf".into(), source, "-C".into(), folder.to_string_lossy().to_string()], 600).map(|_| ())
}

fn un7z(seven: &Path, path: &Path, folder: &Path, reserve: u64) -> R<()> {
    let source = path.to_string_lossy().to_string();
    let args = ["l", "-slt", "-ba", "-sccUTF-8", "--", &source].map(String::from);
    let listing = checked(seven, &args, 60)?;
    let sizes = parse_7z_listing(&listing, folder)?;
    check_space(folder, &sizes, reserve, NO_SPACE)?;
    let out = format!("-o{}", folder.to_string_lossy());
    let args = ["x", "-y", "-aos", "-bb0", "-spd", &out, "--", &source].map(String::from);
    checked(seven, &args, 600).map(|_| ())
}

/// Dépaquette `path` dans `folder` (dossier privé) après contrôle des membres et de la place.
pub fn unpack(path: &Path, folder: &Path, tools: &Tools, reserve: u64) -> R<()> {
    let name = lower_name(path);
    if name.ends_with(".zip") {
        unzip(path, folder, reserve)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tar.xz") {
        untar(path, folder, reserve)
    } else {
        // .7z et .vdi.xz / .vmdk.xz : 7-Zip (TokaISO décompresse .xz en Python, ici 7-Zip le fait).
        let seven = tools
            .seven_zip
            .as_ref()
            .ok_or("Décompresse ce fichier avec 7-Zip, puis choisis le disque ou l’image OVF.")?;
        un7z(seven, path, folder, reserve)
    }
}

/// Disque principal ou OVF/OVA unique trouvé dans le dossier d'extraction.
pub fn image_in(folder: &Path, platform: &str) -> R<PathBuf> {
    static PART: OnceLock<Option<Regex>> = OnceLock::new();
    let part = PART.get_or_init(|| Regex::new(r"(?i)-(?:flat|s\d{3}|f\d{3}|delta)\.vmdk$").ok());
    let mut paths: Vec<PathBuf> = walkdir::WalkDir::new(folder)
        .follow_links(false)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    paths.sort();
    let wanted: Vec<&str> = [".ova", ".ovf"].into_iter().chain(raw_suffixes(platform).iter().copied()).collect();
    for want in wanted {
        let candidates: Vec<&PathBuf> = paths
            .iter()
            .filter(|p| suffix(p) == want && !part.as_ref().map(|r| r.is_match(&lower_name(p))).unwrap_or(false))
            .collect();
        match candidates.len() {
            1 => return Ok(candidates[0].clone()),
            0 => continue,
            _ => break,
        }
    }
    Err("Choisis le disque principal ou le fichier OVF extrait de cette archive.".into())
}

/// Une OVA est un tar : membres valides et place suffisante pour l'import.
pub fn check_ova(image: &Path, folder: &Path) -> R<()> {
    if suffix(image) != ".ova" {
        return Ok(());
    }
    let mut file = std::fs::File::open(image).map_err(|e| e.to_string())?;
    let members = tar_members(&mut file)?;
    for (name, _) in &members {
        archive_path(folder, name)?;
    }
    check_space(folder, &members.iter().map(|m| m.1).collect::<Vec<_>>(), 0, NO_SPACE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn chemins_archive() {
        let base = Path::new(r"C:\x");
        assert_eq!(archive_path(base, "./a/b.vmdk").ok(), Some(PathBuf::from(r"C:\x\a\b.vmdk")));
        assert_eq!(archive_path(base, "dir/").ok(), Some(PathBuf::from(r"C:\x\dir")));
        for bad in ["../evil", "a/../../b", "/abs", "C:/x", "a//b", "con.txt", "a/LPT1", "nom.", "nom ", "a\u{1}b", ""] {
            assert!(archive_path(base, bad).is_err(), "{bad}");
        }
        assert!(archive_path(base, "COM10.txt").is_ok());
    }

    fn header(name: &str, size: u64, kind: u8) -> Vec<u8> {
        let mut h = vec![0u8; 512];
        h[..name.len()].copy_from_slice(name.as_bytes());
        let s = format!("{size:011o}\0");
        h[124..136].copy_from_slice(s.as_bytes());
        h[156] = kind;
        h[257..262].copy_from_slice(b"ustar");
        h
    }

    #[test]
    fn ova_tar() {
        let mut data = header("vm.ovf", 3, b'0');
        data.extend([b'a', b'b', b'c'].iter().copied().chain(std::iter::repeat_n(0, 509)));
        data.extend(header("disk.vmdk", 0, b'0'));
        data.extend(vec![0u8; 1024]);
        let members = tar_members(&mut Cursor::new(&data)).expect("tar");
        assert_eq!(members, vec![("vm.ovf".to_string(), 3), ("disk.vmdk".to_string(), 0)]);
        let mut link = header("lien", 0, b'2');
        link.extend(vec![0u8; 1024]);
        assert!(tar_members(&mut Cursor::new(&link)).is_err());
    }

    #[test]
    fn listing_bsdtar() {
        let windows = "drwxrwxrwx  0 0      0           0 nov. 23 15:29 ./\r\n-rw-rw-rw-  0 0      0           3 nov. 23 15:29 ./sous/disk 1.vmdk\r\n";
        let rows = parse_tar_listing(windows).expect("liste Windows");
        assert_eq!(rows[1], ('-', 3, "./sous/disk 1.vmdk".to_string()));
        assert!(archive_path(Path::new(r"C:\x"), &rows[1].2).is_ok());
        let out = "drwxr-xr-x  0 0      0           0 Jan  1  2024 zabbix/\n-rw-r--r--  0 1000   1000  123456 Mar 12 10:20 zabbix/disk 1.vmdk\nlrwxrwxrwx  0 0 0 0 Jan  1  2024 l -> /etc\n";
        let rows = parse_tar_listing(out).expect("liste");
        assert_eq!(rows[0], ('d', 0, "zabbix/".to_string()));
        assert_eq!(rows[1], ('-', 123456, "zabbix/disk 1.vmdk".to_string()));
        assert_eq!(rows[2].0, 'l');
        assert!(parse_tar_listing("n'importe quoi").is_err());
    }

    #[test]
    fn listing_7z() {
        let base = Path::new(r"C:\x");
        let ok = "Path = disk.vdi\r\nSize = 42\r\nAttributes = A\r\n\r\nPath = sous/b.vmdk\r\nSize = 7\r\n";
        assert_eq!(parse_7z_listing(ok, base).ok(), Some(vec![42, 7]));
        assert!(parse_7z_listing("Path = l\nSymbolic Link = /etc\nSize = 1\n", base).is_err());
        assert!(parse_7z_listing("Path = l\nSize = 1\nAttributes = lrwxrwxrwx\n", base).is_err());
        assert!(parse_7z_listing("Path = ../x\nSize = 1\n", base).is_err());
        assert!(parse_7z_listing("Size = 1\n", base).is_err());
    }

    #[test]
    fn choix_image() {
        let dir = std::env::temp_dir().join(format!("nitrite-img-{}", super::super::new_id()));
        std::fs::create_dir_all(dir.join("a")).expect("dossier");
        std::fs::write(dir.join("a/disk.vmdk"), b"x").expect("vmdk");
        std::fs::write(dir.join("a/disk-flat.vmdk"), b"x").expect("flat");
        assert_eq!(image_in(&dir, "vmware").ok(), Some(dir.join("a").join("disk.vmdk")));
        std::fs::write(dir.join("a/vm.ovf"), b"x").expect("ovf");
        assert_eq!(image_in(&dir, "virtualbox").ok(), Some(dir.join("a").join("vm.ovf")));
        std::fs::write(dir.join("b.ovf"), b"x").expect("ovf2");
        assert!(image_in(&dir, "virtualbox").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
