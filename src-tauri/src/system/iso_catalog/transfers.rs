//! Port de TokaISO 7.9 (MIT, Cédrick Roy)
//! File de téléchargement (classe Transfers de TokaISO.py) : priorités, reprise par
//! en-tête Range, bascule sur miroir de même empreinte, limite de débit, créneaux,
//! manifeste de trousse, vérification SHA-256 et kit Office (outil officiel + XML).

use super::common::*;
use super::files::{filename_from, office_kit};
use super::sources::{download_agent, iso_marker, same_file};
use super::tools::{in_window, schedule_data};
use super::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const TERMINAL: [&str; 3] = ["done", "failed", "cancelled"];
const LIMITS: [u64; 6] = [0, 512, 1024, 2048, 5120, 10240];
const CHUNK: usize = 256 * 1024;

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Mirror {
    pub url: String,
    pub source: String,
    pub source_index: usize,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Job {
    pub id: String,
    pub item: String,
    pub name: String,
    pub url: String,
    pub requested_url: String,
    pub source_index: usize,
    pub kit: String,
    pub source: String,
    pub kind: String,
    pub format: String,
    pub verified: String,
    pub preferred_filename: String,
    pub language: String,
    pub arch: String,
    pub expected_sha256: String,
    pub expected_size: u64,
    pub mirrors: Vec<Mirror>,
    pub file: String,
    pub state: String,
    pub busy: bool,
    pub received: u64,
    pub total: u64,
    pub speed: u64,
    pub error: String,
    pub folder: String,
    pub group: String,
    pub validator: String,
    pub cross_mirror: bool,
    pub mirror_used: bool,
    pub priority: bool,
    pub retries: u32,
    pub retry_at: f64,
    pub deferred: bool,
    pub ticket: u64,
    pub sha256: String,
    /// "verified", "mismatch" ou vide (empreinte inconnue).
    pub verification: String,
    pub personal: bool,
}

impl Job {
    pub(super) fn partial(&self) -> PathBuf {
        Path::new(&self.folder).join(format!(".nitrite-{}.part", self.id))
    }
    fn proof(&self) -> Value {
        json!({"sha256": self.expected_sha256, "size": self.expected_size, "kind": if self.kit.is_empty() { "official" } else { "kit" }})
    }
}

#[derive(Default)]
pub struct Queue {
    pub folder: String,
    pub jobs: Vec<Job>,
    pub bundles: Map<String, Value>,
    pub favorites: BTreeSet<String>,
    pub limit_kib: u64,
    pub schedule: Value,
    pending: BinaryHeap<Reverse<(bool, u64, String)>>,
    ticket: u64,
}

impl Queue {
    fn job(&mut self, id: &str) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }
}

pub struct Rate {
    next_chunk: Instant,
}

impl Default for Rate {
    fn default() -> Self {
        Self { next_chunk: Instant::now() }
    }
}

enum Failure {
    Http(u16),
    Net(String),
    Invalid(String),
    Fatal(String),
}

impl Failure {
    fn message(&self) -> String {
        match self {
            Failure::Http(code) => format!("Erreur HTTP {code}"),
            Failure::Net(m) | Failure::Invalid(m) | Failure::Fatal(m) => m.clone(),
        }
    }
    fn transient(&self) -> bool {
        match self {
            Failure::Http(code) => matches!(code, 408 | 425 | 429) || (500..600).contains(code),
            Failure::Net(_) => true,
            _ => false,
        }
    }
}

fn net(e: impl std::fmt::Display) -> Failure {
    Failure::Net(format!("Erreur réseau : {e}"))
}

fn fatal(e: impl std::fmt::Display) -> Failure {
    Failure::Fatal(e.to_string())
}

pub fn default_folder() -> String {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("C:\\")).join("Downloads").join("NiTriTe-ISO").to_string_lossy().to_string()
}

fn estimated_seconds(jobs: &[Job]) -> Option<u64> {
    let active: Vec<&Job> = jobs.iter().filter(|j| j.state == "running" || j.state == "queued").collect();
    let rate: u64 = active.iter().filter(|j| j.state == "running").map(|j| j.speed).sum();
    if rate == 0 || active.iter().any(|j| j.total == 0) {
        return None;
    }
    Some(active.iter().map(|j| j.total.saturating_sub(j.received)).sum::<u64>() / rate)
}

