//! Atelier clé USB Ventoy.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy).
//!
//! Sources : media_tools.py, usb_media.py, usb_boot.py, usb_health.py,
//! usb_templates.py, usb_workshop.py, ventoy_design.py, ventoy_profiles.py, winops.py.

pub mod boot;
pub mod commands;
pub mod design;
pub mod disks;
pub mod health;
pub mod media;
pub mod profiles;
pub mod templates;
pub mod themes;
pub mod ventoy;
pub mod vm;
pub mod writers;

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub type Res<T> = Result<T, String>;

/// Extensions acceptées sur une clé Ventoy (MEDIA_SUFFIXES).
pub const MEDIA_SUFFIXES: &[&str] = &[".iso", ".img", ".wim", ".efi", ".vhd", ".vhdx"];
pub const FAT32_MAX: u64 = (1u64 << 32) - 1;
pub const MIB: u64 = 1024 * 1024;
pub const GIB: u64 = 1024 * MIB;

// ── Verrou global des opérations USB (engine.media_lock) ──────────────────────

static MEDIA_LOCK: AtomicBool = AtomicBool::new(false);

/// Garde du verrou USB : relâché automatiquement à la sortie du bloc.
pub struct MediaGuard;

impl MediaGuard {
    pub fn acquire() -> Res<MediaGuard> {
        MEDIA_LOCK
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| MediaGuard)
            .map_err(|_| "Une opération USB est déjà en cours.".to_string())
    }
}

impl Drop for MediaGuard {
    fn drop(&mut self) {
        MEDIA_LOCK.store(false, Ordering::Release);
    }
}

/// Exécute un corps bloquant hors du thread de commande (règle anti-gel).
pub async fn blocking<T, F>(f: F) -> Res<T>
where
    T: Send + 'static,
    F: FnOnce() -> Res<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("Tâche interrompue : {e}"))?
}

// ── Dossiers ──────────────────────────────────────────────────────────────────

/// `%LOCALAPPDATA%\NiTriTe` (créé au besoin).
pub fn app_home() -> Res<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(dirs::data_local_dir)
        .ok_or("Dossier LOCALAPPDATA introuvable.")?;
    ensure_dir(&base.join("NiTriTe"))
}

/// Dossier de travail de l'atelier (`%LOCALAPPDATA%\NiTriTe\usb-workshop`).
pub fn workshop_home() -> Res<PathBuf> {
    ensure_dir(&app_home()?.join("usb-workshop"))
}

/// Crée un dossier et refuse une redirection (jonction, lien).
pub fn ensure_dir(path: &Path) -> Res<PathBuf> {
    std::fs::create_dir_all(path).map_err(|e| format!("Création de {} : {e}", path.display()))?;
    if is_reparse(path) {
        return Err(format!("Le dossier {} est redirigé.", path.display()));
    }
    Ok(path.to_path_buf())
}

/// Dossier temporaire unique dans le dossier de l'atelier, supprimé au Drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(prefix: &str) -> Res<TempDir> {
        let path = workshop_home()?.join(format!("{prefix}{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir(&path).map_err(|e| format!("Dossier temporaire : {e}"))?;
        Ok(TempDir(path))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            tracing::warn!("usb_workshop: nettoyage de {} impossible : {e}", self.0.display());
        }
    }
}

// ── Fichiers ──────────────────────────────────────────────────────────────────

/// Point d'analyse (jonction, lien symbolique) ou métadonnées illisibles.
pub fn is_reparse(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                meta.file_attributes() & 0x400 != 0 || meta.file_type().is_symlink()
            }
            #[cfg(not(windows))]
            {
                meta.file_type().is_symlink()
            }
        }
        Err(_) => true,
    }
}

/// Fichier local absolu existant (catalog_store.file_input), jamais un chemin de périphérique.
pub fn file_input(value: &str) -> Res<PathBuf> {
    if value.starts_with(r"\\.\") || value.starts_with(r"\\?\") || value.starts_with(r"\??\") {
        return Err("Choisis une image locale valide.".into());
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() || !path.is_file() {
        return Err("Le fichier choisi est introuvable.".into());
    }
    Ok(path)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_bytes(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

pub fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// SHA-256 d'un fichier par blocs de 1 Mio (workshop.file_digest).
pub fn file_digest(path: &Path) -> Res<String> {
    let err = |e: std::io::Error| format!("Lecture de {} : {e}", file_name(path));
    let mut file = std::fs::File::open(path).map_err(err)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; MIB as usize];
    loop {
        let n = file.read(&mut buf).map_err(err)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// (taille, date de modification en ns) : détecte un fichier modifié pendant un traitement.
pub fn stamp(path: &Path) -> Res<(u64, u128)> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Fichier illisible : {e}"))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Ok((meta.len(), mtime))
}

/// Empreinte stable : refuse un fichier modifié pendant la lecture.
pub fn stable_file_hash(path: &Path) -> Res<String> {
    let before = stamp(path)?;
    let digest = file_digest(path).map_err(|_| "Impossible de lire un des fichiers du média.".to_string())?;
    if stamp(path)? != before {
        return Err("Le fichier a changé pendant la vérification.".into());
    }
    Ok(digest)
}

/// Lecture JSON tolérante (json_read) : valeur par défaut si absent ou invalide.
pub fn json_read(path: &Path, default: Value) -> Value {
    std::fs::read(path)
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or(default)
}

/// Écriture JSON atomique (fichier temporaire puis renommage).
pub fn json_write(path: &Path, value: &Value) -> Res<()> {
    let raw = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let stage = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    std::fs::write(&stage, raw).map_err(|e| format!("Écriture de {} : {e}", path.display()))?;
    std::fs::rename(&stage, path).map_err(|e| {
        remove_quiet(&stage);
        format!("Écriture de {} : {e}", path.display())
    })
}

/// Suppression d'un fichier de travail (absent = déjà fait), échec journalisé.
pub fn remove_quiet(path: &Path) {
    if let Err(e) = std::fs::remove_file(path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("usb_workshop: suppression de {} impossible : {e}", path.display());
        }
    }
}

/// Une extension parmi `suffixes` (insensible à la casse).
pub fn has_suffix(name: &str, suffixes: &[&str]) -> bool {
    let lower = name.to_lowercase();
    suffixes.iter().any(|s| lower.ends_with(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_guard_is_exclusive() {
        let first = MediaGuard::acquire().expect("premier verrou");
        assert!(MediaGuard::acquire().is_err());
        drop(first);
        assert!(MediaGuard::acquire().is_ok());
    }

    #[test]
    fn digest_of_known_vector() {
        assert_eq!(sha256_bytes(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn file_input_rejects_device_paths() {
        assert!(file_input(r"\\.\PhysicalDrive0").is_err());
        assert!(file_input("relatif.iso").is_err());
    }

    #[test]
    fn suffix_is_case_insensitive() {
        assert!(has_suffix("Ubuntu.ISO", MEDIA_SUFFIXES));
        assert!(!has_suffix("setup.exe", MEDIA_SUFFIXES));
    }
}
