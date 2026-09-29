//! Equivalents des plugins Tauri utilises par l'interface (dialogues de
//! fichiers, lecture/ecriture de fichiers texte, dossier personnel).
//!
//! Dans l'application native, l'interface appelle directement
//! `@tauri-apps/plugin-dialog` / `plugin-fs`. Dans le panneau web, les cales
//! JavaScript (web/shims) appellent ces points d'entree : la boite « Enregistrer
//! sous » s'ouvre sur le poste ou tourne l'agent, et le fichier y est ecrit.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppCtx;

pub fn routes() -> Router<Arc<AppCtx>> {
    Router::new()
        .route("/host/dialog/open", post(dialog_open))
        .route("/host/dialog/save", post(dialog_save))
        .route("/host/fs/read-text", post(fs_read_text))
        .route("/host/fs/write-text", post(fs_write_text))
        .route("/host/fs/mkdir", post(fs_mkdir))
        .route("/host/path/home", get(path_home))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    pub name: String,
    #[serde(default)]
    pub extensions: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogOptions {
    pub title: Option<String>,
    pub default_path: Option<String>,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default)]
    pub directory: bool,
}

fn build(o: &DialogOptions) -> rfd::FileDialog {
    let mut d = rfd::FileDialog::new();
    if let Some(t) = &o.title {
        d = d.set_title(t);
    }
    if let Some(p) = &o.default_path {
        let path = std::path::Path::new(p);
        if path.is_dir() {
            d = d.set_directory(path);
        } else {
            if let Some(parent) = path.parent().filter(|p| p.is_dir()) {
                d = d.set_directory(parent);
            }
            if let Some(name) = path.file_name() {
                d = d.set_file_name(name.to_string_lossy());
            }
        }
    }
    for f in &o.filters {
        let exts: Vec<&str> = f.extensions.iter().map(String::as_str).collect();
        d = d.add_filter(&f.name, &exts);
    }
    d
}

fn internal(e: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(e.to_string()))).into_response()
}

async fn dialog_open(Json(o): Json<DialogOptions>) -> Response {
    let res = tokio::task::spawn_blocking(move || {
        let d = build(&o);
        let to_s = |p: std::path::PathBuf| p.to_string_lossy().into_owned();
        match (o.directory, o.multiple) {
            (true, true) => d.pick_folders().map(|v| json!(v.into_iter().map(to_s).collect::<Vec<_>>())),
            (true, false) => d.pick_folder().map(|p| json!(to_s(p))),
            (false, true) => d.pick_files().map(|v| json!(v.into_iter().map(to_s).collect::<Vec<_>>())),
            (false, false) => d.pick_file().map(|p| json!(to_s(p))),
        }
        .unwrap_or(Value::Null)
    })
    .await;
    match res {
        Ok(v) => Json(v).into_response(),
        Err(e) => internal(e),
    }
}

async fn dialog_save(Json(o): Json<DialogOptions>) -> Response {
    let res = tokio::task::spawn_blocking(move || build(&o).save_file().map(|p| p.to_string_lossy().into_owned())).await;
    match res {
        Ok(v) => Json(json!(v)).into_response(),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
struct PathArg {
    path: String,
    #[serde(default)]
    recursive: bool,
}

#[derive(Deserialize)]
struct WriteArg {
    path: String,
    contents: String,
}

/// Limite de lecture : les fichiers lus par l'interface sont des exports
/// texte (profils, themes, rapports), jamais des fichiers volumineux.
const MAX_TEXT: u64 = 32 * 1024 * 1024;

async fn fs_read_text(Json(a): Json<PathArg>) -> Response {
    let res = tokio::task::spawn_blocking(move || -> std::io::Result<String> {
        let meta = std::fs::metadata(&a.path)?;
        if meta.len() > MAX_TEXT {
            return Err(std::io::Error::other("fichier trop volumineux (> 32 Mo)"));
        }
        std::fs::read_to_string(&a.path)
    })
    .await;
    match res {
        Ok(Ok(s)) => Json(json!(s)).into_response(),
        Ok(Err(e)) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!(e.to_string()))).into_response(),
        Err(e) => internal(e),
    }
}

async fn fs_write_text(Json(a): Json<WriteArg>) -> Response {
    let res = tokio::task::spawn_blocking(move || std::fs::write(&a.path, a.contents)).await;
    match res {
        Ok(Ok(())) => Json(Value::Null).into_response(),
        Ok(Err(e)) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!(e.to_string()))).into_response(),
        Err(e) => internal(e),
    }
}

async fn fs_mkdir(Json(a): Json<PathArg>) -> Response {
    let res = tokio::task::spawn_blocking(move || {
        if a.recursive { std::fs::create_dir_all(&a.path) } else { std::fs::create_dir(&a.path) }
    })
    .await;
    match res {
        Ok(Ok(())) => Json(Value::Null).into_response(),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::AlreadyExists => Json(Value::Null).into_response(),
        Ok(Err(e)) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!(e.to_string()))).into_response(),
        Err(e) => internal(e),
    }
}

async fn path_home() -> Json<Value> {
    Json(json!(dirs::home_dir().map(|p| p.to_string_lossy().into_owned())))
}
