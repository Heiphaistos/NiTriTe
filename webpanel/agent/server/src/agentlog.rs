//! Journal de l'agent : %LOCALAPPDATA%\NiTriTe-WebPanel\agent.log.
//! L'agent tourne sans console (sous-systeme Windows « windows ») : sans ce
//! fichier, un demarrage rate ne laisserait aucune trace.

use std::io::Write;
use std::path::PathBuf;

const MAX_BYTES: u64 = 1024 * 1024;

pub fn dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("NiTriTe-WebPanel"))
}

pub fn path() -> Option<PathBuf> {
    dir().map(|d| d.join("agent.log"))
}

/// Ecrit une ligne horodatee (et la recopie sur la sortie standard, utile en
/// developpement). Au-dela de 1 Mo, l'ancien journal devient agent.log.old.
pub fn line(msg: &str) {
    println!("{msg}");
    let Some(p) = path() else { return };
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if std::fs::metadata(&p).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&p, p.with_extension("log.old"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "{} {msg}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}
