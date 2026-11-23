use serde::Serialize;
#[cfg(target_os = "windows")]
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use crate::maintenance::commands::decode_output;

#[derive(Debug, Clone, Serialize, Default)]
pub struct HostsEntry {
    pub ip: String,
    pub hostname: String,
    pub comment: String,
    pub active: bool,
    pub line_number: u32,
}

/// Chemin du fichier hosts : `%SystemRoot%` plutôt que `C:\Windows` en dur
/// (Windows installé sur un autre lecteur, WinPE).
fn hosts_path() -> String {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    format!(r"{}\System32\drivers\etc\hosts", root)
}

// Anti-freeze : lecture fichier hosts + résolutions associées peuvent
// bloquer — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn get_hosts_entries() -> Vec<HostsEntry> {
    tokio::task::spawn_blocking(get_hosts_entries_blocking)
        .await
        .unwrap_or_default()
}

fn get_hosts_entries_blocking() -> Vec<HostsEntry> {
    let path = hosts_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            // Repli : fichier hosts non UTF-8 (commentaires hérités ANSI/OEM).
            // decode_output gère le repli OEM, from_utf8_lossy donnerait du mojibake.
            #[cfg(target_os = "windows")]
            {
                let o = Command::new("powershell")
                    .args(["-NoProfile","-NonInteractive","-Command", "Get-Content -LiteralPath $env:SystemRoot\\System32\\drivers\\etc\\hosts -Raw"])
                    .creation_flags(0x08000000).output().ok();
                if let Some(o) = o {
                    decode_output(&o.stdout)
                } else { return vec![]; }
            }
            #[cfg(not(target_os = "windows"))]
            return vec![];
        }
    };
    parse_hosts(&content)
}

fn parse_hosts(content: &str) -> Vec<HostsEntry> {
    // BOM UTF-8 (fichier réécrit par Set-Content -Encoding UTF8 de PS 5.1) :
    // sans ce retrait, une entrée en 1re ligne était ignorée.
    let content = content.trim_start_matches('\u{feff}');
    let mut entries = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let active = !trimmed.starts_with('#');
        let parse_line = if active { trimmed } else { trimmed.trim_start_matches('#').trim() };

        let parts: Vec<&str> = parse_line.splitn(2, |c: char| c.is_whitespace()).collect();
        if !parts.is_empty() {
            let ip = parts[0].trim();
            let rest = parts.get(1).unwrap_or(&"").trim();
            let (hostname, comment) = if let Some(ci) = rest.find('#') {
                (rest[..ci].trim(), rest[ci+1..].trim())
            } else {
                (rest, "")
            };
            // L'en-tête standard (« # This file contains... ») est du texte libre,
            // pas des entrées désactivées : on n'accepte qu'une vraie IP en 1er mot.
            if !ip.is_empty() && !hostname.is_empty() && ip.parse::<std::net::IpAddr>().is_ok() {
                entries.push(HostsEntry {
                    ip: ip.to_string(),
                    hostname: hostname.split_whitespace().next().unwrap_or("").to_string(),
                    comment: comment.to_string(),
                    active,
                    line_number: i as u32 + 1,
                });
            }
        }
    }
    entries
}

// ─── Édition au niveau octet ──────────────────────────────────────────────────
// Les modifications se font directement en Rust sur les octets du fichier :
// seule la ligne visée change, les autres restent identiques (encodage, CRLF,
// BOM). L'ancienne version passait par Get-Content/Set-Content -Encoding UTF8
// de PowerShell 5.1, qui relisait le fichier en ANSI et le réécrivait avec un
// BOM à chaque modification.

/// Découpe en lignes en gardant le `\n` final de chaque ligne.
fn split_lines(bytes: &[u8]) -> Vec<&[u8]> {
    bytes.split_inclusive(|b| *b == b'\n').collect()
}

fn line_body(line: &[u8]) -> &[u8] {
    let l = line.strip_suffix(b"\n").unwrap_or(line);
    l.strip_suffix(b"\r").unwrap_or(l)
}

fn trim_start_ws(b: &[u8]) -> &[u8] {
    let n = b.iter().take_while(|c| c.is_ascii_whitespace()).count();
    &b[n..]
}

