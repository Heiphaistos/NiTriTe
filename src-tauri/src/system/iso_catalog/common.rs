//! Port de TokaISO 7.9 (MIT, Cédrick Roy)
//! Aides partagées : lecture/écriture JSON atomique, validation des adresses,
//! empreintes SHA-256 et chemins locaux.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub type Res<T> = Result<T, String>;

pub fn json_read(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// Écriture atomique (fichier .tmp puis renommage), comme `json_write` de TokaISO.
pub fn json_write(path: &Path, data: &Value) -> Res<()> {
    let stage = path.with_extension("tmp");
    let text = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    std::fs::write(&stage, text).map_err(|e| format!("Écriture impossible : {e}"))?;
    std::fs::rename(&stage, path).map_err(|e| format!("Écriture impossible : {e}"))
}

/// `label()` de TokaISO : texte nettoyé et tronqué (en caractères).
pub fn label(value: Option<&Value>, limit: usize) -> String {
    let text = match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    };
    text.trim().chars().take(limit).collect()
}

pub fn is_hex64(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Empreinte SHA-256 facultative : vide, ou 64 caractères hexadécimaux.
pub fn checksum(value: Option<&Value>) -> Res<String> {
    let digest = label(value, 200).to_lowercase();
    if !digest.is_empty() && !is_hex64(&digest) {
        return Err("L’empreinte SHA-256 doit contenir 64 caractères hexadécimaux.".into());
    }
    Ok(digest)
}

/// Adresse IP non routable sur Internet (réseau local, boucle, lien local, etc.).
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified()
                || v4.is_broadcast() || v4.is_documentation() || v4.is_multicast()
                || (o[0] == 100 && (64..128).contains(&o[1])) || o[0] == 0
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private_ip(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            v6.is_loopback() || v6.is_unspecified() || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

fn private_host(host: &str) -> bool {
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let lower = bare.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") || lower.ends_with(".local") {
        return true;
    }
    bare.parse::<IpAddr>().map(is_private_ip).unwrap_or(false)
}

/// Validation d'une adresse de téléchargement (port de `address()` durci) :
/// HTTPS uniquement, pas d'identifiants, pas de fragment, pas d'adresse privée.
pub fn address(value: Option<&Value>) -> Res<String> {
    let text = match value {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err("Entre une adresse de téléchargement valide.".into()),
    };
    if text.is_empty() || text.len() > 2048 || text.chars().any(|c| (c as u32) <= 32) {
        return Err("Entre une adresse de téléchargement valide.".into());
    }
    let parsed = reqwest::Url::parse(text).map_err(|_| "Entre une adresse de téléchargement valide.".to_string())?;
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.host_str().is_none() || parsed.fragment().is_some() {
        return Err("L’adresse doit identifier directement le fichier.".into());
    }
    if parsed.scheme() != "https" {
        return Err("Entre une adresse HTTPS.".into());
    }
    if private_host(parsed.host_str().unwrap_or_default()) {
        return Err("Les adresses du réseau local ne sont pas acceptées.".into());
    }
    Ok(text.to_string())
}

/// Vérifie au moment du téléchargement que l'hôte ne se résout pas vers une adresse privée.
pub fn resolves_public(url: &reqwest::Url) -> Res<()> {
    use std::net::ToSocketAddrs;
    let host = url.host_str().ok_or("Adresse sans hôte.")?;
    if private_host(host) {
        return Err("Les adresses du réseau local ne sont pas acceptées.".into());
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs: Vec<_> = (host.trim_start_matches('[').trim_end_matches(']'), port)
        .to_socket_addrs()
        .map_err(|e| format!("Résolution DNS impossible : {e}"))?
        .collect();
    if addrs.is_empty() || addrs.iter().any(|a| is_private_ip(a.ip())) {
        return Err("L’hôte pointe vers une adresse du réseau local.".into());
    }
    Ok(())
}

/// Politique de redirection commune : chaque saut doit rester en HTTPS et public.
pub fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.error("Trop de redirections.");
        }
        let url = attempt.url();
        if url.scheme() != "https" || private_host(url.host_str().unwrap_or_default()) {
            return attempt.error("Redirection refusée (HTTPS public requis).");
        }
        attempt.follow()
    })
}

/// Fichier local choisi par l'utilisateur : chemin absolu, fichier existant, canonicalisé.
pub fn file_input(value: Option<&Value>) -> Res<PathBuf> {
    let text = value.and_then(Value::as_str).ok_or("Choisis un fichier local.")?;
    let path = Path::new(text);
    if !path.is_absolute() || text.starts_with(r"\\.\") || text.starts_with(r"\\?\") {
        return Err("Choisis un fichier local.".into());
    }
    let real = path.canonicalize().map_err(|_| "Le fichier choisi est introuvable.".to_string())?;
    if !real.is_file() {
        return Err("Le fichier choisi est introuvable.".into());
    }
    Ok(real)
}

/// Retire le préfixe `\\?\` ajouté par `canonicalize` sous Windows.
pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 d'un fichier, interrompable par `stop`.
pub fn file_digest(path: &Path, stop: Option<&AtomicBool>) -> Res<Option<String>> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("Lecture impossible : {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if stop.is_some_and(|s| s.load(Ordering::Relaxed)) {
            return Ok(None);
        }
        let n = file.read(&mut buf).map_err(|e| format!("Lecture impossible : {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(Some(hex(&hasher.finalize())))
}

pub fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

pub fn u64_of(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

pub fn sources_of(item: &Value) -> &[Value] {
    item.get("sources").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// Index de source fourni par le frontend : entier dans les bornes.
pub fn source_at(item: &Value, index: Option<usize>) -> Res<&Value> {
    index.and_then(|i| sources_of(item).get(i)).ok_or_else(|| "Source introuvable.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn address_rules() {
        assert!(address(Some(&json!("https://example.org/a.iso"))).is_ok());
        for bad in ["http://example.org/a.iso", "https://user:pw@example.org/a", "https://example.org/a#x",
                    "https://192.168.1.2/a.iso", "https://127.0.0.1/a", "https://localhost/a", "https://[::1]/a",
                    "https://10.0.0.1/a", "https://169.254.169.254/latest", "https://100.64.0.1/a", "ftp://x.org/a",
                    "https://exa mple.org/a"] {
            assert!(address(Some(&json!(bad))).is_err(), "{bad}");
        }
        assert!(address(Some(&json!(42))).is_err());
    }

    #[test]
    fn checksum_rules() {
        assert_eq!(checksum(Some(&json!(" ABCDEF0123456789abcdef0123456789ABCDEF0123456789abcdef0123456789 "))).unwrap().len(), 64);
        assert_eq!(checksum(None).unwrap(), "");
        assert!(checksum(Some(&json!("abc"))).is_err());
    }

    #[test]
    fn label_truncates_chars() {
        assert_eq!(label(Some(&json!("  éééé  ")), 2), "éé");
    }
}
