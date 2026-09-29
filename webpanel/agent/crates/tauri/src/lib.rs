//! Cale « tauri » du panneau web.
//!
//! NiTriTe est ecrit pour Tauri : ses commandes sont des fonctions
//! `#[tauri::command]`, ses progressions passent par `window.emit(...)`, son
//! etat partage par `tauri::State`. Cette cale fournit exactement cette
//! surface, sans fenetre ni WebView :
//!
//! - `#[command]` enregistre chaque fonction dans un registre global
//!   (`inventory`) avec un adaptateur JSON -> arguments -> JSON ;
//! - `Emitter::emit` publie sur un bus d'evenements que le serveur relaie en
//!   WebSocket au navigateur ;
//! - `Builder::run` confie la main au serveur HTTP enregistre par l'agent
//!   (`set_runner`), avec la liste blanche des commandes de
//!   `generate_handler!` — la meme que dans l'application native.

use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::{Arc, OnceLock, RwLock};

use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;

pub use tauri_macros::{command, mobile_entry_point};

// ── Erreur ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

// ── Bus d'evenements ──────────────────────────────────────────────────────────

/// Evenement publie par le backend (`window.emit`), relaye au navigateur.
#[derive(Debug, Clone, Serialize)]
pub struct EventMessage {
    pub event: String,
    pub payload: Value,
}

fn bus() -> &'static broadcast::Sender<EventMessage> {
    static BUS: OnceLock<broadcast::Sender<EventMessage>> = OnceLock::new();
    // 1024 evenements d'avance : un client lent perd les plus anciens
    // (lignes de journal), jamais le serveur ne bloque le backend.
    BUS.get_or_init(|| broadcast::channel(1024).0)
}

/// Abonnement au bus (une souscription par client WebSocket).
pub fn subscribe_events() -> broadcast::Receiver<EventMessage> {
    bus().subscribe()
}

fn publish<S: Serialize>(event: &str, payload: S) -> Result<()> {
    let payload = serde_json::to_value(payload).map_err(|e| Error(e.to_string()))?;
    // Aucun client connecte : l'evenement est simplement perdu, comme dans
    // Tauri quand aucune page n'ecoute.
    let _ = bus().send(EventMessage { event: event.to_string(), payload });
    Ok(())
}

pub trait Emitter {
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) -> Result<()>;
}

// ── Fenetre / application ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Window {
    _private: (),
}

impl Emitter for Window {
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) -> Result<()> {
        publish(event, payload)
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppHandle {
    _private: (),
}

impl Emitter for AppHandle {
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) -> Result<()> {
        publish(event, payload)
    }
}

impl AppHandle {
    /// `cleanup_on_exit` termine l'application : ici, l'agent.
    pub fn exit(&self, code: i32) {
        std::process::exit(code);
    }
}

// ── Etat gere (`Builder::manage` / `tauri::State`) ────────────────────────────

type AnyArc = Arc<dyn Any + Send + Sync>;

fn managed_map() -> &'static RwLock<HashMap<TypeId, AnyArc>> {
    static MAP: OnceLock<RwLock<HashMap<TypeId, AnyArc>>> = OnceLock::new();
    MAP.get_or_init(Default::default)
}

pub struct State<'a, T: Send + Sync + 'static>(&'a T);

impl<'a, T: Send + Sync + 'static> State<'a, T> {
    pub fn inner(&self) -> &'a T {
        self.0
    }
}

impl<T: Send + Sync + 'static> Deref for State<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0
    }
}

impl<T: Send + Sync + 'static> Clone for State<'_, T> {
    fn clone(&self) -> Self {
        State(self.0)
    }
}

// ── Registre des commandes ────────────────────────────────────────────────────

/// Resultat d'une commande : valeur JSON, ou erreur deja serialisee (Tauri
/// rejette la promesse `invoke` avec l'erreur serialisee, idem ici).
pub type CommandResult = std::result::Result<Value, Value>;
pub type CommandFuture = Pin<Box<dyn Future<Output = CommandResult> + Send>>;

pub struct Invoke {
    pub args: Value,
}

pub struct CommandDef {
    pub name: &'static str,
    pub handler: fn(Invoke) -> CommandFuture,
}

inventory::collect!(CommandDef);

/// Liste blanche issue de `generate_handler!`.
pub struct Handler {
    names: Vec<String>,
}

impl Handler {
    pub fn new(paths: &[&str]) -> Self {
        // `stringify!(module::fonction)` -> "module :: fonction" : seul le
        // dernier segment est le nom de commande cote JS.
        let names = paths
            .iter()
            .map(|p| p.rsplit("::").next().unwrap_or(p).trim().to_string())
            .collect();
        Handler { names }
    }
}

fn allowed() -> &'static RwLock<HashSet<String>> {
    static ALLOWED: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
    ALLOWED.get_or_init(Default::default)
}

/// Commandes exposees (enregistrees ET autorisees par `generate_handler!`).
pub fn command_names() -> Vec<&'static str> {
    let allowed = allowed().read().unwrap();
    let mut v: Vec<&'static str> = inventory::iter::<CommandDef>
        .into_iter()
        .map(|c| c.name)
        .filter(|n| allowed.contains(*n))
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

