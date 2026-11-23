//! Projets PXE exportés, serveur HTTP local et diagnostic du menu iPXE.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : pxe.py (Pxe, local_interfaces, official, export, diagnose).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use super::pxe_menu::{eligible_address, menu_text, MenuEntry};
use super::pxe_server::{RequestLog, ServeConfig, Server, CHECK_AGENT};
use super::{check_space, file_digest, file_input, hashed_copy, home, is_reparse, json_read, json_write, new_id, now_iso, suffix, R, MIB};

/// Binaires iPXE officiels : empreintes figées (catalogue TokaISO vérifié le 2026-11-15).
pub struct Official {
    pub url: &'static str,
    pub sha256: &'static str,
    pub filename: &'static str,
}
pub const IPXE_ISO: Official = Official {
    url: "https://boot.ipxe.org/ipxe.iso",
    sha256: "7ec601ec4632a76da024e919f2ae98cc9633579b6256f913909f8c7b2bf9b7a4",
    filename: "ipxe.iso",
};
pub const WIMBOOT: Official = Official {
    url: "https://github.com/ipxe/wimboot/releases/download/v2.9.0/wimboot",
    sha256: "5f067ccdc4d084d5bf77b6c853bd0f8402dfc2b4cd1b103d358993ae97fae8e3",
    filename: "wimboot",
};
const DOWNLOAD_LIMIT: u64 = 16 * MIB;
const LOG_SIZE: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub folder: String,
    pub entries: Vec<MenuEntry>,
    pub wimboot: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Client {
    pub address: String,
    pub at: String,
}

#[derive(Default)]
struct State {
    projects: BTreeMap<String, Project>,
    server: Option<Server>,
    url: String,
}

/// Données mises à jour par les threads du serveur (verrou séparé, sans attente sur `State`).
#[derive(Default)]
struct Runtime {
    prefix: String,
    bind: Option<Ipv4Addr>,
    last_client: Option<Client>,
    log: VecDeque<RequestLog>,
}

fn projects_path() -> PathBuf {
    home().join("pxe-projects.json")
}

fn state() -> MutexGuard<'static, State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(State { projects: json_read(&projects_path()), ..Default::default() }))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

fn runtime() -> MutexGuard<'static, Runtime> {
    static RT: OnceLock<Mutex<Runtime>> = OnceLock::new();
    RT.get_or_init(Default::default).lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(Debug, Clone, Serialize)]
pub struct PxeStatus {
    pub projects: Vec<Project>,
    pub url: String,
    pub running: bool,
    pub command: String,
    pub log: Vec<RequestLog>,
}

