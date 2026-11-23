//! Port de TokaISO 7.9 (MIT, Cédrick Roy)
//! Santé des liens du catalogue et classement des miroirs (sources.py).

use super::common::*;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SOURCE_WORKERS: usize = 6;
const SOURCE_TIMEOUT: u64 = 9;
const PAGE_SIZE: usize = 40;
const MEASUREMENT_LIFETIME: f64 = 14.0 * 86400.0;
const FAILURE_LIFETIME: f64 = 24.0 * 3600.0;
pub const HEALTH_STATES: [&str; 8] = ["unverified", "file", "portal", "expired", "blocked", "changed", "page", "error"];

pub fn download_agent(url: &str) -> &'static str {
    if url.starts_with("https://downloads.sourceforge.net/project/osboxes/") {
        "curl/8.13.0"
    } else {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) NiTriTe-TokaISO/7.9"
    }
}

pub fn client(timeout: u64) -> Res<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .https_only(true)
        .redirect(redirect_policy())
        .connect_timeout(Duration::from_secs(timeout))
        .timeout(Duration::from_secs(timeout))
        .build()
        .map_err(|e| e.to_string())
}

/// Les signatures binaires reconnues par TokaISO dans un échantillon.
pub fn looks_binary(sample: &[u8]) -> bool {
    const MAGIC: [&[u8]; 6] = [b"QFI\xfb", b"MZ", b"PK\x03\x04", b"7z\xbc\xaf\x27\x1c", b"\x1f\x8b", b"\xfd7zXZ\x00"];
    MAGIC.iter().any(|m| sample.starts_with(m))
        || sample.get(257..262) == Some(b"ustar")
        || iso_marker(sample)
}

pub fn iso_marker(sample: &[u8]) -> bool {
    (32768..sample.len()).step_by(2048).any(|o| sample.get(o + 1..o + 6) == Some(b"CD001"))
}

fn verify_link(http: &reqwest::blocking::Client, source: &Value) -> Value {
    let portal = str_of(source, "kind") == "portal";
    let url = str_of(source, "url");
    let started = Instant::now();
    let mut result = json!({"ok": false, "status": "error", "size": 0, "rate": 0, "checked": now_iso()});
    let mut request = http.get(url).header("User-Agent", download_agent(url)).header("Accept-Encoding", "identity");
    if !portal {
        request = request.header("Range", "bytes=0-65535");
    }
    let outcome = request.send().and_then(|r| r.error_for_status());
    match outcome {
        Ok(response) => {
            let code = response.status().as_u16();
            result["code"] = json!(code);
            result["latency_ms"] = json!(started.elapsed().as_millis().max(1) as u64);
            if portal {
                result["ok"] = json!(true);
                result["status"] = json!("portal");
                result["message"] = json!(format!("Portail accessible (HTTP {code})"));
            } else {
                let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
                let media = header("content-type").split(';').next().unwrap_or("").trim().to_lowercase();
                let length = if code == 206 { header("content-range").rsplit('/').next().unwrap_or("").to_string() } else { header("content-length") };
                let mut sample = Vec::new();
                let read = response.take(65536).read_to_end(&mut sample);
                if let Err(e) = read {
                    result["message"] = json!(format!("Contrôle impossible · {}", e.to_string().chars().take(350).collect::<String>()));
                } else {
                    let binary = looks_binary(&sample);
                    let head = String::from_utf8_lossy(&sample[..sample.len().min(64)]).trim_start().to_lowercase();
                    if sample.is_empty() || ["text/html", "application/json", "application/xml"].contains(&media.as_str())
                        || (media.starts_with("text/") && !binary)
                        || head.starts_with("<!doctype") || head.starts_with("<html") || head.starts_with("<?xml") {
                        result["status"] = json!("page");
                        result["message"] = json!("Le serveur a renvoyé une page ou une réponse vide.");
                    } else {
                        let size = length.parse::<u64>().unwrap_or(0);
                        let expected = u64_of(source, "size");
                        let changed = size > 0 && expected > 0 && size != expected;
                        let rate = sample.len() as f64 / started.elapsed().as_secs_f64().max(0.001);
                        result["ok"] = json!(!changed);
                        result["status"] = json!(if changed { "changed" } else { "file" });
                        result["size"] = json!(size);
                        result["ranges"] = json!(code == 206);
                        result["rate"] = json!(rate.round() as u64);
                        result["message"] = json!(if changed { "La taille diffère de celle du catalogue.".to_string() } else { format!("Fichier accessible par échantillon (HTTP {code})") });
                    }
                }
            }
        }
        Err(error) => match error.status().map(|s| s.as_u16()) {
            Some(code) => {
                let status = match code { 404 | 410 => "expired", 401 | 403 | 429 => "blocked", _ => "error" };
                result["status"] = json!(status);
                result["code"] = json!(code);
                result["message"] = json!(format!("Source inaccessible (HTTP {code})"));
            }
            None => {
                let detail: String = error.to_string().chars().take(350).collect();
                result["message"] = json!(format!("Contrôle impossible · {detail}"));
            }
        },
    }
    result["elapsed"] = json!((started.elapsed().as_secs_f64() * 100.0).round() / 100.0);
    result
}

