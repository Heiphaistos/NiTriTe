//! Port de TokaISO 7.9 (MIT, Cédrick Roy)
//! Catalogue embarqué (402 fiches, 845 sources) + téléchargeur ISO.
//!
//! Sécurité : le frontend ne transmet jamais d'URL à télécharger, seulement un
//! identifiant de fiche et l'index de sa source. Seules les fiches personnelles
//! portent des adresses saisies par l'utilisateur, validées (HTTPS, hôte public).

mod catalog;
mod common;
mod files;
mod library;
mod sources;
mod tools;
mod transfers;

use common::*;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};

pub struct Engine {
    home: PathBuf,
    catalog: Mutex<catalog::CatalogueStore>,
    items: RwLock<Arc<Vec<Value>>>,
    sources: Arc<sources::SourceHealth>,
    library: library::MediaLibrary,
    checklists: tools::Checklists,
    queue: Mutex<transfers::Queue>,
    wake: Condvar,
    rate: Mutex<transfers::Rate>,
    /// Émetteur de l'événement `iso-downloads` (fixé par la commande Tauri).
    emit: OnceLock<Box<dyn Fn(&Value) + Send + Sync>>,
}

static ENGINE: OnceLock<Result<Arc<Engine>, String>> = OnceLock::new();

impl Engine {
    fn start() -> Res<Arc<Engine>> {
        Self::start_in(crate::utils::paths::config_dir().join("iso_catalog"))
    }

