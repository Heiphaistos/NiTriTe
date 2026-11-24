//! Serveur HTTP de fichiers en lecture seule pour iPXE (std, un thread par connexion).
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : pxe.py (http_server).

use serde::Serialize;
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use super::is_reparse;
use super::pxe_menu::parse_range;

/// Agent HTTP du diagnostic interne : ses lectures du menu ne comptent pas comme un client.
pub const CHECK_AGENT: &str = "NiTriTe PXE Check";
const MAX_CONNECTIONS: usize = 32;
const MAX_HEADER: usize = 16 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct RequestLog {
    pub at: String,
    pub client: String,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub bytes: u64,
}

pub struct ServeConfig {
    pub folder: PathBuf,
    pub files: HashSet<String>,
    pub prefix: String,
    /// Appelé après une lecture complète de menu.ipxe par un vrai client.
    pub on_menu: Box<dyn Fn(Ipv4Addr) + Send + Sync>,
    pub on_log: Box<dyn Fn(RequestLog) + Send + Sync>,
}

pub struct Server {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub port: u16,
}

impl Server {
    /// Écoute uniquement sur `bind` (port choisi par le système).
    pub fn start(bind: Ipv4Addr, config: ServeConfig) -> std::io::Result<Server> {
        let listener = TcpListener::bind(SocketAddrV4::new(bind, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let config = Arc::new(config);
        let thread = std::thread::Builder::new()
            .name("boot-lab-pxe".into())
            .spawn(move || accept_loop(listener, config, flag))?;
        Ok(Server { stop, thread: Some(thread), port })
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

fn accept_loop(listener: TcpListener, config: Arc<ServeConfig>, stop: Arc<AtomicBool>) {
    let active = Arc::new(AtomicUsize::new(0));
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                if active.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                    continue; // refus implicite : la connexion se ferme
                }
                active.fetch_add(1, Ordering::SeqCst);
                let (config, stop, active) = (config.clone(), stop.clone(), active.clone());
                let spawned = std::thread::Builder::new().name("boot-lab-pxe-client".into()).spawn(move || {
                    handle(stream, peer, &config, &stop);
                    active.fetch_sub(1, Ordering::SeqCst);
                });
                if spawned.is_err() {
                    tracing::warn!("boot_lab: thread client PXE impossible");
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(60)),
        }
    }
}

struct Request {
    method: String,
    target: String,
    range: Option<String>,
    agent: Option<String>,
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream.take(MAX_HEADER as u64));
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let (mut range, mut agent) = (None, None);
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "range" => range = Some(value.trim().to_string()),
                "user-agent" => agent = Some(value.trim().to_string()),
                _ => {}
            }
        }
    }
    Some(Request { method, target, range, agent })
}

fn head(stream: &mut TcpStream, status: u16, reason: &str, extra: &[(String, String)]) -> std::io::Result<()> {
    let mut text = format!("HTTP/1.0 {status} {reason}\r\nServer: NiTriTe\r\nConnection: close\r\n");
    for (k, v) in extra {
        text.push_str(&format!("{k}: {v}\r\n"));
    }
    text.push_str("\r\n");
    stream.write_all(text.as_bytes())
}

fn error(stream: &mut TcpStream, status: u16, reason: &str) {
    let len = [("Content-Length".to_string(), "0".to_string())];
    let _ = head(stream, status, reason, &len);
}

fn handle(mut stream: TcpStream, peer: SocketAddr, config: &ServeConfig, stop: &AtomicBool) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
    let Some(request) = read_request(&stream) else {
        return;
    };
    let route = request.target.split(['?', '#']).next().unwrap_or("").to_string();
    let (status, bytes, complete) = reply(&mut stream, &request, &route, config, stop);
    let client = match peer.ip() {
        std::net::IpAddr::V4(ip) => ip,
        std::net::IpAddr::V6(_) => Ipv4Addr::UNSPECIFIED,
    };
    (config.on_log)(RequestLog {
        at: super::now_iso(),
        client: peer.ip().to_string(),
        method: request.method.chars().take(16).collect(),
        path: route.chars().take(300).collect(),
        status,
        bytes,
    });
    let full_menu = complete
        && status == 200
        && request.method == "GET"
        && route.ends_with("/menu.ipxe")
        && request.agent.as_deref() != Some(CHECK_AGENT);
    if full_menu {
        (config.on_menu)(client);
    }
}

