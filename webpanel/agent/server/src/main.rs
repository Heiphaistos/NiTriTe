//! NiTriTe Agent — NiTriTe sans fenetre.
//!
//! Le backend complet de NiTriTe (toutes ses commandes) tourne ici et l'on
//! s'en sert depuis le navigateur : http://127.0.0.1:7878. Aucune WebView,
//! aucun Chromium embarque : sur un petit PC, c'est le navigateur deja ouvert
//! qui affiche l'interface.
//!
//! Securite (l'agent tourne en administrateur, comme NiTriTe) :
//! - ecoute sur 127.0.0.1 uniquement (sauf `--lan`, explicite) ;
//! - jeton aleatoire par session, exige sur toute l'API ;
//! - en-tete Host verifie (parade au « DNS rebinding ») et Origin verifie ;
//! - les confirmations d'administration restent des boites Windows natives.

#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod host;
mod security;
mod session;
mod web;

use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

pub const DEFAULT_PORT: u16 = 7878;

#[derive(Debug, Clone)]
pub struct Options {
    pub port: u16,
    pub lan: bool,
    pub open_browser: bool,
    /// Arret automatique apres ce delai sans navigateur connecte (0 = jamais).
    pub idle_exit: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options { port: DEFAULT_PORT, lan: false, open_browser: true, idle_exit: Duration::from_secs(15 * 60) }
    }
}

pub fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut o = Options::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--port" | "-p" => {
                o.port = it.next().and_then(|v| v.parse().ok()).ok_or("--port attend un numero")?;
            }
            "--lan" => o.lan = true,
            "--no-browser" => o.open_browser = false,
            "--stay" => o.idle_exit = Duration::ZERO,
            "--idle-minutes" => {
                let m: u64 = it.next().and_then(|v| v.parse().ok()).ok_or("--idle-minutes attend un nombre")?;
                o.idle_exit = Duration::from_secs(m * 60);
            }
            "--help" | "-h" => return Err(HELP.to_string()),
            other => return Err(format!("option inconnue : {other}\n\n{HELP}")),
        }
    }
    Ok(o)
}

const HELP: &str = "nitrite-agent [--port N] [--no-browser] [--stay | --idle-minutes N] [--lan]

  --port N          port d'ecoute (defaut 7878, le suivant libre si occupe)
  --no-browser      ne pas ouvrir le navigateur au demarrage
  --stay            ne jamais s'arreter tout seul
  --idle-minutes N  arret apres N minutes sans navigateur connecte (defaut 15)
  --lan             ecouter sur le reseau local (acces depuis un autre PC,
                    jeton toujours exige). Les boites de confirmation
                    s'affichent sur CE poste.";

pub struct AppCtx {
    pub token: String,
    pub port: u16,
    pub lan: bool,
    pub clients: AtomicUsize,
    pub last_activity: AtomicI64,
}

impl AppCtx {
    pub fn touch(&self) {
        self.last_activity.store(now_secs(), Ordering::Relaxed);
    }
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

static OPTIONS: OnceLock<Options> = OnceLock::new();

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = match parse_args(&args) {
        Ok(o) => o,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };

    // Instance deja lancee : on ouvre simplement le navigateur dessus.
    if let Some(existing) = session::find_running() {
        if opts.open_browser {
            let _ = open::that(existing.url());
        }
        println!("NiTriTe Agent deja actif : {}", existing.url());
        return;
    }

    let _ = OPTIONS.set(opts);
    tauri::set_runner(serve_forever);
    // Initialise le backend NiTriTe (journaux, config, etat) puis appelle
    // `Builder::run`, qui passe la main a `serve_forever`.
    nitrite_lib::run();
}

fn pick_listener(lan: bool, first_port: u16) -> std::io::Result<TcpListener> {
    let ip = if lan { [0, 0, 0, 0] } else { [127, 0, 0, 1] };
    let mut last_err = None;
    for port in first_port..first_port.saturating_add(20) {
        match TcpListener::bind(SocketAddr::from((ip, port))) {
            Ok(l) => return Ok(l),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("aucun port libre")))
}

fn serve_forever() {
    let opts = OPTIONS.get().cloned().unwrap_or_default();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("runtime tokio");
    rt.block_on(async move {
        let std_listener = match pick_listener(opts.lan, opts.port) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Impossible d'ouvrir un port : {e}");
                std::process::exit(1);
            }
        };
        std_listener.set_nonblocking(true).expect("socket non bloquante");
        let port = std_listener.local_addr().map(|a| a.port()).unwrap_or(opts.port);
        let ctx = Arc::new(AppCtx {
            token: security::new_token(),
            port,
            lan: opts.lan,
            clients: AtomicUsize::new(0),
            last_activity: AtomicI64::new(now_secs()),
        });
        let sess = session::Session { port, token: ctx.token.clone(), pid: std::process::id() };
        session::write(&sess);

        let app = router(ctx.clone());
        let listener = tokio::net::TcpListener::from_std(std_listener).expect("listener tokio");

        println!("NiTriTe Agent {} — {}", env!("CARGO_PKG_VERSION"), sess.url());
        if opts.lan {
            println!("ATTENTION : mode reseau local actif. Partagez le lien (jeton compris) uniquement avec des personnes de confiance.");
        }
        if opts.open_browser {
            let _ = open::that(sess.url());
        }

        if !opts.idle_exit.is_zero() {
            tokio::spawn(idle_watchdog(ctx.clone(), opts.idle_exit));
        }

        let server = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>());
        let shutdown = async {
            let _ = tokio::signal::ctrl_c().await;
        };
        let _ = server.with_graceful_shutdown(shutdown).await;
        session::clear();
    });
}