pub fn status() -> PxeStatus {
    let s = state();
    let log = runtime().log.iter().rev().cloned().collect();
    PxeStatus {
        projects: s.projects.values().cloned().collect(),
        url: s.url.clone(),
        running: s.server.is_some(),
        command: if s.url.is_empty() { String::new() } else { format!("dhcp\nchain {}", s.url) },
        log,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interface {
    pub name: String,
    pub address: String,
}

/// Adresses IPv4 privées de ce PC (lecture seule).
pub fn interfaces() -> Vec<Interface> {
    let script = "$rows=@(Get-NetIPAddress -AddressFamily IPv4 -ErrorAction Stop | Where-Object {$_.AddressState -eq 'Preferred'} | \
                  ForEach-Object {@{name=$_.InterfaceAlias; address=$_.IPAddress}}); ConvertTo-Json -InputObject $rows -Compress";
    let rows: Vec<Interface> = match super::powershell_json(script, serde_json::json!({}), 8) {
        Ok(value) => serde_json::from_value(value).unwrap_or_default(),
        Err(_) => {
            use std::net::ToSocketAddrs;
            let host = std::env::var("COMPUTERNAME").unwrap_or_default();
            let mut found: Vec<String> = (host.as_str(), 0)
                .to_socket_addrs()
                .map(|it| it.filter_map(|a| if let SocketAddr::V4(v4) = a { Some(v4.ip().to_string()) } else { None }).collect())
                .unwrap_or_default();
            found.sort();
            found.dedup();
            found.into_iter().map(|a| Interface { name: a.clone(), address: a }).collect()
        }
    };
    rows.into_iter().filter(|r| eligible_address(&r.address)).collect()
}

/// Télécharge un binaire officiel (HTTPS uniquement, taille bornée, SHA-256 figé) ou réutilise la copie vérifiée.
pub fn official(item: &Official, folder: &Path) -> R<PathBuf> {
    let path = folder.join(item.filename);
    if path.is_file() && file_digest(&path).ok().as_deref() == Some(item.sha256) {
        return Ok(path);
    }
    let stage = path.with_extension("part");
    let result = (|| -> R<()> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::custom(|a| {
                if a.url().scheme() == "https" && a.previous().len() < 10 { a.follow() } else { a.stop() }
            }))
            .user_agent("NiTriTe")
            .build()
            .map_err(|e| e.to_string())?;
        let mut response = client.get(item.url).send().map_err(|e| format!("Téléchargement impossible : {e}"))?;
        if response.url().scheme() != "https" {
            return Err("Redirection sans HTTPS.".into());
        }
        if !response.status().is_success() {
            return Err(format!("Téléchargement refusé (HTTP {}).", response.status().as_u16()));
        }
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&stage).map_err(|e| e.to_string())?;
        let mut received = 0u64;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = response.read(&mut buf).map_err(|e| format!("Téléchargement interrompu : {e}"))?;
            if n == 0 {
                break;
            }
            received += n as u64;
            if received > DOWNLOAD_LIMIT {
                return Err("Taille du téléchargement inattendue.".into());
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
        drop(out);
        if received == 0 || file_digest(&stage)? != item.sha256 {
            return Err("Empreinte SHA-256 différente de celle publiée.".into());
        }
        std::fs::rename(&stage, &path).map_err(|e| e.to_string())
    })();
    let _ = std::fs::remove_file(&stage);
    result.map(|_| path)
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportRequest {
    pub paths: Vec<String>,
    pub destination: String,
    #[serde(default)]
    pub san: bool,
}

fn media_paths(paths: &[String], allowed: &[&str]) -> R<Vec<PathBuf>> {
    if !(1..=24).contains(&paths.len()) {
        return Err("Choisis de 1 à 24 fichiers.".into());
    }
    let mut result: Vec<PathBuf> = Vec::new();
    for value in paths {
        let path = file_input(value)?;
        if !allowed.contains(&suffix(&path).as_str()) || is_reparse(&path) {
            return Err("Le fichier choisi n’est pas un média pris en charge.".into());
        }
        if !result.contains(&path) {
            result.push(path);
        }
    }
    Ok(result)
}

/// Dossier PXE autonome : images numérotées, menu.ipxe, wimboot si WIM, Demarrage.txt, SHA256.txt.
pub fn export(req: &ExportRequest) -> R<Project> {
    let folder = PathBuf::from(&req.destination);
    if !folder.is_absolute() || !folder.is_dir() || is_reparse(&folder) {
        return Err("Choisis un dossier de destination.".into());
    }
    let paths = media_paths(&req.paths, &[".iso", ".wim"])?;
    let sizes: Vec<u64> = paths.iter().map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)).collect();
    check_space(&folder, &sizes, 16 * MIB, "Espace libre insuffisant.")?;
    if paths.iter().any(|p| suffix(p) == ".iso") && !req.san {
        return Err("Confirme que tes ISO prennent en charge le démarrage iPXE par HTTP.".into());
    }
    let key = new_id();
    let target = folder.join(format!("NiTriTe-PXE-{}", &key[..8]));
    let images = target.join("images");
    std::fs::create_dir(&target).map_err(|e| e.to_string())?;
    std::fs::create_dir(&images).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        let index = i + 1;
        let filename = format!("{index:02}{}", suffix(path));
        let sha256 = hashed_copy(path, &images.join(&filename))?;
        let label = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        entries.push(MenuEntry { index, label, file: format!("images/{filename}"), sha256 });
    }
    let wimboot = entries.iter().any(|e| e.file.ends_with(".wim"));
    if wimboot {
        official(&WIMBOOT, &target)?;
    }
    let write = |name: &str, text: String| std::fs::write(target.join(name), text).map_err(|e| e.to_string());
    write("menu.ipxe", menu_text(&entries))?;
    write(
        "Demarrage.txt",
        "NiTriTe PXE\n\nDémarre le client ou la VM sur iPXE. Ouvre sa console (Ctrl+B), lance dhcp, puis chain avec l’adresse du menu HTTP.\n\
         Les ISO doivent prendre en charge le démarrage SAN par HTTP. Pour Windows, utilise un boot.wim WinPE démarrable.\n\
         Pour utiliser un serveur PXE existant, sers ce dossier par HTTP et utilise menu.ipxe comme script iPXE.\n"
            .into(),
    )?;
    write("SHA256.txt", entries.iter().map(|e| format!("{}  {}\n", e.sha256, e.file)).collect())?;
    let project = Project { id: key.clone(), folder: target.to_string_lossy().to_string(), entries, wimboot };
    let mut s = state();
    let mut updated = s.projects.clone();
    updated.insert(key, project.clone());
    json_write(&projects_path(), &updated)?;
    s.projects = updated;
    Ok(project)
}