/// Renvoie (statut, octets du corps envoyés, corps complet envoyé).
fn reply(stream: &mut TcpStream, request: &Request, route: &str, config: &ServeConfig, stop: &AtomicBool) -> (u16, u64, bool) {
    let body = match request.method.as_str() {
        "GET" => true,
        "HEAD" => false,
        _ => {
            error(stream, 501, "Not Implemented");
            return (501, 0, false);
        }
    };
    let Some(name) = route.strip_prefix(config.prefix.as_str()).filter(|n| config.files.contains(*n)) else {
        error(stream, 404, "Not Found");
        return (404, 0, false);
    };
    let path = config.folder.join(name);
    let parent = path.parent().map(PathBuf::from).unwrap_or_else(|| config.folder.clone());
    if [&config.folder, &parent, &path].iter().any(|p| is_reparse(p)) {
        error(stream, 404, "Not Found");
        return (404, 0, false);
    }
    let Ok(mut file) = std::fs::File::open(&path) else {
        error(stream, 404, "Not Found");
        return (404, 0, false);
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let Some((start, length, partial)) = parse_range(request.range.as_deref(), size) else {
        let headers = [
            ("Content-Range".to_string(), format!("bytes */{size}")),
            ("Content-Length".to_string(), "0".to_string()),
        ];
        let _ = head(stream, 416, "Range Not Satisfiable", &headers);
        return (416, 0, false);
    };
    let kind = if name.ends_with(".ipxe") { "text/plain" } else { "application/octet-stream" };
    let mut headers = vec![
        ("Accept-Ranges".to_string(), "bytes".to_string()),
        ("Content-Type".to_string(), kind.to_string()),
        ("Content-Length".to_string(), length.to_string()),
    ];
    if partial {
        headers.push(("Content-Range".to_string(), format!("bytes {start}-{}/{size}", start + length - 1)));
    }
    let (status, reason) = if partial { (206, "Partial Content") } else { (200, "OK") };
    if head(stream, status, reason, &headers).is_err() || !body {
        return (status, 0, false);
    }
    if file.seek(SeekFrom::Start(start)).is_err() {
        return (status, 0, false);
    }
    let mut remaining = length;
    let mut buf = vec![0u8; 256 * 1024];
    while remaining > 0 && !stop.load(Ordering::SeqCst) {
        let want = remaining.min(buf.len() as u64) as usize;
        let n = match file.read(&mut buf[..want]) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        if stream.write_all(&buf[..n]).is_err() {
            break;
        }
        remaining -= n as u64;
    }
    (status, length - remaining, remaining == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn get(port: u16, path: &str, extra: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connexion");
        write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n{extra}\r\n").expect("envoi");
        let mut out = String::new();
        s.read_to_string(&mut out).expect("lecture");
        out
    }

    #[test]
    fn sert_uniquement_les_fichiers_du_projet() {
        let dir = std::env::temp_dir().join(format!("nitrite-pxe-test-{}", super::super::new_id()));
        std::fs::create_dir_all(dir.join("images")).expect("dossier");
        std::fs::write(dir.join("menu.ipxe"), "#!ipxe\necho ok\n").expect("menu");
        std::fs::write(dir.join("images/01.iso"), b"0123456789").expect("iso");
        std::fs::write(dir.join("secret.txt"), b"non").expect("secret");
        let menus = Arc::new(Mutex::new(Vec::new()));
        let logs = Arc::new(Mutex::new(0usize));
        let (m, l) = (menus.clone(), logs.clone());
        let mut server = Server::start(
            Ipv4Addr::LOCALHOST,
            ServeConfig {
                folder: dir.clone(),
                files: ["menu.ipxe", "images/01.iso"].map(String::from).into_iter().collect(),
                prefix: "/pxe-abc/".into(),
                on_menu: Box::new(move |ip| m.lock().map(|mut v| v.push(ip)).unwrap_or(())),
                on_log: Box::new(move |_| l.lock().map(|mut n| *n += 1).unwrap_or(())),
            },
        )
        .expect("serveur");
        let menu = get(server.port, "/pxe-abc/menu.ipxe", "");
        assert!(menu.starts_with("HTTP/1.0 200 OK"), "{menu}");
        assert!(menu.ends_with("#!ipxe\necho ok\n"));
        let part = get(server.port, "/pxe-abc/images/01.iso", "Range: bytes=2-4\r\n");
        assert!(part.starts_with("HTTP/1.0 206"));
        assert!(part.contains("Content-Range: bytes 2-4/10"));
        assert!(part.ends_with("234"));
        assert!(get(server.port, "/pxe-abc/images/01.iso", "Range: bytes=50-\r\n").starts_with("HTTP/1.0 416"));
        assert!(get(server.port, "/pxe-abc/secret.txt", "").starts_with("HTTP/1.0 404"));
        assert!(get(server.port, "/pxe-abc/../secret.txt", "").starts_with("HTTP/1.0 404"));
        assert!(get(server.port, "/autre/menu.ipxe", "").starts_with("HTTP/1.0 404"));
        let check = format!("User-Agent: {CHECK_AGENT}\r\n");
        assert!(get(server.port, "/pxe-abc/menu.ipxe", &check).starts_with("HTTP/1.0 200"));
        let port = server.port;
        server.stop();
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err(), "le serveur doit être fermé");
        assert_eq!(menus.lock().map(|v| v.len()).unwrap_or(99), 1);
        assert_eq!(logs.lock().map(|n| *n).unwrap_or(0), 7);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