/// Arret quand plus aucun onglet n'est connecte depuis `idle` : fermer le
/// navigateur suffit a liberer le PC, sans interface pour « quitter ».
async fn idle_watchdog(ctx: Arc<AppCtx>, idle: Duration) {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let quiet = now_secs() - ctx.last_activity.load(Ordering::Relaxed);
        if ctx.clients.load(Ordering::Relaxed) == 0 && quiet >= idle.as_secs() as i64 {
            println!("Aucun navigateur connecte depuis {} min : arret.", idle.as_secs() / 60);
            session::clear();
            std::process::exit(0);
        }
    }
}

pub fn router(ctx: Arc<AppCtx>) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/commands", get(commands))
        .route("/invoke/{cmd}", post(invoke))
        .route("/events", get(events))
        .route("/host/shutdown", post(shutdown))
        .merge(host::routes())
        .layer(axum::middleware::from_fn_with_state(ctx.clone(), security::require_token));

    Router::new()
        .nest("/api", api)
        .fallback(web::static_file)
        .layer(axum::middleware::from_fn_with_state(ctx.clone(), security::check_host_and_origin))
        .with_state(ctx)
}

async fn health(State(ctx): State<Arc<AppCtx>>) -> Json<Value> {
    Json(json!({
        "name": "NiTriTe Agent",
        "agentVersion": env!("CARGO_PKG_VERSION"),
        "commands": tauri::command_names().len(),
        "lan": ctx.lan,
    }))
}

async fn commands() -> Json<Vec<&'static str>> {
    Json(tauri::command_names())
}

async fn invoke(State(ctx): State<Arc<AppCtx>>, Path(cmd): Path<String>, body: Option<Json<Value>>) -> Response {
    ctx.touch();
    let args = body.map(|Json(v)| v).unwrap_or(Value::Null);
    let args = if args.is_null() { json!({}) } else { args };
    match tauri::dispatch(&cmd, args) {
        Err(tauri::DispatchError::NotFound) => {
            (StatusCode::NOT_FOUND, Json(json!(format!("Commande inconnue : {cmd}")))).into_response()
        }
        Ok(fut) => {
            // Une commande qui panique ne doit pas emporter le serveur.
            match tokio::spawn(fut).await {
                Ok(Ok(v)) => Json(v).into_response(),
                Ok(Err(e)) => (StatusCode::UNPROCESSABLE_ENTITY, Json(e)).into_response(),
                Err(join) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(format!("La commande {cmd} a echoue : {join}")))).into_response(),
            }
        }
    }
}

async fn events(State(ctx): State<Arc<AppCtx>>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| event_loop(socket, ctx))
}

async fn event_loop(mut socket: WebSocket, ctx: Arc<AppCtx>) {
    ctx.clients.fetch_add(1, Ordering::Relaxed);
    ctx.touch();
    let mut rx = tauri::subscribe_events();
    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                Ok(ev) => {
                    let Ok(text) = serde_json::to_string(&ev) else { continue };
                    if socket.send(Message::Text(text.into())).await.is_err() { break; }
                }
                // Client trop lent : on saute les evenements perdus.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => ctx.touch(),
            },
        }
    }
    ctx.clients.fetch_sub(1, Ordering::Relaxed);
    ctx.touch();
}

async fn shutdown() -> StatusCode {
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(300)).await;
        session::clear();
        std::process::exit(0);
    });
    StatusCode::ACCEPTED
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn parses_options() {
        let o = parse_args(&s(&["--port", "9000", "--no-browser", "--stay"])).unwrap();
        assert_eq!(o.port, 9000);
        assert!(!o.open_browser);
        assert!(o.idle_exit.is_zero());
        assert!(!o.lan);
        let o = parse_args(&s(&["--idle-minutes", "3", "--lan"])).unwrap();
        assert_eq!(o.idle_exit, Duration::from_secs(180));
        assert!(o.lan);
    }

    #[test]
    fn rejects_unknown_options() {
        assert!(parse_args(&s(&["--bogus"])).is_err());
        assert!(parse_args(&s(&["--port"])).is_err());
    }
}
