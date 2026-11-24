//! Téléchargement vérifié de Ventoy, installation sur la clé, copie vérifiée des images.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : media_tools.py.

use super::disks::{self, UsbDisk};
use super::{app_home, ensure_dir, file_digest, file_name, hex, remove_quiet, Res, FAT32_MAX, MIB};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const VENTOY_VERSION: &str = "1.1.18";
pub const VENTOY_URL: &str = "https://github.com/ventoy/Ventoy/releases/download/v1.1.18/ventoy-1.1.18-windows.zip";
/// Valeur du `sha256.txt` officiel de la release GitHub ventoy/Ventoy v1.1.18
/// (identique au condensat publié par GitHub pour l'asset et à TokaISO 7.9).
pub const VENTOY_SHA: &str = "082c478e8c432e1ac653899fcf50e3a4077a2851bc6112b5ddd30545962164a9";
const VENTOY_LIMIT: u64 = 70 * MIB;

/// Lance Ventoy2Disk en mode ligne de commande (`VTOYCLI /I|/U /PhyDrive:N [/FS:EXFAT] [/GPT]`)
/// et lit son marqueur `cli_done.txt` (0 = succès, 1 = échec) sous le même jeton élevé.
const VENTOY_ELEVATED: &str = r#"
$mode=if($config.update){'/U'}else{'/I'}
$a=@('VTOYCLI',$mode,('/PhyDrive:'+$device.Number))
if(!$config.update){
  $a+='/FS:EXFAT'
  if($config.gpt){$a+='/GPT'}
}
$status=8
$done=Join-Path $config.folder 'cli_done.txt'
try {
  Start-Process -FilePath $config.binary -WorkingDirectory $config.folder -ArgumentList $a -WindowStyle Hidden -Wait
  for($attempt=0; $attempt -lt 30; $attempt++){
    try {
      $value=[IO.File]::ReadAllText($done).Trim()
      if($value -eq '0' -or $value -eq '1'){ $status=[int]$value; break }
    } catch [IO.IOException] {} catch [UnauthorizedAccessException] {}
    Start-Sleep -Milliseconds 500
  }
} finally {
  foreach($name in @('cli_done.txt','cli_percent.txt','cli_log.txt')){
    Remove-Item -LiteralPath (Join-Path $config.folder $name) -Force -ErrorAction SilentlyContinue
  }
}
exit $status
"#;

fn valid_sha(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// download_bounded : HTTPS uniquement (redirections comprises), taille bornée, SHA-256 obligatoire.
/// Le fichier n'atteint `target` que si l'empreinte correspond.
pub fn download_bounded(url: &str, target: &Path, limit: u64, expected: &str) -> Res<()> {
    if !valid_sha(expected) {
        return Err("Aucune empreinte SHA-256 publiée : téléchargement refusé.".into());
    }
    if !url.starts_with("https://") {
        return Err("Redirection sans HTTPS.".into());
    }
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.url().scheme() != "https" {
            attempt.error("Redirection sans HTTPS.")
        } else if attempt.previous().len() > 8 {
            attempt.stop()
        } else {
            attempt.follow()
        }
    });
    let client = reqwest::blocking::Client::builder()
        .redirect(policy)
        .connect_timeout(Duration::from_secs(22))
        .timeout(Duration::from_secs(1800))
        .user_agent("NiTriTe")
        .build()
        .map_err(|e| format!("Client HTTPS : {e}"))?;
    let mut response = client.get(url).send().map_err(|e| format!("Téléchargement impossible : {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Le serveur a répondu {}.", response.status()));
    }
    let stage = target.with_extension("part");
    remove_quiet(&stage);
    let result = (|| -> Res<()> {
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)
            .map_err(|e| format!("Écriture du téléchargement : {e}"))?;
        let mut hasher = Sha256::new();
        let mut received = 0u64;
        let mut buf = vec![0u8; MIB as usize];
        loop {
            let n = response.read(&mut buf).map_err(|e| format!("Téléchargement interrompu : {e}"))?;
            if n == 0 {
                break;
            }
            received += n as u64;
            if received > limit {
                return Err("Taille du téléchargement inattendue.".into());
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(|e| format!("Écriture du téléchargement : {e}"))?;
        }
        out.sync_all().map_err(|e| e.to_string())?;
        drop(out);
        if received == 0 || !hex(&hasher.finalize()).eq_ignore_ascii_case(expected) {
            return Err("Empreinte SHA-256 différente de celle publiée.".into());
        }
        std::fs::rename(&stage, target).map_err(|e| format!("Enregistrement du téléchargement : {e}"))
    })();
    remove_quiet(&stage);
    result
}

