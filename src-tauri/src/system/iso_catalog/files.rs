//! Port de TokaISO 7.9 (MIT, Cédrick Roy)
//! Nom du fichier reçu (`filename_from`) et kit Office (`office_kit` de TokaISO.py) :
//! uniquement l'outil officiel Microsoft (ODT) et des profils XML générés.

use super::common::Res;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(b) = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Nom de fichier sûr à partir de Content-Disposition ou de l'URL finale (`filename_from`).
pub fn filename_from(disposition: &str, url: &str) -> String {
    let mut name = String::new();
    for part in disposition.split(';').map(str::trim) {
        if let Some(v) = part.strip_prefix("filename*=") {
            name = percent_decode(v.split("''").last().unwrap_or(v).trim_matches('"'));
            break;
        }
        if let Some(v) = part.strip_prefix("filename=") {
            name = v.trim_matches('"').to_string();
        }
    }
    if name.is_empty() {
        let path = reqwest::Url::parse(url).map(|u| u.path().to_string()).unwrap_or_default();
        name = percent_decode(path.rsplit('/').next().unwrap_or(""));
    }
    let cleaned: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || (c as u32) < 32 { '_' } else { c }).collect();
    let mut name = cleaned.trim_matches(|c| c == ' ' || c == '.').to_string();
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && stem.as_bytes()[3].is_ascii_digit() && stem.as_bytes()[3] != b'0');
    if name.is_empty() || reserved {
        name = "NiTriTe-telechargement.bin".into();
    }
    if name.chars().count() > 160 {
        let path = Path::new(&name);
        let stem: String = path.file_stem().map(|s| s.to_string_lossy().chars().take(145).collect()).unwrap_or_default();
        let ext: String = path.extension().map(|e| format!(".{}", e.to_string_lossy().chars().take(14).collect::<String>())).unwrap_or_default();
        name = stem + &ext;
    }
    name
}

/// Kit Office : l'outil officiel Microsoft (ODT) + des profils XML prêts à l'emploi.
pub fn office_kit(target: &Path, installer: &Path, preset: &str) -> Res<()> {
    let retail = preset == "365";
    let group = if retail { "Microsoft365" } else { "OfficeLTSC2024" };
    let channel = if retail { "Current" } else { "PerpetualVL2024" };
    let products: &[(&str, &str)] = if retail { &[("Microsoft365", "O365ProPlusRetail")] } else {
        &[("Office", "ProPlus2024Volume"), ("Visio", "VisioPro2024Volume"), ("Project", "ProjectPro2024Volume")]
    };
    let file = File::create(target).map_err(|e| e.to_string())?;
    let mut kit = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut exe = File::open(installer).map_err(|e| e.to_string())?;
    kit.start_file("ODT-Microsoft.exe", options).map_err(|e| e.to_string())?;
    std::io::copy(&mut exe, &mut kit).map_err(|e| e.to_string())?;
    kit.start_file("LIRE-MOI.txt", options).map_err(|e| e.to_string())?;
    kit.write_all("Extraire ODT-Microsoft.exe dans ce dossier pour obtenir setup.exe.\n\
        Pour chaque produit, ouvrir une invite de commandes dans ce dossier.\n\
        Télécharger : setup.exe /download NomDuProfil.xml\n\
        Installer : setup.exe /configure NomDuProfil.xml\n\
        Choisir uniquement les produits couverts par vos licences Microsoft.\n\
        https://learn.microsoft.com/office/ltsc/2024/deploy\n".as_bytes()).map_err(|e| e.to_string())?;
    for (name, product) in products {
        for language in ["fr-fr", "en-us"] {
            let xml = format!("<?xml version='1.0' encoding='utf-8'?>\n<Configuration><Add OfficeClientEdition=\"64\" Channel=\"{channel}\">\
                <Product ID=\"{product}\"><Language ID=\"{language}\" /></Product></Add></Configuration>");
            kit.start_file(format!("{group}-{name}-{language}.xml"), options).map_err(|e| e.to_string())?;
            kit.write_all(xml.as_bytes()).map_err(|e| e.to_string())?;
        }
    }
    kit.finish().map_err(|e| e.to_string())?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn filenames() {
        assert_eq!(filename_from("attachment; filename=\"a:b.iso\"", "https://e.org/x"), "a_b.iso");
        assert_eq!(filename_from("attachment; filename*=UTF-8''caf%C3%A9.iso", "https://e.org/x"), "café.iso");
        assert_eq!(filename_from("", "https://e.org/dl/debian%2013.iso?x=1"), "debian 13.iso");
        assert_eq!(filename_from("", "https://e.org/"), "NiTriTe-telechargement.bin");
        assert_eq!(filename_from("filename=CON.txt", "https://e.org/"), "NiTriTe-telechargement.bin");
        assert!(filename_from("", &format!("https://e.org/{}.iso", "a".repeat(300))).chars().count() <= 160);
    }

    #[test]
    fn office_kit_contents() {
        let dir = std::env::temp_dir().join(format!("nitrite-kit-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("odt.exe");
        std::fs::write(&exe, b"MZ factice").unwrap();
        let zip_path = dir.join("kit.zip");
        office_kit(&zip_path, &exe, "ltsc").unwrap();
        let archive = zip::ZipArchive::new(File::open(&zip_path).unwrap()).unwrap();
        let names: BTreeSet<String> = archive.file_names().filter_map(Result::ok).map(|n| n.to_string()).collect();
        assert_eq!(names.len(), 8);
        assert!(names.contains("ODT-Microsoft.exe") && names.contains("OfficeLTSC2024-Visio-fr-fr.xml"));
        drop(archive);
        let _ = std::fs::remove_dir_all(dir);
    }
}
