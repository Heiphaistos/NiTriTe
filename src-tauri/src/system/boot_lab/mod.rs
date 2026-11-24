//! Labo PXE + VM de test.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : pxe.py, vmworkbench.py, media_tools.py::iso_test,
//! diagnostics.py, workshop.py (options de labo, trousses hors ligne), labs.json.

pub mod commands;
pub mod diagnostics;
pub mod hypervisors;
pub mod iso_inspect;
pub mod kit;
pub mod labs;
pub mod pxe;
pub mod pxe_menu;
pub mod pxe_server;
pub mod vm;
pub mod vm_archive;
pub mod vm_control;

use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::maintenance::commands::{execute_system_command, CommandResult};
use crate::utils::paths;

pub(crate) type R<T> = Result<T, String>;

/// Taille maximale d'un fichier sur FAT32 (4 Gio - 1).
pub(crate) const FAT32_MAX: u64 = (1u64 << 32) - 1;
pub(crate) const MIB: u64 = 1024 * 1024;

/// Dossier de données du labo (portable, à côté de l'exe).
pub(crate) fn home() -> PathBuf {
    let dir = paths::config_dir().join("boot_lab");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub(crate) fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Exécute un corps bloquant hors du thread de commande (anti-gel de l'interface).
pub(crate) async fn blocking<T, F>(f: F) -> R<T>
where
    T: Send + 'static,
    F: FnOnce() -> R<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("Tâche interrompue : {e}"))?
}

pub(crate) fn json_read<T: DeserializeOwned + Default>(path: &Path) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

/// Écriture atomique : fichier temporaire puis remplacement.
pub(crate) fn json_write<T: Serialize>(path: &Path, value: &T) -> R<()> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let stage = path.with_extension("tmp");
    std::fs::write(&stage, text).map_err(|e| format!("Écriture impossible : {e}"))?;
    std::fs::rename(&stage, path).map_err(|e| {
        let _ = std::fs::remove_file(&stage);
        format!("Écriture impossible : {e}")
    })
}

/// Vrai pour un lien symbolique, un point de réanalyse (jonction…) ou un chemin illisible.
pub(crate) fn is_reparse(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return true;
                }
            }
            meta.file_type().is_symlink()
        }
        Err(_) => true,
    }
}

/// Fichier local choisi par l'utilisateur : chemin absolu et fichier existant.
pub(crate) fn file_input(value: &str) -> R<PathBuf> {
    let path = PathBuf::from(value);
    if value.is_empty() || !path.is_absolute() || !path.is_file() {
        return Err("Le fichier choisi est introuvable.".into());
    }
    Ok(path)
}

pub(crate) fn lower_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()
}

pub(crate) fn suffix(path: &Path) -> String {
    path.extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

fn strip_verbatim(text: &str) -> String {
    text.trim_start_matches(r"\\?\").to_lowercase()
}

/// Système de fichiers et espace libre du volume qui contient `path`.
pub(crate) fn volume(path: &Path) -> R<(String, u64)> {
    let full = std::fs::canonicalize(path).map_err(|_| "Volume introuvable pour ce dossier.".to_string())?;
    let target = strip_verbatim(&full.to_string_lossy());
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|d| target.starts_with(&strip_verbatim(&d.mount_point().to_string_lossy())))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| (d.file_system().to_string_lossy().to_uppercase(), d.available_space()))
        .ok_or_else(|| "Volume introuvable pour ce dossier.".to_string())
}

/// Contrôle FAT32 + place libre avant toute copie ou extraction.
pub(crate) fn check_space(folder: &Path, sizes: &[u64], reserve: u64, message: &str) -> R<()> {
    let (fs, free) = volume(folder)?;
    if fs == "FAT32" && sizes.iter().any(|s| *s > FAT32_MAX) {
        return Err("Un fichier dépasserait la limite FAT32 de 4 Gio.".into());
    }
    let need = sizes.iter().fold(reserve, |acc, s| acc.saturating_add(*s));
    if free < need {
        return Err(message.into());
    }
    Ok(())
}

pub(crate) fn system32() -> PathBuf {
    PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32")
}

/// Erreur renvoyée par `run` quand le délai est dépassé.
pub(crate) const TIMEOUT: &str = "timeout";

/// Lance un programme avec ses arguments en tableau et un délai maximal.
pub(crate) fn run(exe: &Path, args: &[String], timeout: u64) -> Result<CommandResult, String> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    execute_system_command(&exe.to_string_lossy(), &refs, timeout).map_err(|e| match e {
        crate::error::NiTriTeError::Timeout(_) => TIMEOUT.to_string(),
        other => other.to_string(),
    })
}

/// Script PowerShell dont les paramètres arrivent en JSON base64 dans `$p` (jamais concaténés).
/// Renvoie le JSON écrit par le script (`null` si rien).
pub(crate) fn powershell_json(script: &str, params: serde_json::Value, timeout: u64) -> R<serde_json::Value> {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(params.to_string());
    let full = format!(
        "[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); \
         $p=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded}'))|ConvertFrom-Json; {script}"
    );
    let exe = system32().join(r"WindowsPowerShell\v1.0\powershell.exe");
    let args = ["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", &full].map(String::from);
    let out = run(&exe, &args, timeout)?;
    if !out.success {
        let msg = out.stderr.trim();
        return Err(if msg.is_empty() { "Windows n’a pas pu exécuter la commande.".into() } else { msg.to_string() });
    }
    let text = out.stdout.trim().trim_start_matches('\u{feff}');
    if text.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_str(text).map_err(|_| "Réponse de Windows illisible.".to_string())
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Lit `input` jusqu'au bout en calculant le SHA-256 ; copie vers `output` si fourni.
fn pump(input: &mut impl Read, mut output: Option<&mut std::fs::File>) -> R<String> {
    let mut digest = Sha256::new();
    let mut buf = vec![0u8; MIB as usize];
    loop {
        let n = input.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if let Some(out) = output.as_deref_mut() {
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
        digest.update(&buf[..n]);
    }
    Ok(hex(&digest.finalize()))
}

pub(crate) fn file_digest(path: &Path) -> R<String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    pump(&mut file, None)
}

/// Copie avec SHA-256, contrôle que la source n'a pas changé et relecture de la copie
/// (media_tools.named_copy). `destination` ne doit pas exister.
pub(crate) fn hashed_copy(source: &Path, destination: &Path) -> R<String> {
    let before = std::fs::metadata(source).map_err(|e| e.to_string())?;
    let stage = destination.with_file_name(format!(".nitrite-{}.part", new_id()));
    let result = (|| -> R<String> {
        let mut input = std::fs::File::open(source).map_err(|e| e.to_string())?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)
            .map_err(|e| e.to_string())?;
        let digest = pump(&mut input, Some(&mut output))?;
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
        let after = std::fs::metadata(source).map_err(|e| e.to_string())?;
        if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
            return Err("Le fichier a changé pendant la copie.".into());
        }
        if file_digest(&stage)? != digest {
            return Err("La copie ne correspond pas au fichier d’origine.".into());
        }
        if destination.exists() {
            return Err("Le fichier de destination existe déjà.".into());
        }
        std::fs::rename(&stage, destination).map_err(|e| e.to_string())?;
        Ok(digest)
    })();
    let _ = std::fs::remove_file(&stage);
    result
}