fn record_menu(prefix: &str, client: Ipv4Addr) {
    let mut rt = runtime();
    if rt.prefix == prefix && rt.bind != Some(client) && !client.is_loopback() && !client.is_unspecified() {
        rt.last_client = Some(Client { address: client.to_string(), at: now_iso() });
    }
}

fn record_log(entry: RequestLog) {
    let mut rt = runtime();
    if rt.log.len() >= LOG_SIZE {
        rt.log.pop_front();
    }
    rt.log.push_back(entry);
}

/// Sert un projet exporté sur l'IPv4 locale choisie (jamais 0.0.0.0).
pub fn start(id: &str, bind: &str) -> R<PxeStatus> {
    {
        let mut s = state();
        if s.server.is_some() {
            return Err("Arrête le serveur PXE actuel avant d’en démarrer un autre.".into());
        }
        let project = s.projects.get(id).cloned().ok_or("Choisis un projet PXE exporté.")?;
        if !eligible_address(bind) {
            return Err("Choisis une adresse IPv4 locale de ce PC.".into());
        }
        let address: Ipv4Addr = bind.parse().map_err(|_| "Choisis une adresse IPv4 locale de ce PC.".to_string())?;
        let mut files: HashSet<String> = project.entries.iter().map(|e| e.file.clone()).collect();
        files.insert("menu.ipxe".into());
        if project.wimboot {
            files.insert("wimboot".into());
        }
        let folder = PathBuf::from(&project.folder);
        if !folder.is_dir() {
            return Err("Le dossier PXE exporté est introuvable.".into());
        }
        let prefix = format!("/pxe-{}/", &new_id()[..16]);
        {
            let mut rt = runtime();
            rt.prefix = prefix.clone();
            rt.bind = Some(address);
            rt.last_client = None;
            rt.log.clear();
        }
        let menu_prefix = prefix.clone();
        let server = Server::start(
            address,
            ServeConfig {
                folder,
                files,
                prefix: prefix.clone(),
                on_menu: Box::new(move |client| record_menu(&menu_prefix, client)),
                on_log: Box::new(record_log),
            },
        )
        .map_err(|_| "Le serveur PXE ne peut pas écouter sur cette adresse IPv4.".to_string())?;
        s.url = format!("http://{bind}:{}{prefix}menu.ipxe", server.port);
        s.server = Some(server);
        tracing::info!("boot_lab: serveur PXE démarré sur {}", s.url);
    }
    Ok(status())
}

pub fn stop() -> PxeStatus {
    {
        let mut s = state();
        if let Some(mut server) = s.server.take() {
            server.stop();
            tracing::info!("boot_lab: serveur PXE arrêté");
        }
        s.url.clear();
        let mut rt = runtime();
        rt.prefix.clear();
        rt.bind = None;
        rt.last_client = None;
    }
    status()
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnose {
    pub running: bool,
    pub http: bool,
    pub external: bool,
    pub client: Option<Client>,
    pub command: String,
    pub port: u16,
}

/// Lit le menu comme le ferait iPXE (agent de contrôle : ne compte pas comme client).
pub fn diagnose() -> Diagnose {
    let url = state().url.clone();
    let client = runtime().last_client.clone();
    let parsed = url.strip_prefix("http://").and_then(|rest| {
        let (host_port, path) = rest.split_once('/')?;
        let (host, port) = host_port.split_once(':')?;
        Some((host.parse::<Ipv4Addr>().ok()?, port.parse::<u16>().ok()?, format!("/{path}")))
    });
    let Some((host, port, path)) = parsed else {
        return Diagnose { running: false, http: false, external: false, client: None, command: String::new(), port: 0 };
    };
    let http = (|| -> Option<bool> {
        let mut stream = TcpStream::connect_timeout(&SocketAddr::from((host, port)), Duration::from_secs(4)).ok()?;
        stream.set_read_timeout(Some(Duration::from_secs(4))).ok()?;
        write!(stream, "GET {path} HTTP/1.0\r\nHost: {host}:{port}\r\nUser-Agent: {CHECK_AGENT}\r\n\r\n").ok()?;
        let mut reply = Vec::new();
        stream.take(64 * 1024).read_to_end(&mut reply).ok()?;
        let text = String::from_utf8_lossy(&reply);
        let (head, body) = text.split_once("\r\n\r\n")?;
        Some(head.starts_with("HTTP/1.0 200") && body.starts_with("#!ipxe"))
    })()
    .unwrap_or(false);
    Diagnose { running: true, http, external: !host.is_loopback(), client, command: format!("dhcp\nchain {url}"), port }
}

pub fn cache_dir() -> PathBuf {
    let dir = home().join("pxe-cache");
    let _ = std::fs::create_dir_all(&dir);
    dir
}