/// Extrait une archive ZIP vérifiée dans `folder` (chemins enclos uniquement, pas de liens).
pub fn extract_zip(archive: &Path, folder: &Path, max_entries: usize) -> Res<()> {
    let file = std::fs::File::open(archive).map_err(|e| format!("Archive illisible : {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "Archive ZIP illisible.".to_string())?;
    if zip.len() > max_entries {
        return Err("L’archive contient trop d’éléments.".into());
    }
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|_| "Archive ZIP illisible.".to_string())?;
        if entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err("L’archive contient des éléments incompatibles.".into());
        }
        let relative = entry.enclosed_name().ok_or("L’archive contient un chemin invalide.")?;
        let destination = folder.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|e| format!("Extraction : {e}"))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("Extraction : {e}"))?;
    }
    Ok(())
}

/// Zip officiel en cache (`%LOCALAPPDATA%\NiTriTe\ventoy\1.1.18\`), vérifié à chaque usage,
/// puis réextrait à neuf : un exécutable modifié sur le disque n'est jamais lancé en administrateur.
pub fn ensure_ventoy() -> Res<PathBuf> {
    let dir = ensure_dir(&app_home()?.join("ventoy").join(VENTOY_VERSION))?;
    let archive = dir.join(format!("ventoy-{VENTOY_VERSION}-windows.zip"));
    if archive.is_file() && file_digest(&archive).is_ok_and(|d| d == VENTOY_SHA) {
        tracing::info!("usb_workshop: archive Ventoy en cache conforme (SHA-256).");
    } else {
        remove_quiet(&archive);
        download_bounded(VENTOY_URL, &archive, VENTOY_LIMIT, VENTOY_SHA)?;
    }
    let folder = dir.join("app");
    if folder.exists() {
        std::fs::remove_dir_all(&folder).map_err(|e| format!("Nettoyage de l’ancienne extraction Ventoy : {e}"))?;
    }
    ensure_dir(&folder)?;
    extract_zip(&archive, &folder, 2048)?;
    if file_digest(&archive)? != VENTOY_SHA {
        return Err("Empreinte SHA-256 différente de celle publiée.".into());
    }
    walkdir::WalkDir::new(&folder)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .find(|e| e.file_type().is_file() && e.file_name().eq_ignore_ascii_case("Ventoy2Disk.exe"))
        .map(|e| e.into_path())
        .ok_or_else(|| "Installateur Ventoy introuvable dans l’archive.".to_string())
}

/// initialize_usb : installe (/I) ou met à jour (/U) Ventoy sur la clé confirmée.
pub fn initialize_usb(
    id: &str,
    confirm: &str,
    gpt: bool,
    update: bool,
    before_write: &dyn Fn() -> Res<()>,
) -> Res<Value> {
    let drive = disks::pick_usb(id)?;
    if drive.ventoy != update || confirm != drive.confirmation() {
        return Err("Confirme la lettre ou le numéro exact de la clé USB.".into());
    }
    let binary = ensure_ventoy()?;
    let folder = binary.parent().ok_or("Installateur Ventoy introuvable dans l’archive.")?.to_path_buf();
    let current = disks::pick_usb(&drive.id)?;
    if current.identity() != drive.identity() {
        return Err("La clé a changé. Sélectionne-la de nouveau.".into());
    }
    before_write()?;
    let mut config = current.identity();
    config["binary"] = json!(binary.to_string_lossy());
    config["folder"] = json!(folder.to_string_lossy());
    config["gpt"] = json!(gpt);
    config["update"] = json!(update);
    let code = disks::elevated_usb(&config, VENTOY_ELEVATED, 240).map_err(|e| {
        if e == disks::TIMEOUT_MSG {
            "Ventoy est peut-être encore en cours. Actualise l’état de la clé avant de relancer une opération.".to_string()
        } else {
            e
        }
    })?;
    match code {
        0 => Ok(json!({"disks": disks::usb_disks(), "prepared": !update, "updated": update})),
        1 => Err("Ventoy a signalé un échec. Actualise l’état de la clé avant de réessayer.".into()),
        _ => Err("Ventoy n’a pas confirmé l’opération. Actualise l’état de la clé.".into()),
    }
}

/// Copie atomique vérifiée (`nom (1).iso` si le nom existe déjà).
pub fn named_copy(source: &Path, folder: &Path) -> Res<Value> {
    let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = source.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
    let name = (0..10000)
        .map(|i| if i == 0 { file_name(source) } else { format!("{stem} ({i}){ext}") })
        .find(|n| !folder.join(n).exists())
        .ok_or("Trop de fichiers portant ce nom.")?;
    let destination = folder.join(&name);
    let stage = folder.join(format!(".tokaiso-{}.part", uuid::Uuid::new_v4().simple()));
    let before = super::stamp(source)?;
    let result = (|| -> Res<Value> {
        let mut input = std::fs::File::open(source).map_err(|e| format!("Lecture de l’image : {e}"))?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)
            .map_err(|e| format!("Écriture sur la clé : {e}"))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; MIB as usize];
        loop {
            let n = input.read(&mut buf).map_err(|e| format!("Lecture de l’image : {e}"))?;
            if n == 0 {
                break;
            }
            output.write_all(&buf[..n]).map_err(|e| format!("Écriture sur la clé : {e}"))?;
            hasher.update(&buf[..n]);
        }
        output.sync_all().map_err(|e| format!("Écriture sur la clé : {e}"))?;
        drop(output);
        if super::stamp(source)? != before {
            return Err("Le fichier a changé pendant la copie.".into());
        }
        let digest = hex(&hasher.finalize());
        if file_digest(&stage)? != digest {
            return Err("La copie ne correspond pas au fichier d’origine.".into());
        }
        if destination.exists() {
            return Err("Un fichier est apparu sur la clé pendant la copie.".into());
        }
        std::fs::rename(&stage, &destination).map_err(|e| format!("Écriture sur la clé : {e}"))?;
        let size = std::fs::metadata(&destination).map(|m| m.len()).unwrap_or(0);
        Ok(json!({"name": name, "size": size, "sha256": digest}))
    })();
    remove_quiet(&stage);
    result
}