/// Vérifie que la ligne visée porte bien le nom d'hôte affiché dans l'UI :
/// si le fichier a changé entre l'affichage et l'action (autre outil, ajout
/// sans rechargement), on refuse au lieu de modifier la mauvaise ligne.
fn check_expected(line: &[u8], expected: Option<&str>) -> Result<(), String> {
    let Some(exp) = expected.map(str::trim).filter(|e| !e.is_empty()) else { return Ok(()) };
    let text = String::from_utf8_lossy(line_body(line));
    let body = text.trim().trim_start_matches('#');
    let body = body.split('#').next().unwrap_or("");
    if body.split_whitespace().skip(1).any(|h| h.eq_ignore_ascii_case(exp)) {
        Ok(())
    } else {
        Err("Le fichier hosts a changé depuis l'affichage : actualisez la liste avant de modifier.".to_string())
    }
}

fn apply_delete(bytes: &[u8], line_number: u32, expected: Option<&str>) -> Result<Vec<u8>, String> {
    let lines = split_lines(bytes);
    let idx = (line_number as usize).checked_sub(1).ok_or("Numéro de ligne invalide")?;
    let line = lines.get(idx).ok_or("Ligne introuvable")?;
    check_expected(line, expected)?;
    Ok(lines.iter().enumerate().filter(|(i, _)| *i != idx).flat_map(|(_, l)| l.iter().copied()).collect())
}

fn apply_toggle(bytes: &[u8], line_number: u32, enable: bool, expected: Option<&str>) -> Result<Vec<u8>, String> {
    let lines = split_lines(bytes);
    let idx = (line_number as usize).checked_sub(1).ok_or("Numéro de ligne invalide")?;
    let line = *lines.get(idx).ok_or("Ligne introuvable")?;
    check_expected(line, expected)?;
    let body = line_body(line);
    let ending = &line[body.len()..];
    let start = trim_start_ws(body);
    let new_body: Vec<u8> = if enable {
        let n = start.iter().take_while(|c| **c == b'#').count();
        trim_start_ws(&start[n..]).to_vec()
    } else if start.starts_with(b"#") {
        body.to_vec()
    } else {
        [b"# ".as_slice(), body].concat()
    };
    let mut out = Vec::with_capacity(bytes.len() + 2);
    for (i, l) in lines.iter().enumerate() {
        if i == idx { out.extend_from_slice(&new_body); out.extend_from_slice(ending); }
        else { out.extend_from_slice(l); }
    }
    Ok(out)
}

fn apply_add(bytes: &[u8], ip: &str, host: &str, comment: &str) -> Result<Vec<u8>, String> {
    let text = String::from_utf8_lossy(bytes);
    if parse_hosts(&text).iter().any(|e| e.active && e.ip == ip && e.hostname.eq_ignore_ascii_case(host)) {
        return Err(format!("Entrée déjà présente : {} -> {}", ip, host));
    }
    let mut out = bytes.to_vec();
    if !out.is_empty() && !out.ends_with(b"\n") { out.extend_from_slice(b"\r\n"); }
    let line = if comment.is_empty() { format!("{}\t{}\r\n", ip, host) } else { format!("{}\t{}\t# {}\r\n", ip, host, comment) };
    out.extend_from_slice(line.as_bytes());
    Ok(out)
}

/// Sauvegarde automatique (hosts.bak = état avant la dernière modification)
/// puis écriture et relecture. Fail-closed : pas de sauvegarde, pas d'écriture.
fn write_hosts_with_backup(new_bytes: &[u8]) -> Result<(), String> {
    let path = hosts_path();
    std::fs::copy(&path, format!("{}.bak", path))
        .map_err(|e| format!("Sauvegarde préalable impossible, aucune modification faite ({}). Droits administrateur requis.", e))?;
    std::fs::write(&path, new_bytes)
        .map_err(|e| format!("Écriture du fichier hosts refusée ({}). Droits administrateur requis.", e))?;
    // Relecture : un antivirus (Defender « HostsFileHijack ») peut annuler l'écriture.
    if std::fs::read(&path).map_err(|e| e.to_string())? != new_bytes {
        return Err("Le fichier hosts a été modifié ou restauré juste après l'écriture (antivirus ?).".to_string());
    }
    Ok(())
}

fn read_hosts_bytes() -> Result<Vec<u8>, String> {
    std::fs::read(hosts_path()).map_err(|e| format!("Lecture du fichier hosts impossible : {}", e))
}

// Anti-freeze : écriture fichier hosts est bloquante — jamais inline sur le
// thread de commande.
#[tauri::command]
pub async fn add_hosts_entry(ip: String, hostname: String, comment: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || add_hosts_entry_blocking(ip, hostname, comment))
        .await
        .map_err(|e| e.to_string())?
}