/// Deux sources fournissent le même fichier : même SHA-256, même taille, ni kit ni portail.
pub fn same_file(first: &Value, second: &Value) -> bool {
    let (a, b) = (str_of(first, "sha256"), str_of(second, "sha256"));
    let excluded = |v: &Value| matches!(str_of(v, "kind"), "kit" | "portal");
    is_hex64(a) && a.eq_ignore_ascii_case(b) && u64_of(first, "size") == u64_of(second, "size") && !excluded(first) && !excluded(second)
}

fn read_mapping(path: &Path) -> Map<String, Value> {
    json_read(path).and_then(|v| v.as_object().cloned()).unwrap_or_default()
        .into_iter().filter(|(_, row)| row.is_object()).collect()
}

fn write_mapping(path: &Path, data: &Map<String, Value>) {
    // Un échec d'écriture ne doit jamais interrompre un téléchargement ou un contrôle.
    if let Err(e) = json_write(path, &Value::Object(data.clone())) {
        tracing::warn!("iso_catalog: {e}");
    }
}

fn checked_time(row: &Value) -> f64 {
    chrono::DateTime::parse_from_rfc3339(str_of(row, "checked")).map(|d| d.timestamp() as f64).unwrap_or(0.0)
}

fn statistic(row: &Value, key: &str) -> f64 {
    row.get(key).and_then(Value::as_f64).filter(|v| *v > 0.0 && *v < 1e15).unwrap_or(0.0)
}

fn section_matches(item: &Value, family: &str) -> bool {
    str_of(item, "family") == family || str_of(item, "section") == family
}

#[derive(Default)]
struct Health {
    results: Map<String, Value>,
    performance: Map<String, Value>,
    progress: Value,
}

pub struct SourceHealth {
    path: PathBuf,
    performance_path: PathBuf,
    inner: Arc<Mutex<Health>>,
    cancel: Arc<AtomicBool>,
}