    fn start_in(home: PathBuf) -> Res<Arc<Engine>> {
        std::fs::create_dir_all(&home).map_err(|e| format!("Dossier de données inaccessible : {e}"))?;
        let store = catalog::CatalogueStore::new(&home)?;
        let items = store.active();
        let engine = Arc::new(Engine {
            sources: Arc::new(sources::SourceHealth::new(&home, &items)),
            library: library::MediaLibrary::new(&home, &items),
            checklists: tools::Checklists::new(&home),
            catalog: Mutex::new(store),
            items: RwLock::new(Arc::new(items)),
            queue: Mutex::new(transfers::Queue::default()),
            wake: Condvar::new(),
            rate: Mutex::new(transfers::Rate::default()),
            emit: OnceLock::new(),
            home,
        });
        engine.restore_queue();
        // Deux ouvriers comme TokaISO + un répartiteur pour les créneaux horaires.
        // Ils s'arrêtent avec le processus ; les .part restent pour la reprise.
        for _ in 0..2 {
            let me = Arc::clone(&engine);
            std::thread::spawn(move || me.worker());
        }
        let me = Arc::clone(&engine);
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            me.dispatch_tick();
        });
        Ok(engine)
    }

    fn items(&self) -> Arc<Vec<Value>> {
        Arc::clone(&self.items.read().unwrap_or_else(|p| p.into_inner()))
    }

    fn refresh_items(&self, store: &catalog::CatalogueStore) -> Vec<Value> {
        let items = store.active();
        self.library.catalogue_changed(&items);
        *self.items.write().unwrap_or_else(|p| p.into_inner()) = Arc::new(items.clone());
        items
    }

    fn item(&self, payload: &Value) -> Res<Value> {
        let id = str_of(payload, "item");
        self.items().iter().find(|i| str_of(i, "id") == id).cloned().ok_or_else(|| "Choisis une image du catalogue.".into())
    }

    fn source_index(payload: &Value) -> Option<usize> {
        payload.get("source").and_then(Value::as_u64).map(|i| i as usize)
    }

    fn catalog_action(&self, action: &str, payload: &Value) -> Res<Value> {
        let mut store = self.catalog.lock().unwrap_or_else(|p| p.into_inner());
        let mut result = match action {
            "catalog-status" => return Ok(store.status()),
            "catalog-preview" => return store.preview(payload),
            "catalog-apply" => store.change(true)?,
            "catalog-rollback" => store.change(false)?,
            "personal-export" => {
                let target = tools::output_path(str_of(payload, "path"))?;
                json_write(&target, &store.personal_export())?;
                return Ok(json!({"path": display_path(&target)}));
            }
            "personal-save" => store.personal_action("save", payload)?,
            "personal-delete" => store.personal_action("delete", payload)?,
            "personal-import" => store.personal_action("import", payload)?,
            _ => return Err("Commande de catalogue inconnue.".into()),
        };
        result["items"] = json!(self.refresh_items(&store));
        Ok(result)
    }

    fn act(&self, action: &str, payload: &Value) -> Res<Value> {
        match action {
            "catalog-data" => {
                let status = self.catalog.lock().unwrap_or_else(|p| p.into_inner()).status();
                Ok(json!({"items": *self.items(), "status": status, "profile": tools::machine_profile()}))
            }
            a if a.starts_with("catalog-") || a.starts_with("personal-") => self.catalog_action(a, payload),
            "snapshot" => Ok(self.snapshot()),
            "add" => self.add_jobs(std::slice::from_ref(payload), None),
            "batch" => {
                let items = payload.get("items").and_then(Value::as_array).ok_or("Sélectionne de 1 à 24 fichiers.")?;
                self.add_jobs(items, Some(str_of(payload, "name")))
            }
            "folder" | "bandwidth" | "schedule" | "favorite" | "clear" | "pause" | "resume" | "cancel" | "prioritize" | "switch-mirror" => {
                self.queue_action(action, payload)
            }
            "source-test" => self.sources.test(item_source(&self.item(payload)?, payload)?),
            "source-mirrors" => {
                let item = self.item(payload)?;
                self.sources.mirrors(&item, Self::source_index(payload).ok_or("Source introuvable.")?)
            }
            "source-item" => Ok(self.sources.item_checks(&self.item(payload)?)),
            "source-dashboard" => Ok(self.sources.dashboard(&self.items(), payload)),
            "source-overview" => Ok(self.sources.overview(&self.items())),
            "source-scan-start" => {
                let items = self.items();
                self.sources.start_scan(&items, str_of(payload, "family"))?;
                Ok(self.sources.overview(&items))
            }
            "source-scan-stop" => {
                self.sources.stop_scan();
                Ok(self.sources.overview(&self.items()))
            }
            "library-list" => Ok(json!({"files": self.library.list()})),
            "library-add" => self.library.add(str_of(payload, "path")),
            "library-remove" => self.library.remove(str_of(payload, "id")),
            "library-reusable" => {
                let item = self.item(payload)?;
                let found: Vec<usize> = sources_of(&item).iter().enumerate()
                    .filter(|(_, s)| self.library.find_verified(s, false).is_some()).map(|(i, _)| i).collect();
                Ok(json!({"sources": found}))
            }
            "library-reuse" => {
                let item = self.item(payload)?;
                let file = self.library.find_verified(item_source(&item, payload)?, true).ok_or("Réindexe le fichier local avant de le réutiliser.")?;
                Ok(json!({"file": file}))
            }
            "media-verify" => tools::verify_file(&self.items(), payload),
            "media-manifest" => tools::verify_manifest(payload, &self.home),
            "deployment-compatibility" => {
                let item = self.item(payload)?;
                let folder = PathBuf::from(&self.queue().folder);
                tools::compatibility(&item, Self::source_index(payload).ok_or("Choisis une image du catalogue.")?, &folder, payload.get("path"))
            }
            "advisor" => tools::advisor(&self.items(), payload),
            "checklist-export" => {
                self.checklists.export(str_of(payload, "id"), str_of(payload, "path"))?;
                Ok(json!({"ok": true}))
            }
            a if a.starts_with("checklist-") => self.checklists.action(&a["checklist-".len()..], payload),
            _ => Err("Commande inconnue.".into()),
        }
    }
}

fn item_source<'a>(item: &'a Value, payload: &Value) -> Res<&'a Value> {
    source_at(item, Engine::source_index(payload))
}