/// Valide et nettoie l'entrée. Le nom d'hôte suit RFC 1123 (ASCII, 253 car.
/// max, étiquettes de 1 à 63 car. sans tiret en bordure ; `_` toléré).
fn validate_entry(ip: &str, hostname: &str, comment: &str) -> Result<(String, String, String), String> {
    let ip_c: String = ip.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string();
    let host_c: String = hostname.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string();
    let comment_c: String = comment.chars()
        .filter(|c| c.is_alphanumeric() || " -_.".contains(*c))
        .collect::<String>().trim().to_string();
    if ip_c.is_empty() || host_c.is_empty() {
        return Err("IP et hostname requis".to_string());
    }
    if ip_c.parse::<std::net::IpAddr>().is_err() {
        return Err(format!("IP invalide : {}", ip_c));
    }
    let labels_ok = host_c.len() <= 253 && host_c.split('.').all(|l| {
        !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-')
            && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    });
    if !labels_ok {
        return Err(format!("Hostname invalide : {}", host_c));
    }
    Ok((ip_c, host_c, comment_c))
}

fn add_hosts_entry_blocking(ip: String, hostname: String, comment: String) -> Result<String, String> {
    let (ip_c, host_c, comment_c) = validate_entry(&ip, &hostname, &comment)?;
    let new_bytes = apply_add(&read_hosts_bytes()?, &ip_c, &host_c, &comment_c)?;
    write_hosts_with_backup(&new_bytes)?;
    Ok(format!("Entrée ajoutée : {} -> {}", ip_c, host_c))
}