impl SourceHealth {
    pub fn new(home: &Path, items: &[Value]) -> Self {
        let urls: std::collections::HashSet<&str> = items.iter().flat_map(sources_of).map(|s| str_of(s, "url")).collect();
        let path = home.join("sources.json");
        let performance_path = home.join("mirror-performance.json");
        let results = read_mapping(&path).into_iter()
            .filter(|(url, row)| urls.contains(url.as_str()) && checked_time(row) > 0.0 && HEALTH_STATES.contains(&str_of(row, "status")))
            .collect();
        let cutoff = now_secs() - MEASUREMENT_LIFETIME;
        let performance = read_mapping(&performance_path).into_iter()
            .filter(|(_, row)| statistic(row, "last_success").max(statistic(row, "last_failure")) >= cutoff)
            .collect();
        let progress = json!({"running": false, "done": 0, "total": 0, "family": "", "cancelled": false});
        Self { path, performance_path, inner: Arc::new(Mutex::new(Health { results, performance, progress })), cancel: Arc::new(AtomicBool::new(false)) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Health> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn record_transfer(&self, url: &str, latency_ms: u64, rate: u64, failed: bool) {
        let now = now_secs();
        let mut h = self.lock();
        let row = h.performance.entry(url.to_string()).or_insert_with(|| json!({}));
        row["last_outcome"] = json!(if failed { "failed" } else { "success" });
        if failed {
            row["last_failure"] = json!(now);
        } else {
            let previous_measurement = statistic(row, "last_success");
            row["last_success"] = json!(now);
            for (key, measurement) in [("rate", rate as f64), ("latency_ms", latency_ms as f64)] {
                if measurement > 0.0 && measurement < 1e15 {
                    let previous = if now - previous_measurement < MEASUREMENT_LIFETIME { statistic(row, key) } else { 0.0 };
                    row[key] = json!((if previous > 0.0 { (measurement + previous) / 2.0 } else { measurement }).round() as u64);
                }
            }
        }
        write_mapping(&self.performance_path, &h.performance);
    }

    /// Classement des miroirs : échecs récents en dernier, puis débit mesuré, puis latence.
    pub fn rank_mirrors(&self, mirrors: &mut [Value]) {
        let now = now_secs();
        let h = self.lock();
        let empty = json!({});
        let score = |candidate: &Value| -> (u8, f64, f64) {
            let url = str_of(candidate, "url");
            let row = h.performance.get(url).unwrap_or(&empty);
            let check = h.results.get(url).unwrap_or(&empty);
            let check_time = checked_time(check);
            let measured = statistic(row, "last_success");
            let checked = if str_of(check, "status") == "file" { check_time } else { 0.0 };
            let good = measured.max(checked);
            let check_failed = check.as_object().is_some_and(|m| !m.is_empty()) && check.get("ok") != Some(&json!(true));
            let failed = statistic(row, "last_failure").max(if check_failed { check_time } else { 0.0 });
            let last_failed = failed > good || (failed == good && str_of(row, "last_outcome") == "failed");
            if last_failed && now - failed < FAILURE_LIFETIME {
                return (2, 0.0, f64::INFINITY);
            }
            if now - good >= MEASUREMENT_LIFETIME {
                return (1, 0.0, f64::INFINITY);
            }
            let current = if now - measured < MEASUREMENT_LIFETIME { row } else { &empty };
            let sample = if now - checked < MEASUREMENT_LIFETIME { check } else { &empty };
            let rate = Some(statistic(current, "rate")).filter(|v| *v > 0.0).unwrap_or(statistic(sample, "rate"));
            let latency = Some(statistic(current, "latency_ms")).filter(|v| *v > 0.0).unwrap_or(statistic(sample, "latency_ms"));
            (0, -rate, if latency > 0.0 { latency } else { f64::INFINITY })
        };
        mirrors.sort_by(|a, b| score(a).partial_cmp(&score(b)).unwrap_or(std::cmp::Ordering::Equal));
    }

    fn probe(&self, http: &reqwest::blocking::Client, source: &Value) -> Value {
        let result = verify_link(http, source);
        self.lock().results.insert(str_of(source, "url").to_string(), result.clone());
        result
    }

    fn save(&self) {
        let h = self.lock();
        write_mapping(&self.path, &h.results);
    }

    fn check_of(h: &Health, source: &Value) -> Option<Value> {
        let check = h.results.get(str_of(source, "url"))?;
        let status = str_of(check, "status");
        if !["file", "changed", "portal"].contains(&status) {
            return Some(check.clone());
        }
        if (status == "portal") != (str_of(source, "kind") == "portal") {
            return None;
        }
        if status == "portal" {
            return Some(check.clone());
        }
        let size = u64_of(check, "size");
        let changed = size > 0 && u64_of(source, "size") > 0 && u64_of(source, "size") != size;
        let mut out = check.clone();
        out["ok"] = json!(!changed);
        out["status"] = json!(if changed { "changed" } else { "file" });
        out["message"] = json!(if changed { "La taille diffère de celle du catalogue.".to_string() } else {
            format!("Fichier accessible par échantillon (HTTP {})", check.get("code").cloned().unwrap_or(json!("?"))) });
        Some(out)
    }

    pub fn test(&self, source: &Value) -> Res<Value> {
        let result = self.probe(&client(SOURCE_TIMEOUT)?, source);
        self.save();
        Ok(result)
    }

    /// Teste toutes les sources de même empreinte puis les classe (« source-mirrors »).
    pub fn mirrors(&self, item: &Value, index: usize) -> Res<Value> {
        let all = sources_of(item);
        let asset = all.get(index).ok_or("Source introuvable.")?;
        let choices: Vec<(usize, &Value)> = all.iter().enumerate().filter(|(p, s)| *p == index || same_file(asset, s)).collect();
        let http = client(SOURCE_TIMEOUT)?;
        let rows: Vec<Value> = std::thread::scope(|scope| {
            let handles: Vec<_> = choices.iter().map(|(p, s)| {
                let http = &http;
                scope.spawn(move || {
                    let mut row = self.probe(http, s);
                    row["index"] = json!(p);
                    row["label"] = json!(str_of(s, "label"));
                    row["url"] = json!(str_of(s, "url"));
                    row
                })
            }).collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        self.save();
        let mut ranked = rows;
        self.rank_mirrors(&mut ranked);
        let h = self.lock();
        let out: Vec<Value> = ranked.into_iter().map(|mut row| {
            row["performance"] = h.performance.get(str_of(&row, "url")).cloned().unwrap_or(json!({}));
            if let Some(m) = row.as_object_mut() {
                m.remove("url");
            }
            row
        }).collect();
        Ok(json!({"mirrors": out}))
    }

    pub fn item_checks(&self, item: &Value) -> Value {
        let h = self.lock();
        json!({"checks": sources_of(item).iter().map(|s| Self::check_of(&h, s)).collect::<Vec<_>>()})
    }

    pub fn dashboard(&self, items: &[Value], payload: &Value) -> Value {
        let mut categories: Vec<String> = items.iter()
            .flat_map(|i| [str_of(i, "family"), str_of(i, "section")])
            .filter(|s| !s.is_empty()).map(str::to_string)
            .collect::<std::collections::HashSet<_>>().into_iter().collect();
        categories.sort_by_key(|c| c.to_lowercase());
        let family = payload.get("family").and_then(Value::as_str).filter(|f| categories.iter().any(|c| c == f)).unwrap_or("");
        let status = payload.get("status").and_then(Value::as_str).filter(|s| HEALTH_STATES.contains(s)).unwrap_or("");
        let page = payload.get("page").and_then(Value::as_u64).unwrap_or(0) as usize;
        let h = self.lock();
        let (mut count, mut totals, mut rows) = (0usize, HashMap::<String, usize>::new(), Vec::new());
        for item in items.iter().filter(|i| family.is_empty() || section_matches(i, family)) {
            for (index, source) in sources_of(item).iter().enumerate() {
                count += 1;
                let check = Self::check_of(&h, source);
                let state = check.as_ref().map(|c| str_of(c, "status").to_string()).unwrap_or_else(|| "unverified".into());
                *totals.entry(state.clone()).or_default() += 1;
                if !status.is_empty() && state != status {
                    continue;
                }
                rows.push(json!({"item": item["id"], "name": item["name"], "source": index, "label": str_of(source, "label"),
                    "filename": str_of(source, "filename"), "family": str_of(item, "family"), "check": check,
                    "performance": h.performance.get(str_of(source, "url")).cloned().unwrap_or(json!({}))}));
            }
        }
        rows.sort_by(|a, b| (str_of(a, "name").to_lowercase(), u64_of(a, "source")).cmp(&(str_of(b, "name").to_lowercase(), u64_of(b, "source"))));
        let pages = rows.len().div_ceil(PAGE_SIZE).max(1);
        let page = page.min(pages - 1);
        let filtered = rows.len();
        let slice: Vec<Value> = rows.into_iter().skip(page * PAGE_SIZE).take(PAGE_SIZE).collect();
        json!({"progress": h.progress, "categories": categories, "family": family, "page": page, "pages": pages,
               "count": count, "filtered": filtered, "totals": totals, "rows": slice})
    }

    pub fn overview(&self, items: &[Value]) -> Value {
        let h = self.lock();
        let mut review: Vec<Value> = Vec::new();
        for item in items {
            for (index, source) in sources_of(item).iter().enumerate() {
                if let Some(check) = Self::check_of(&h, source).filter(|c| c.get("ok") != Some(&json!(true))) {
                    review.push(json!({"item": item["id"], "name": item["name"], "source": index, "label": str_of(source, "label"), "check": check}));
                }
            }
        }
        review.sort_by(|a, b| str_of(&b["check"], "checked").cmp(str_of(&a["check"], "checked")));
        let count = review.len();
        review.truncate(200);
        json!({"progress": h.progress, "checked": h.results.len(), "review": review, "reviewCount": count})
    }

    pub fn stop_scan(&self) {
        let mut h = self.lock();
        if h.progress["running"] == json!(true) {
            self.cancel.store(true, Ordering::Relaxed);
            h.progress["cancelled"] = json!(true);
        }
    }

    /// Lance un contrôle en arrière-plan (6 contrôles simultanés, par vagues).
    pub fn start_scan(self: &Arc<Self>, items: &[Value], family: &str) -> Res<()> {
        let mut h = self.lock();
        if h.progress["running"] == json!(true) {
            return Ok(());
        }
        let catalogue: Vec<&Value> = items.iter().filter(|i| family.is_empty() || section_matches(i, family)).collect();
        if !family.is_empty() && catalogue.is_empty() {
            return Err("Cette catégorie du catalogue est introuvable.".into());
        }
        let mut seen = std::collections::HashSet::new();
        let sources: Vec<Value> = catalogue.iter().flat_map(|i| sources_of(i)).filter(|s| seen.insert(str_of(s, "url").to_string())).cloned().collect();
        self.cancel.store(false, Ordering::Relaxed);
        h.progress = json!({"running": !sources.is_empty(), "done": 0, "total": sources.len(), "family": family, "cancelled": false});
        drop(h);
        if sources.is_empty() {
            return Ok(());
        }
        let me = Arc::clone(self);
        std::thread::spawn(move || me.scan(sources));
        Ok(())
    }

    fn scan(&self, sources: Vec<Value>) {
        if let Ok(http) = client(SOURCE_TIMEOUT) {
            for wave in sources.chunks(SOURCE_WORKERS) {
                if self.cancel.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::scope(|scope| {
                    for source in wave {
                        let http = &http;
                        scope.spawn(move || {
                            self.probe(http, source);
                            let mut h = self.lock();
                            let done = u64_of(&h.progress, "done") + 1;
                            h.progress["done"] = json!(done);
                        });
                    }
                });
                self.save();
            }
        }
        let mut h = self.lock();
        let unfinished = u64_of(&h.progress, "done") < u64_of(&h.progress, "total");
        h.progress["running"] = json!(false);
        h.progress["cancelled"] = json!(self.cancel.load(Ordering::Relaxed) && unfinished);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_file_rules() {
        let h = "a".repeat(64);
        let a = json!({"sha256": h, "size": 5, "kind": "official"});
        assert!(same_file(&a, &json!({"sha256": h.to_uppercase(), "size": 5, "kind": "community"})));
        assert!(!same_file(&a, &json!({"sha256": h, "size": 6, "kind": "official"})));
        assert!(!same_file(&a, &json!({"sha256": h, "size": 5, "kind": "kit"})));
        assert!(!same_file(&json!({"sha256": "", "size": 5}), &json!({"sha256": "", "size": 5})));
    }

    #[test]
    fn binary_detection() {
        assert!(looks_binary(b"MZ\x90\x00"));
        let mut iso = vec![0u8; 34816];
        iso[32769..32774].copy_from_slice(b"CD001");
        assert!(looks_binary(&iso));
        assert!(!looks_binary(b"<html>"));
    }

    #[test]
    fn mirror_ranking() {
        let dir = std::env::temp_dir().join(format!("nitrite-iso-rank-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let health = SourceHealth::new(&dir, &[]);
        health.record_transfer("https://lent.example/a", 50, 1_000, false);
        health.record_transfer("https://rapide.example/a", 80, 9_000, false);
        health.record_transfer("https://panne.example/a", 0, 0, true);
        let mut mirrors = vec![json!({"url": "https://panne.example/a"}), json!({"url": "https://inconnu.example/a"}),
                               json!({"url": "https://lent.example/a"}), json!({"url": "https://rapide.example/a"})];
        health.rank_mirrors(&mut mirrors);
        let order: Vec<&str> = mirrors.iter().map(|m| str_of(m, "url")).collect();
        assert_eq!(order, ["https://rapide.example/a", "https://lent.example/a", "https://inconnu.example/a", "https://panne.example/a"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