impl Engine {
    pub(super) fn queue(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn restore_queue(&self) {
        let saved = json_read(&self.home.join("downloads.json")).unwrap_or(Value::Null);
        let mut q = self.queue();
        q.folder = Some(str_of(&saved, "folder")).filter(|f| !f.is_empty()).map(str::to_string).unwrap_or_else(default_folder);
        q.jobs = saved.get("jobs").cloned().and_then(|j| serde_json::from_value(j).ok()).unwrap_or_default();
        q.bundles = saved.get("bundles").and_then(Value::as_object).cloned().unwrap_or_default();
        q.favorites = saved.get("favorites").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
        q.limit_kib = u64_of(&saved, "limit_kib");
        q.schedule = saved.get("schedule").and_then(|s| schedule_data(s).ok()).unwrap_or_else(super::tools::default_schedule);
        let enabled = q.schedule["enabled"] == json!(true);
        for i in 0..q.jobs.len() {
            let job = &mut q.jobs[i];
            job.busy = false;
            job.speed = 0;
            if !TERMINAL.contains(&job.state.as_str()) {
                let automatic = job.state != "paused" && (enabled || job.retry_at > 0.0);
                job.state = if automatic { "queued" } else { "paused" }.into();
                job.deferred = automatic;
                job.received = job.partial().metadata().map(|m| m.len()).unwrap_or(0);
                let job = job.clone();
                Self::update_manifest(&mut q, &job, true);
            }
        }
    }

    fn save_queue(q: &Queue, home: &Path) {
        let data = json!({"folder": q.folder, "jobs": q.jobs, "bundles": q.bundles, "favorites": q.favorites,
                          "limit_kib": q.limit_kib, "schedule": q.schedule});
        if let Err(e) = json_write(&home.join("downloads.json"), &data) {
            tracing::warn!("iso_catalog: sauvegarde de la file impossible : {e}");
        }
    }

    pub fn snapshot_of(q: &Queue) -> Value {
        let jobs: Vec<Value> = q.jobs.iter().rev().map(|job| {
            let mut row = serde_json::to_value(job).unwrap_or(json!({}));
            if let Some(m) = row.as_object_mut() {
                for key in ["url", "validator", "mirrors"] {
                    m.remove(key);
                }
                m.insert("mirror_count".into(), json!(job.mirrors.len()));
            }
            row
        }).collect();
        json!({"folder": q.folder, "favorites": q.favorites, "limit_kib": q.limit_kib, "schedule": q.schedule,
               "schedule_open": in_window(&q.schedule), "eta": estimated_seconds(&q.jobs), "jobs": jobs})
    }

    pub fn snapshot(&self) -> Value {
        Self::snapshot_of(&self.queue())
    }

    fn publish(&self, q: &Queue) {
        if let Some(emit) = self.emit.get() {
            emit(&Self::snapshot_of(q));
        }
    }

    fn write_manifest(bundle: &Value) -> Res<()> {
        let mut data = bundle.clone();
        data["files"] = json!(bundle["files"].as_object().map(|m| m.values().cloned().collect::<Vec<_>>()).unwrap_or_default());
        json_write(&Path::new(str_of(bundle, "folder")).join("NiTriTe-ISO-manifeste.json"), &data)
    }

    fn update_manifest(q: &mut Queue, job: &Job, write: bool) {
        let Some(bundle) = q.bundles.get_mut(&job.group) else { return };
        bundle["files"][&job.id] = json!({"name": job.name, "file": job.file, "source": job.source, "url": job.url, "format": job.format,
            "language": job.language, "arch": job.arch, "verified": job.verified, "state": job.state,
            "expected_sha256": job.expected_sha256, "sha256": job.sha256, "verification": job.verification});
        if write {
            if let Err(e) = Self::write_manifest(bundle) {
                tracing::warn!("iso_catalog: manifeste : {e}");
            }
        }
    }

    fn enqueue(q: &mut Queue, id: &str) {
        let open = in_window(&q.schedule);
        q.ticket += 1;
        let ticket = q.ticket;
        let Some(job) = q.job(id) else { return };
        if job.busy || !open || job.retry_at > now_secs() {
            job.deferred = true;
            return;
        }
        job.deferred = false;
        job.ticket = ticket;
        let priority = job.priority;
        q.pending.push(Reverse((!priority, ticket, id.to_string())));
    }

    /// Ajoute 1 à 24 fichiers ; avec `batch`, crée un dossier de trousse et son manifeste.
    pub fn add_jobs(&self, selection: &[Value], batch: Option<&str>) -> Res<Value> {
        if selection.is_empty() || selection.len() > 24 {
            return Err("Sélectionne de 1 à 24 fichiers.".into());
        }
        let items = self.items();
        let mut new_jobs = Vec::new();
        for choice in selection {
            let item = items.iter().find(|i| str_of(i, "id") == str_of(choice, "item")).ok_or("Fiche introuvable.")?;
            let index = choice.get("source").and_then(Value::as_u64).map(|i| i as usize);
            let source = source_at(item, index).map_err(|_| "Source inconnue.".to_string())?;
            if str_of(source, "kind") == "portal" {
                return Err("Ce lien s’ouvre sur le portail de l’éditeur.".into());
            }
            let all = sources_of(item);
            let mirrors = all.iter().enumerate()
                .filter(|(_, o)| str_of(o, "url") != str_of(source, "url") && same_file(source, o))
                .map(|(p, o)| Mirror { url: str_of(o, "url").into(), source: str_of(o, "label").into(), source_index: p }).collect();
            new_jobs.push(Job {
                id: uuid::Uuid::new_v4().simple().to_string()[..24].to_string(), item: str_of(item, "id").into(), name: str_of(item, "name").into(),
                url: str_of(source, "url").into(), requested_url: str_of(source, "url").into(), source_index: index.unwrap_or(0),
                kit: str_of(source, "kit").into(), source: str_of(source, "label").into(), kind: str_of(source, "kind").into(),
                format: str_of(source, "format").into(), verified: str_of(source, "verified").into(),
                preferred_filename: if item.get("vm").is_some() { str_of(source, "filename").into() } else { String::new() },
                language: str_of(source, "language").into(), arch: str_of(source, "arch").into(),
                expected_sha256: str_of(source, "sha256").to_lowercase(), expected_size: u64_of(source, "size"),
                mirrors, state: "queued".into(), total: u64_of(source, "size"), personal: item.get("personal") == Some(&json!(true)),
                ..Job::default()
            });
        }
        let keys: BTreeSet<String> = new_jobs.iter().map(|j| format!("{}#{}", j.url, j.kit)).collect();
        if keys.len() != new_jobs.len() {
            return Err("La sélection contient le même fichier plusieurs fois.".into());
        }
        let mut q = self.queue();
        let mut folder = PathBuf::from(&q.folder);
        let mut group = String::new();
        if let Some(name) = batch {
            if name.trim().is_empty() {
                return Err("Donne un nom à la trousse.".into());
            }
            let clean: String = name.chars().take(48).map(|c| if c.is_alphanumeric() || " ._-".contains(c) { c } else { '-' }).collect();
            let clean = Some(clean.trim_matches(|c| c == ' ' || c == '.' || c == '-').to_string()).filter(|s| !s.is_empty()).unwrap_or("Ma trousse".into());
            group = uuid::Uuid::new_v4().simple().to_string()[..6].to_string();
            folder = folder.join(format!("Trousse - {clean} - {group}"));
            std::fs::create_dir_all(&folder).map_err(|e| format!("Création du dossier impossible : {e}"))?;
            q.bundles.insert(group.clone(), json!({"name": clean, "folder": folder.to_string_lossy(), "created": now_iso(), "files": {}}));
        }
        for job in &mut new_jobs {
            job.folder = folder.to_string_lossy().to_string();
            job.group = group.clone();
        }
        for job in &new_jobs {
            q.jobs.push(job.clone());
            Self::update_manifest(&mut q, job, false);
        }
        if let Some(bundle) = q.bundles.get(&group) {
            if let Err(e) = Self::write_manifest(bundle) {
                q.jobs.retain(|j| !new_jobs.iter().any(|n| n.id == j.id));
                q.bundles.remove(&group);
                return Err(e);
            }
        }
        for job in &new_jobs {
            Self::enqueue(&mut q, &job.id);
        }
        Self::save_queue(&q, &self.home);
        self.wake.notify_all();
        self.publish(&q);
        Ok(Self::snapshot_of(&q))
    }

    fn switch_mirror(&self, q: &mut Queue, id: &str, index: Option<usize>) -> Res<()> {
        let items = self.items();
        let job = q.job(id).ok_or("Tâche introuvable.")?;
        let choices = items.iter().find(|i| str_of(i, "id") == job.item).map(sources_of).unwrap_or(&[]);
        let original = choices.iter().find(|s| str_of(s, "url") == job.requested_url);
        let mirror = index.and_then(|i| choices.get(i));
        let (Some(original), Some(mirror), Some(index)) = (original, mirror, index) else {
            return Err("Ce miroir ne fournit pas la même empreinte et la même taille.".into());
        };
        if !same_file(original, mirror) || !same_file(&job.proof(), mirror) {
            return Err("Ce miroir ne fournit pas la même empreinte et la même taille.".into());
        }
        if str_of(mirror, "url") != job.url {
            job.cross_mirror = job.partial().is_file();
            job.url = str_of(mirror, "url").into();
            job.source = str_of(mirror, "label").into();
            job.source_index = index;
            job.validator.clear();
            job.mirror_used = true;
            job.mirrors = choices.iter().enumerate().filter(|(p, s)| *p != index && same_file(original, s))
                .map(|(p, s)| Mirror { url: str_of(s, "url").into(), source: str_of(s, "label").into(), source_index: p }).collect();
        }
        job.state = "queued".into();
        job.error.clear();
        job.retry_at = 0.0;
        Self::enqueue(q, id);
        Ok(())
    }

    /// Actions de la file et réglages (port de `Transfers.act`).
    pub fn queue_action(&self, action: &str, payload: &Value) -> Res<Value> {
        let mut q = self.queue();
        match action {
            "folder" => {
                let target = Path::new(str_of(payload, "folder"));
                if !target.is_absolute() || !target.is_dir() {
                    return Err("Dossier de téléchargement introuvable.".into());
                }
                q.folder = display_path(&target.canonicalize().map_err(|e| e.to_string())?);
            }
            "bandwidth" => {
                let limit = payload.get("limit").and_then(Value::as_u64).filter(|l| LIMITS.contains(l)).ok_or("Limite de débit invalide.")?;
                q.limit_kib = limit;
                self.rate.lock().unwrap_or_else(|p| p.into_inner()).next_chunk = Instant::now();
            }
            "schedule" => {
                q.schedule = schedule_data(payload)?;
                if q.schedule["retry"] != json!(true) {
                    for job in q.jobs.iter_mut().filter(|j| j.retry_at > 0.0 && j.state == "queued") {
                        job.state = "failed".into();
                        job.retry_at = 0.0;
                        job.deferred = false;
                    }
                }
            }
            "favorite" => {
                let item = str_of(payload, "item").to_string();
                if !self.items().iter().any(|i| str_of(i, "id") == item) {
                    return Err("Fiche introuvable.".into());
                }
                if !q.favorites.remove(&item) {
                    q.favorites.insert(item);
                }
            }
            "clear" => {
                let (done, kept): (Vec<Job>, Vec<Job>) = q.jobs.drain(..).partition(|j| TERMINAL.contains(&j.state.as_str()) && !j.busy);
                q.jobs = kept;
                for job in done {
                    let _ = std::fs::remove_file(job.partial());
                    Self::update_manifest(&mut q, &job, true);
                }
            }
            _ => {
                let id = str_of(payload, "id").to_string();
                let job = q.job(&id).ok_or("Tâche introuvable.")?;
                let (state, busy) = (job.state.clone(), job.busy);
                match action {
                    "prioritize" if state == "queued" => {
                        job.priority = !job.priority;
                        Self::enqueue(&mut q, &id);
                    }
                    "prioritize" => return Err("Choisis un téléchargement encore en attente.".into()),
                    "pause" if state == "running" || state == "queued" => {
                        job.state = "paused".into();
                        job.speed = 0;
                        job.retry_at = 0.0;
                        job.deferred = false;
                    }
                    "cancel" if state != "done" && state != "cancelled" => {
                        job.state = "cancelled".into();
                        job.speed = 0;
                        if !busy {
                            let _ = std::fs::remove_file(job.partial());
                        }
                    }
                    "resume" if (state == "paused" || state == "failed") && !busy => {
                        job.state = "queued".into();
                        job.error.clear();
                        job.retry_at = 0.0;
                        job.retries = 0;
                        Self::enqueue(&mut q, &id);
                    }
                    "switch-mirror" if ["paused", "failed", "queued"].contains(&state.as_str()) && !busy => {
                        let index = payload.get("source").and_then(Value::as_u64).map(|i| i as usize);
                        self.switch_mirror(&mut q, &id, index)?;
                    }
                    _ => return Err("Cette action est indisponible pour cette tâche.".into()),
                }
                if let Some(job) = q.job(&id).cloned() {
                    Self::update_manifest(&mut q, &job, true);
                }
            }
        }
        Self::save_queue(&q, &self.home);
        self.wake.notify_all();
        self.publish(&q);
        Ok(Self::snapshot_of(&q))
    }

    /// Remet en file les tâches différées quand le créneau s'ouvre, met en attente sinon.
    pub fn dispatch_tick(&self) {
        let mut q = self.queue();
        let usable = in_window(&q.schedule);
        let now = now_secs();
        let mut ready = Vec::new();
        let mut changed = false;
        for job in q.jobs.iter_mut() {
            if !usable && job.state == "running" {
                job.state = "queued".into();
                job.deferred = true;
                job.speed = 0;
                changed = true;
            } else if usable && job.state == "queued" && job.deferred && !job.busy && job.retry_at <= now {
                ready.push(job.id.clone());
            }
        }
        for id in &ready {
            Self::enqueue(&mut q, id);
        }
        if changed || !ready.is_empty() {
            self.wake.notify_all();
            self.publish(&q);
        }
    }

    /// Boucle d'un ouvrier : prend la tâche la plus prioritaire et la transfère.
    pub fn worker(&self) {
        loop {
            let id = {
                let mut q = self.queue();
                let next = loop {
                    match q.pending.pop() {
                        Some(Reverse((_, ticket, id))) => break (ticket, id),
                        None => q = self.wake.wait(q).unwrap_or_else(|p| p.into_inner()),
                    }
                };
                let (ticket, id) = next;
                let open = in_window(&q.schedule);
                let Some(job) = q.job(&id) else { continue };
                if job.state != "queued" || job.ticket != ticket || job.busy {
                    continue;
                }
                if !open || job.retry_at > now_secs() {
                    job.deferred = true;
                    continue;
                }
                job.state = "running".into();
                job.busy = true;
                job.deferred = false;
                job.retry_at = 0.0;
                let job = job.clone();
                Self::update_manifest(&mut q, &job, true);
                self.publish(&q);
                id
            };
            let outcome = self.transfer(&id);
            let mut q = self.queue();
            let retry = q.schedule["retry"] == json!(true);
            if let Some(job) = q.job(&id) {
                if let Err(failure) = outcome {
                    if job.state == "running" {
                        if retry && failure.transient() && job.retries < 8 {
                            job.retry_at = now_secs() + (15.0 * 2f64.powi(job.retries as i32)).min(180.0);
                            job.retries += 1;
                            job.state = "queued".into();
                            job.deferred = true;
                        } else {
                            job.state = "failed".into();
                        }
                        job.error = failure.message();
                    }
                }
                job.busy = false;
                job.speed = 0;
                if job.state == "cancelled" {
                    let _ = std::fs::remove_file(job.partial());
                }
                let job = job.clone();
                Self::update_manifest(&mut q, &job, true);
            }
            Self::save_queue(&q, &self.home);
            self.publish(&q);
        }
    }

    fn state_of(&self, id: &str) -> String {
        self.queue().job(id).map(|j| j.state.clone()).unwrap_or_default()
    }

    fn running(&self, id: &str) -> bool {
        self.state_of(id) == "running"
    }

    /// Essaie la source principale puis les miroirs classés (3 tentatives chacun).
    fn transfer(&self, id: &str) -> Result<(), Failure> {
        let (primary, proof, item_id, folder) = {
            let mut q = self.queue();
            let job = q.job(id).ok_or_else(|| fatal("Tâche introuvable."))?;
            (Mirror { url: job.url.clone(), source: job.source.clone(), source_index: job.source_index }, job.proof(), job.item.clone(), job.folder.clone())
        };
        std::fs::create_dir_all(&folder).map_err(|e| fatal(format!("Dossier de téléchargement inaccessible : {e}")))?;
        let current: Vec<Value> = self.items().iter().find(|i| str_of(i, "id") == item_id).map(|i| sources_of(i).to_vec()).unwrap_or_default();
        let mut mirrors: Vec<Value> = current.iter().enumerate()
            .filter(|(_, s)| str_of(s, "url") != primary.url && same_file(&proof, s))
            .map(|(i, s)| json!({"url": str_of(s, "url"), "source": str_of(s, "label"), "source_index": i})).collect();
        self.sources.rank_mirrors(&mut mirrors);
        let ranked: Vec<Mirror> = mirrors.into_iter().filter_map(|m| serde_json::from_value(m).ok()).collect();
        let candidates: Vec<Mirror> = std::iter::once(primary).chain(ranked.iter().cloned()).collect();
        {
            let mut q = self.queue();
            if let Some(job) = q.job(id) {
                job.mirrors = ranked;
            }
        }
        let http = super::sources::client(20).map_err(fatal)?;
        for (index, candidate) in candidates.iter().enumerate() {
            for attempt in 0..3 {
                {
                    let mut q = self.queue();
                    let job = q.job(id).ok_or_else(|| fatal("Tâche introuvable."))?;
                    if job.state != "running" {
                        return Ok(());
                    }
                    if job.url != candidate.url {
                        let _ = std::fs::remove_file(job.partial());
                        job.url = candidate.url.clone();
                        job.source = candidate.source.clone();
                        job.source_index = candidate.source_index;
                        job.validator.clear();
                        job.received = 0;
                        job.mirror_used = true;
                        job.cross_mirror = false;
                        job.mirrors = candidates.iter().filter(|c| c.url != candidate.url).cloned().collect();
                        Self::save_queue(&q, &self.home);
                    }
                }
                match self.receive(&http, id) {
                    Ok(Some((latency, rate))) => {
                        self.sources.record_transfer(&candidate.url, latency, rate, false);
                        return Ok(());
                    }
                    Ok(None) => return Ok(()),
                    Err(Failure::Fatal(m)) => return Err(Failure::Fatal(m)),
                    Err(failure) => {
                        if matches!(failure, Failure::Http(416)) {
                            if let Some(job) = self.queue().job(id) {
                                let _ = std::fs::remove_file(job.partial());
                            }
                        }
                        let permanent = matches!(failure, Failure::Invalid(_))
                            || matches!(failure, Failure::Http(code) if code < 500 && ![408, 416, 429].contains(&code));
                        if permanent || attempt == 2 {
                            if self.running(id) {
                                self.sources.record_transfer(&candidate.url, 0, 0, true);
                            }
                            if index == candidates.len() - 1 {
                                return Err(failure);
                            }
                            break;
                        }
                        std::thread::sleep(Duration::from_secs(attempt + 1));
                    }
                }
            }
        }
        Ok(())
    }

    fn throttle(&self, id: &str, chunk: usize, limit_kib: u64) {
        if limit_kib == 0 {
            return;
        }
        let deadline = {
            let mut rate = self.rate.lock().unwrap_or_else(|p| p.into_inner());
            let now = Instant::now();
            rate.next_chunk = rate.next_chunk.max(now) + Duration::from_secs_f64(chunk as f64 / (limit_kib * 1024) as f64);
            rate.next_chunk
        };
        while Instant::now() < deadline && self.running(id) {
            std::thread::sleep((deadline - Instant::now()).min(Duration::from_millis(100)));
        }
    }

    /// Un transfert : reprise Range/If-Range, contrôles de type, écriture du .part,
    /// SHA-256 final, kit Office, renommage. Retourne (latence ms, débit) si terminé.
    fn receive(&self, http: &reqwest::blocking::Client, id: &str) -> Result<Option<(u64, u64)>, Failure> {
        let job = self.queue().job(id).cloned().ok_or_else(|| fatal("Tâche introuvable."))?;
        let part = job.partial();
        let mut size = part.metadata().map(|m| m.len()).unwrap_or(0);
        let cross = job.cross_mirror && !job.expected_sha256.is_empty();
        let can_resume = size > 0 && (!job.validator.is_empty() || cross);
        let url = reqwest::Url::parse(&job.url).map_err(|_| Failure::Invalid("Adresse invalide.".into()))?;
        if job.personal {
            resolves_public(&url).map_err(Failure::Invalid)?;
        }
        // Pas de `.timeout()` par requête : dans reqwest il borne TOUT le corps (une ISO de 6 Go
        // échouerait au bout de 20 s). Le délai du client (`client(20)`) s'applique à chaque lecture.
        let mut request = http.get(url).header("User-Agent", download_agent(&job.url)).header("Accept-Encoding", "identity");
        if can_resume {
            request = request.header("Range", format!("bytes={size}-"));
            if !cross {
                request = request.header("If-Range", job.validator.clone());
            }
        }
        let requested_at = Instant::now();
        let mut response = request.send().map_err(net)?;
        let status = response.status().as_u16();
        if status >= 400 {
            return Err(Failure::Http(status));
        }
        let body_started = Instant::now();
        let latency = (body_started - requested_at).as_millis().max(1) as u64;
        let mut limited = self.queue().limit_kib > 0;
        let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
        let kind = header("content-type").split(';').next().unwrap_or("application/octet-stream").trim().to_lowercase();
        let kind = if kind.is_empty() { "application/octet-stream".to_string() } else { kind };
        if (kind.starts_with("text/") && kind != "text/plain") || kind == "application/json" || kind == "application/xml" {
            return Err(Failure::Invalid("La source a renvoyé une page au lieu du fichier.".into()));
        }
        let mut validator = header("etag");
        if validator.is_empty() || validator.starts_with("W/") {
            validator = header("last-modified");
        }
        let continuing = can_resume && status == 206;
        let content_range = header("content-range");
        let content_length = header("content-length").parse::<u64>().unwrap_or(0);
        let disposition = header("content-disposition");
        let final_url = response.url().to_string();
        let mut first_block = Vec::new();
        if kind == "text/plain" {
            if !["OVA", "QCOW2", "ISO"].contains(&job.format.as_str()) {
                return Err(Failure::Invalid("La source a renvoyé du texte au lieu du fichier.".into()));
            }
            if !continuing {
                let want = if job.format == "ISO" { 65536 } else { 512 };
                (&mut response).take(want).read_to_end(&mut first_block).map_err(net)?;
                let ok = (job.format == "QCOW2" && first_block.starts_with(b"QFI\xfb"))
                    || (job.format == "OVA" && first_block.get(257..262) == Some(b"ustar"))
                    || (job.format == "ISO" && iso_marker(&first_block));
                if !ok {
                    return Err(Failure::Invalid("Signature de l’image virtuelle inconnue.".into()));
                }
            }
        }
        let total;
        if continuing {
            let parsed = content_range.strip_prefix("bytes ").and_then(|r| {
                let (range, total) = r.split_once('/')?;
                let (start, _) = range.split_once('-')?;
                Some((start.parse::<u64>().ok()?, total.parse::<u64>().ok()?))
            });
            match parsed {
                Some((start, t)) if start == size && (cross || validator.is_empty() || validator == job.validator) => total = t,
                _ => return Err(Failure::Net("La réponse de reprise du serveur a changé.".into())),
            }
            if cross && job.expected_size > 0 && total != job.expected_size {
                return Err(Failure::Invalid("La taille diffère de celle du catalogue.".into()));
            }
        } else {
            if status != 200 {
                return Err(Failure::Net("Réponse inattendue du serveur de téléchargement.".into()));
            }
            size = 0;
            total = content_length;
        }
        let name = if job.preferred_filename.is_empty() { filename_from(&disposition, &final_url) } else { job.preferred_filename.clone() };
        let initial = size;
        let already = part.metadata().map(|m| m.len()).unwrap_or(0);
        if total > 0 && super::tools::disk_space(Path::new(&job.folder)).is_some_and(|free| free < total.saturating_sub(already)) {
            return Err(fatal("Espace disque insuffisant. Libère de l’espace puis reprends ce téléchargement."));
        }
        let mut stream = OpenOptions::new().create(true).write(true).append(continuing).truncate(!continuing).open(&part).map_err(fatal)?;
        {
            let mut q = self.queue();
            if let Some(j) = q.job(id) {
                j.validator = validator;
                j.file = name.clone();
                j.received = size;
                j.total = total;
                j.cross_mirror = false;
            }
            Self::save_queue(&q, &self.home);
            self.publish(&q);
        }
        if !first_block.is_empty() {
            stream.write_all(&first_block).map_err(fatal)?;
            size += first_block.len() as u64;
        }
        let (mut tick, mut last) = (Instant::now(), size);
        let mut buf = vec![0u8; CHUNK];
        while self.running(id) {
            let n = response.read(&mut buf).map_err(net)?;
            if n == 0 {
                break;
            }
            let limit = self.queue().limit_kib;
            limited |= limit > 0;
            self.throttle(id, n, limit);
            if !self.running(id) {
                break;
            }
            stream.write_all(&buf[..n]).map_err(fatal)?;
            size += n as u64;
            let elapsed = tick.elapsed().as_secs_f64();
            if elapsed >= 0.35 {
                let mut q = self.queue();
                if let Some(j) = q.job(id) {
                    j.received = size;
                    j.speed = ((size - last) as f64 / elapsed).round() as u64;
                }
                self.publish(&q);
                (last, tick) = (size, Instant::now());
            }
        }
        stream.flush().map_err(fatal)?;
        drop(stream);
        let wire_rate = if limited { 0 } else { ((size - initial) as f64 / body_started.elapsed().as_secs_f64().max(0.01)).round() as u64 };
        {
            let mut q = self.queue();
            let j = q.job(id).ok_or_else(|| fatal("Tâche introuvable."))?;
            j.received = size;
            if j.state != "running" {
                return Ok(None);
            }
        }
        if size == 0 || (total > 0 && size != total) {
            return Err(Failure::Net("Transfert incomplet.".into()));
        }
        let mut verification = String::new();
        let mut digest = String::new();
        if !job.expected_sha256.is_empty() {
            let stop = std::sync::atomic::AtomicBool::new(false);
            let hashed = std::thread::scope(|scope| {
                let worker = scope.spawn(|| file_digest(&part, Some(&stop)));
                while !worker.is_finished() {
                    if !self.running(id) {
                        stop.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
                worker.join().unwrap_or_else(|_| Err("Calcul de l’empreinte interrompu.".into()))
            }).map_err(fatal)?;
            let Some(hash) = hashed else { return Ok(None) };
            // Empreinte différente : le fichier est conservé et signalé, jamais supprimé en silence.
            verification = if hash == job.expected_sha256 { "verified" } else { "mismatch" }.into();
            digest = hash;
        }
        let mut final_name = name;
        let packed = (!job.kit.is_empty()).then(|| part.with_extension("zip.part"));
        if let Some(packed) = &packed {
            if let Err(e) = office_kit(packed, &part, &job.kit) {
                let _ = std::fs::remove_file(packed);
                return Err(fatal(format!("Création du kit Office impossible : {e}")));
            }
            final_name = format!("{}-NiTriTe.zip", if job.kit == "365" { "Microsoft365" } else { "Office-LTSC-2024" });
        }
        let mut q = self.queue();
        let still_running = q.job(id).is_some_and(|j| j.state == "running");
        if !still_running {
            if let Some(p) = &packed {
                let _ = std::fs::remove_file(p);
            }
            return Ok(None);
        }
        let dir = part.parent().map(Path::to_path_buf).unwrap_or_default();
        let stem = Path::new(&final_name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = Path::new(&final_name).extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let target = (0..10000).map(|i| if i == 0 { dir.join(&final_name) } else { dir.join(format!("{stem} ({i}){ext}")) })
            .find(|p| !p.exists()).ok_or_else(|| fatal("Trop de fichiers portant ce nom."))?;
        std::fs::rename(packed.as_ref().unwrap_or(&part), &target).map_err(fatal)?;
        if packed.is_some() {
            let _ = std::fs::remove_file(&part);
        }
        if let Some(j) = q.job(id) {
            j.state = "done".into();
            j.file = target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            j.total = if total > 0 { total } else { size };
            j.sha256 = digest;
            j.error = if verification == "mismatch" {
                "Empreinte SHA-256 différente de celle publiée : fichier conservé, ne l’utilise pas sans vérification.".into()
            } else { String::new() };
            j.verification = verification;
        }
        Ok(Some((latency, wire_rate)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn failure_classes() {
        assert!(Failure::Http(503).transient() && Failure::Http(429).transient());
        assert!(!Failure::Http(404).transient() && !Failure::Invalid("x".into()).transient());
    }

    #[test]
    fn eta() {
        let job = |state: &str, total, received, speed| Job { state: state.into(), total, received, speed, ..Job::default() };
        assert_eq!(estimated_seconds(&[job("running", 100, 50, 10), job("queued", 100, 0, 0)]), Some(15));
        assert_eq!(estimated_seconds(&[job("running", 0, 50, 10)]), None);
    }

}
