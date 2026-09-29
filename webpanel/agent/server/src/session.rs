//! Instance unique : la session active (port + jeton) est notee dans
//! %LOCALAPPDATA%\NiTriTe-WebPanel\session.json. Relancer l'agent alors qu'il
//! tourne deja rouvre simplement le navigateur sur la session existante.

use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

impl Session {
    /// Le jeton est dans le fragment (`#`) : le navigateur ne l'envoie jamais
    /// au serveur ni dans l'en-tete Referer ; la page le lit puis l'efface.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/#t={}", self.port, self.token)
    }
}

fn path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("NiTriTe-WebPanel").join("session.json"))
}

pub fn write(s: &Session) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(s) {
        let _ = std::fs::write(p, json);
    }
}

pub fn clear() {
    if let Some(p) = path() {
        let _ = std::fs::remove_file(p);
    }
}

/// Session enregistree ET dont le port repond encore.
pub fn find_running() -> Option<Session> {
    let raw = std::fs::read_to_string(path()?).ok()?;
    let s: Session = serde_json::from_str(&raw).ok()?;
    let alive = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], s.port)), Duration::from_millis(400)).is_ok();
    if alive {
        Some(s)
    } else {
        clear();
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_puts_token_in_fragment() {
        let s = Session { port: 7878, token: "abc".into(), pid: 1 };
        assert_eq!(s.url(), "http://127.0.0.1:7878/#t=abc");
    }
}