/// Contrôles d'espace et de format avant copie (FAT32 limité à 4 Gio par fichier).
pub fn check_capacity(device: &UsbDisk, sizes: &[u64]) -> Res<()> {
    if device.format.eq_ignore_ascii_case("FAT32") && sizes.iter().any(|s| *s > FAT32_MAX) {
        return Err("Un fichier dépasserait la limite FAT32 de 4 Gio.".into());
    }
    if device.free < sizes.iter().sum::<u64>() {
        return Err("Espace libre insuffisant sur la clé USB.".into());
    }
    Ok(())
}

/// copy_to_usb : copie les images dans `X:\TokaISO\` (nom conservé pour rester compatible
/// avec les clés préparées par TokaISO) et écrit un rapport SHA-256.
pub fn copy_to_usb(id: &str, paths: &[PathBuf]) -> Res<Value> {
    let device = disks::pick_usb(id)?;
    let base = device.ventoy_root()?;
    let sizes = paths
        .iter()
        .map(|p| std::fs::metadata(p).map(|m| m.len()).map_err(|e| e.to_string()))
        .collect::<Res<Vec<u64>>>()?;
    check_capacity(&device, &sizes)?;
    let target = base.join("TokaISO");
    if target.exists() && (!target.is_dir() || super::is_reparse(&target)) {
        return Err("Le dossier TokaISO sur la clé est incompatible.".into());
    }
    std::fs::create_dir_all(&target).map_err(|e| format!("Écriture sur la clé : {e}"))?;
    let mut copied = vec![];
    for path in paths {
        disks::unchanged(&device)?;
        copied.push(named_copy(path, &target)?);
    }
    let report = format!("TokaISO-SHA256-{}.txt", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    let lines: String = copied
        .iter()
        .map(|row| format!("{}  {}\n", row["sha256"].as_str().unwrap_or(""), row["name"].as_str().unwrap_or("")))
        .collect();
    std::fs::write(target.join(&report), lines).map_err(|e| format!("Rapport SHA-256 : {e}"))?;
    Ok(json!({"folder": target.to_string_lossy(), "files": copied, "report": report}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_requires_a_published_hash() {
        let target = std::env::temp_dir().join("nitrite-usb-test.bin");
        assert!(download_bounded("https://example.invalid/x", &target, 10, "").is_err());
        assert!(download_bounded("http://example.invalid/x", &target, 10, VENTOY_SHA).is_err());
        assert!(!target.exists());
    }

    #[test]
    fn pinned_hash_is_well_formed() {
        assert!(valid_sha(VENTOY_SHA));
        assert!(VENTOY_URL.contains(VENTOY_VERSION));
    }

    #[test]
    fn named_copy_avoids_collisions_and_verifies() {
        let dir = std::env::temp_dir().join(format!("nitrite-usb-{}", uuid::Uuid::new_v4().simple()));
        let out = dir.join("out");
        std::fs::create_dir_all(&out).expect("dossier de test");
        let src = dir.join("image.iso");
        std::fs::write(&src, b"donnees").expect("fichier de test");
        std::fs::write(out.join("image.iso"), b"autre").expect("fichier de test");
        let row = named_copy(&src, &out).expect("copie");
        assert_eq!(row["name"], "image (1).iso");
        assert_eq!(row["sha256"], super::super::sha256_bytes(b"donnees"));
        let leftovers = std::fs::read_dir(&out).expect("lecture").count();
        assert_eq!(leftovers, 2, "aucun fichier .part ne doit rester");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn zip_extraction_refuses_escaping_paths() {
        let dir = std::env::temp_dir().join(format!("nitrite-usb-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).expect("dossier de test");
        let archive = dir.join("a.zip");
        {
            let file = std::fs::File::create(&archive).expect("zip");
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("../evil.txt", zip::write::SimpleFileOptions::default()).expect("entrée");
            zip.write_all(b"x").expect("écriture");
            zip.finish().expect("fin");
        }
        assert!(extract_zip(&archive, &dir.join("out"), 10).is_err());
        assert!(!dir.join("evil.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