// Anti-freeze : écriture fichier hosts est bloquante — jamais inline sur le
// thread de commande.
#[tauri::command]
pub async fn delete_hosts_entry(line_number: u32, expected_hostname: Option<String>) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let new_bytes = apply_delete(&read_hosts_bytes()?, line_number, expected_hostname.as_deref())?;
        write_hosts_with_backup(&new_bytes)?;
        Ok("Entrée supprimée".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// Anti-freeze : écriture fichier hosts est bloquante — jamais inline sur le
// thread de commande.
#[tauri::command]
pub async fn toggle_hosts_entry(line_number: u32, enable: bool, expected_hostname: Option<String>) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let new_bytes = apply_toggle(&read_hosts_bytes()?, line_number, enable, expected_hostname.as_deref())?;
        write_hosts_with_backup(&new_bytes)?;
        Ok("Modifié".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// Anti-freeze : I/O fichier — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn backup_hosts() -> Result<String, String> {
    tokio::task::spawn_blocking(backup_hosts_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn backup_hosts_blocking() -> Result<String, String> {
    let path = hosts_path();
    let backup = format!("{}.bak", path);
    std::fs::copy(&path, &backup).map(|_| format!("Sauvegarde : {}", backup)).map_err(|e| e.to_string())
}

// Anti-freeze : I/O fichier — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn get_hosts_raw() -> String {
    tokio::task::spawn_blocking(get_hosts_raw_blocking)
        .await
        .unwrap_or_default()
}

fn get_hosts_raw_blocking() -> String {
    std::fs::read_to_string(hosts_path()).unwrap_or_default()
}

// Anti-freeze : résolution DNS est bloquante — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn resolve_hostname(hostname: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || resolve_hostname_blocking(hostname))
        .await
        .map_err(|e| e.to_string())?
}

fn resolve_hostname_blocking(hostname: String) -> Result<String, String> {
    use std::net::ToSocketAddrs;
    let addr = format!("{}:80", hostname.trim());
    match addr.to_socket_addrs() {
        Ok(iter) => {
            let mut seen = std::collections::HashSet::new();
            for a in iter { seen.insert(a.ip().to_string()); }
            let ips: Vec<String> = seen.into_iter().collect();
            Ok(ips.join(", "))
        }
        Err(e) => Err(format!("Résolution échouée : {}", e)),
    }
}

#[tauri::command]
pub fn import_hosts_blocklist(url: String, _list_name: String) -> Result<String, String> {
    Err(format!(
        "Import en ligne non disponible dans cette version. Téléchargez manuellement : {} et importez-le.",
        url
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"# Copyright (c) 1993-2009 Microsoft Corp.\r\n#\r\n# 127.0.0.1 localhost\r\n10.0.0.5\tnas.lan\t# NAS\r\n0.0.0.0 ads.example.com\r\n";

    #[test]
    fn valid_ipv4_and_hostname() {
        assert!(validate_entry("192.168.1.10", "local.dev", "").is_ok());
        assert!(validate_entry("127.0.0.1", "my-server", "").is_ok());
        assert!(validate_entry("127.0.0.1", "_dmarc.example.com", "").is_ok());
    }

    #[test]
    fn valid_ipv6() {
        assert!(validate_entry("::1", "localhost6", "").is_ok());
        assert!(validate_entry("2001:db8::1", "test.host", "").is_ok());
    }

    #[test]
    fn invalid_ip_rejected() {
        assert!(validate_entry("not-an-ip", "host.local", "").is_err());
        assert!(validate_entry("999.999.999.999", "host.local", "").is_err());
        assert!(validate_entry("", "host.local", "").is_err());
    }

    #[test]
    fn hostname_rules() {
        assert!(validate_entry("127.0.0.1", "evil;whoami", "").is_err());
        assert!(validate_entry("127.0.0.1", "host|cmd", "").is_err());
        assert!(validate_entry("127.0.0.1", "a b", "").is_err());
        assert!(validate_entry("127.0.0.1", "-bad.com", "").is_err());
        assert!(validate_entry("127.0.0.1", "bad..com", "").is_err());
        assert!(validate_entry("127.0.0.1", &"a".repeat(64), "").is_err());
        // Caractères de contrôle retirés (pas de saut de ligne injecté)
        let (_, h, _) = validate_entry("127.0.0.1", "host\nevil", "").unwrap();
        assert_eq!(h, "hostevil");
    }

    #[test]
    fn comment_is_whitelisted() {
        let (_, _, c) = validate_entry("127.0.0.1", "h", "a;b|c\r\n# d").unwrap();
        assert_eq!(c, "abc d");
    }

    #[test]
    fn parse_ignores_header_and_bom() {
        let mut with_bom = "\u{feff}".as_bytes().to_vec();
        with_bom.extend_from_slice(b"1.2.3.4 first.lan\r\n");
        let e = parse_hosts(&String::from_utf8_lossy(&with_bom));
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].hostname, "first.lan");
        let e = parse_hosts(&String::from_utf8_lossy(SAMPLE));
        assert_eq!(e.len(), 3);
        assert!(!e[0].active && e[0].hostname == "localhost" && e[0].line_number == 3);
        assert_eq!((e[1].line_number, e[1].comment.as_str()), (4, "NAS"));
    }

    #[test]
    fn toggle_only_touches_target_line() {
        let out = apply_toggle(SAMPLE, 5, false, Some("ads.example.com")).unwrap();
        assert_eq!(out, b"# Copyright (c) 1993-2009 Microsoft Corp.\r\n#\r\n# 127.0.0.1 localhost\r\n10.0.0.5\tnas.lan\t# NAS\r\n# 0.0.0.0 ads.example.com\r\n");
        let back = apply_toggle(&out, 5, true, Some("ads.example.com")).unwrap();
        assert_eq!(back, SAMPLE);
        // Ré-activer une entrée déjà active : inchangé
        assert_eq!(apply_toggle(SAMPLE, 4, true, None).unwrap(), SAMPLE);
    }

    #[test]
    fn delete_removes_one_line() {
        let out = apply_delete(SAMPLE, 4, Some("nas.lan")).unwrap();
        assert!(!String::from_utf8_lossy(&out).contains("nas.lan"));
        assert_eq!(split_lines(&out).len(), split_lines(SAMPLE).len() - 1);
        assert!(apply_delete(SAMPLE, 0, None).is_err());
        assert!(apply_delete(SAMPLE, 99, None).is_err());
    }

    #[test]
    fn stale_line_number_refused() {
        // La ligne 4 n'est plus nas.lan : refuser au lieu de modifier la mauvaise ligne
        assert!(apply_delete(SAMPLE, 4, Some("ads.example.com")).is_err());
        assert!(apply_toggle(SAMPLE, 5, false, Some("nas.lan")).is_err());
    }

    #[test]
    fn add_appends_and_refuses_duplicates() {
        let out = apply_add(b"1.1.1.1 a", "2.2.2.2", "b.lan", "").unwrap();
        assert_eq!(out, b"1.1.1.1 a\r\n2.2.2.2\tb.lan\r\n");
        assert!(apply_add(SAMPLE, "10.0.0.5", "NAS.lan", "").is_err());
        // Entrée désactivée identique : on peut la rajouter active
        assert!(apply_add(SAMPLE, "127.0.0.1", "localhost", "").is_ok());
    }
}