pub enum DispatchError {
    /// Commande inconnue ou absente de `generate_handler!`.
    NotFound,
}

pub fn dispatch(name: &str, args: Value) -> std::result::Result<CommandFuture, DispatchError> {
    if !allowed().read().unwrap().contains(name) {
        return Err(DispatchError::NotFound);
    }
    let def = inventory::iter::<CommandDef>
        .into_iter()
        .find(|c| c.name == name)
        .ok_or(DispatchError::NotFound)?;
    Ok((def.handler)(Invoke { args }))
}

// ── Builder ───────────────────────────────────────────────────────────────────

static RUNNER: OnceLock<fn()> = OnceLock::new();

/// L'agent enregistre ici son serveur avant d'appeler `nitrite_lib::run()`.
pub fn set_runner(f: fn()) {
    let _ = RUNNER.set(f);
}

#[derive(Default)]
pub struct Builder {
    handler: Option<Handler>,
}

impl Builder {
    /// Les plugins natifs (dialogues, shell, updater…) n'ont pas d'equivalent
    /// cote agent : le navigateur et les points d'entree /api/host/* les
    /// remplacent.
    pub fn plugin<P>(self, _plugin: P) -> Self {
        self
    }

    pub fn manage<T: Send + Sync + 'static>(self, state: T) -> Self {
        managed_map().write().unwrap().insert(TypeId::of::<T>(), Arc::new(state));
        self
    }

    pub fn invoke_handler(mut self, handler: Handler) -> Self {
        self.handler = Some(handler);
        self
    }

    pub fn run<C>(self, _context: C) -> Result<()> {
        if let Some(h) = self.handler {
            allowed().write().unwrap().extend(h.names);
        }
        match RUNNER.get() {
            Some(run) => {
                run();
                Ok(())
            }
            None => Err(Error("aucun serveur enregistre (tauri::set_runner)".into())),
        }
    }
}

#[macro_export]
macro_rules! generate_handler {
    ($($cmd:path),* $(,)?) => {
        $crate::Handler::new(&[$(stringify!($cmd)),*])
    };
}

#[macro_export]
macro_rules! generate_context {
    () => {
        ()
    };
}

// ── Utilitaires utilises par le code genere par `#[command]` ─────────────────

#[doc(hidden)]
pub mod __private {
    use super::*;

    pub use inventory;
    pub use serde_json;

    /// Etat gere de type `T`. Panique si l'application ne l'a pas `manage()` —
    /// comme Tauri, ou c'est une erreur de programmation.
    pub fn managed<T: Send + Sync + 'static>() -> Arc<T> {
        let any = managed_map()
            .read()
            .unwrap()
            .get(&TypeId::of::<T>())
            .cloned()
            .unwrap_or_else(|| panic!("etat non gere : {}", std::any::type_name::<T>()));
        any.downcast::<T>().expect("type d'etat incoherent")
    }

    pub fn state_ref<T: Send + Sync + 'static>(t: &T) -> State<'_, T> {
        State(t)
    }

    /// Argument `key` (camelCase, convention Tauri) ; absent -> `null`, ce
    /// qui donne `None` pour un `Option<T>`.
    pub fn arg<T: serde::de::DeserializeOwned>(args: &Value, cmd: &str, key: &str) -> std::result::Result<T, Value> {
        let v = args.get(key).cloned().unwrap_or(Value::Null);
        serde_json::from_value(v).map_err(|e| {
            Value::String(format!("invalid args `{key}` for command `{cmd}`: {e}"))
        })
    }

    pub fn ok<T: Serialize>(v: T) -> CommandResult {
        serde_json::to_value(v).map_err(|e| Value::String(e.to_string()))
    }

    pub fn err<E: Serialize>(e: E) -> CommandResult {
        Err(serde_json::to_value(e).unwrap_or_else(|x| Value::String(x.to_string())))
    }

    pub fn window() -> Window {
        Window::default()
    }

    pub fn app_handle() -> AppHandle {
        AppHandle::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handler_keeps_last_path_segment() {
        let h = Handler::new(&["get_apps", "updater_portable :: portable_maj_verifier"]);
        assert_eq!(h.names, vec!["get_apps", "portable_maj_verifier"]);
    }

    #[tokio::test]
    async fn emit_reaches_subscribers() {
        let mut rx = subscribe_events();
        Window::default().emit("install-log", serde_json::json!({ "line": "ok" })).unwrap();
        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.event, "install-log");
        assert_eq!(msg.payload["line"], "ok");
    }

    #[test]
    fn missing_optional_arg_is_none() {
        let v: Option<String> = __private::arg(&serde_json::json!({}), "c", "appId").unwrap();
        assert!(v.is_none());
        let e = __private::arg::<String>(&serde_json::json!({}), "c", "appId").unwrap_err();
        assert!(e.as_str().unwrap().contains("appId"));
    }
}