/// Point d'entrée unique du module (même découpage par action que l'API de TokaISO).
/// La progression des téléchargements est émise par l'événement `iso-downloads`.
#[tauri::command]
pub async fn iso_catalog_action(app: tauri::AppHandle, action: String, payload: Option<Value>) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || {
        let engine = ENGINE.get_or_init(Engine::start).clone()?;
        let _ = engine.emit.set(Box::new(move |snapshot| {
            use tauri::Emitter;
            let _ = app.emit("iso-downloads", snapshot);
        }));
        engine.act(&action, &payload.unwrap_or_else(|| json!({})))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vrai téléchargement (réseau requis) : `cargo test --lib iso_catalog -- --ignored`.
    /// wimboot (76 Ko, SHA-256 publié) puis le kit Office (ODT officiel + XML).
    #[test]
    #[ignore]
    fn real_downloads() {
        let home = std::env::temp_dir().join(format!("nitrite-iso-e2e-{}", uuid::Uuid::new_v4().simple()));
        let engine = Engine::start_in(home.clone()).unwrap();
        let folder = home.join("dl");
        std::fs::create_dir_all(&folder).unwrap();
        engine.act("folder", &json!({"folder": folder.to_string_lossy()})).unwrap();
        engine.act("add", &json!({"item": "pxe-wimboot", "source": 0})).unwrap();
        engine.act("add", &json!({"item": "150", "source": 0})).unwrap();
        assert!(engine.act("add", &json!({"item": "0", "source": 99})).is_err());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        let jobs = loop {
            let snap = engine.snapshot();
            let jobs = snap["jobs"].as_array().cloned().unwrap_or_default();
            if jobs.iter().all(|j| ["done", "failed"].contains(&str_of(j, "state"))) || std::time::Instant::now() > deadline {
                break jobs;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        };
        for job in &jobs {
            assert_eq!(str_of(job, "state"), "done", "{job}");
            assert!(folder.join(str_of(job, "file")).is_file());
        }
        let wimboot = jobs.iter().find(|j| str_of(j, "item") == "pxe-wimboot").unwrap();
        assert_eq!(str_of(wimboot, "verification"), "verified");
        let kit = jobs.iter().find(|j| str_of(j, "item") == "150").unwrap();
        assert_eq!(str_of(kit, "file"), "Microsoft365-NiTriTe.zip");

        // Pause puis reprise par Range (débit bridé à 512 Ko/s) : le SHA-256 final prouve la reprise.
        engine.act("bandwidth", &json!({"limit": 512})).unwrap();
        let snap = engine.act("add", &json!({"item": "rufus-portable", "source": 0})).unwrap();
        let id = str_of(&snap["jobs"][0], "id").to_string();
        let wait = |id: &str, cond: &dyn Fn(&transfers::Job) -> bool| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
            while std::time::Instant::now() < deadline {
                if engine.queue().jobs.iter().find(|j| j.id == id).is_some_and(cond) {
                    return true;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            false
        };
        assert!(wait(&id, &|j| j.received > 300_000));
        engine.act("pause", &json!({"id": id})).unwrap();
        assert!(wait(&id, &|j| !j.busy && j.state == "paused"));
        let paused = engine.queue().jobs.iter().find(|j| j.id == id).cloned().unwrap();
        assert!(paused.partial().metadata().unwrap().len() > 300_000);
        assert!(!paused.validator.is_empty(), "le serveur doit fournir ETag/Last-Modified");
        engine.act("bandwidth", &json!({"limit": 0})).unwrap();
        engine.act("resume", &json!({"id": id})).unwrap();
        assert!(wait(&id, &|j| j.state == "done" || j.state == "failed"));
        let done = engine.queue().jobs.iter().find(|j| j.id == id).cloned().unwrap();
        assert_eq!(done.state, "done", "{}", done.error);
        assert_eq!(done.verification, "verified");

        // Fiche personnelle à l'empreinte fausse : fichier conservé et signalé, jamais supprimé.
        let saved = engine.act("personal-save", &json!({"name": "Test empreinte", "sha256": "0".repeat(64),
            "links": ["https://github.com/ipxe/wimboot/releases/download/v2.9.0/wimboot"]})).unwrap();
        let personal = str_of(&saved["personal"][0], "id").to_string();
        let snap = engine.act("add", &json!({"item": personal, "source": 0})).unwrap();
        let id = str_of(&snap["jobs"][0], "id").to_string();
        assert!(wait(&id, &|j| j.state == "done" || j.state == "failed"));
        let bad = engine.queue().jobs.iter().find(|j| j.id == id).cloned().unwrap();
        assert_eq!((bad.state.as_str(), bad.verification.as_str()), ("done", "mismatch"));
        assert!(folder.join(&bad.file).is_file());
        let _ = std::fs::remove_dir_all(home);
    }
}
