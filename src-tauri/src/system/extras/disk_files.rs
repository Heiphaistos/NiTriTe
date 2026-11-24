use serde::Serialize;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "windows")]
use super::{parse_json_arr, ps, ps_ok};

// ─── Parcours commun (visualiseur, gros fichiers, doublons) ──────────────────
//
// Parcours natif (walkdir) au lieu de `Get-ChildItem -Recurse` via ps() :
// 1. ps() coupe à 180 s — mesuré le 2026-11-23 : Get-ChildItem -Recurse sur
//    C:\ (1,5 million de fichiers) prend 3 min 57 s, donc le scan par défaut
//    (C:\) et tout disque externe de 1-2 To échouaient toujours en timeout ;
// 2. Windows PowerShell 5.1 suit les jonctions/liens symboliques pendant
//    -Recurse (pas d'option -FollowSymlink avant PS 6) : boucles et doublons
//    fantômes ;
// 3. aucun moyen d'annuler. `follow_links(false)` : walkdir ne descend pas
//    dans un point de réanalyse « name surrogate » (lien symbolique OU
//    jonction — `FileType::is_symlink()` de std les couvre tous les deux).

static SCAN_CANCEL: AtomicBool = AtomicBool::new(false);
const CANCELLED: &str = "Analyse annulée.";

/// Annule l'analyse disque en cours (visualiseur, gros fichiers, doublons).
#[tauri::command]
pub fn cancel_disk_scan() {
    SCAN_CANCEL.store(true, Ordering::SeqCst);
}

fn cancelled() -> bool {
    SCAN_CANCEL.load(Ordering::Relaxed)
}

fn walk(root: &Path) -> impl Iterator<Item = walkdir::DirEntry> {
    walkdir::WalkDir::new(root).follow_links(false).into_iter().filter_map(|e| e.ok())
}

fn file_len(e: &walkdir::DirEntry) -> u64 {
    e.metadata().map(|m| m.len()).unwrap_or(0)
}

fn check_root(path: &str) -> Result<&Path, String> {
    let p = Path::new(path.trim());
    if path.trim().is_empty() || !p.is_dir() {
        return Err(format!("Dossier introuvable : {}", path));
    }
    Ok(p)
}

/// Date locale « AAAA-MM-JJ HH:MM » (même rendu que l'ancien script PS).
fn local_stamp(t: std::time::SystemTime) -> String {
    use chrono::{Datelike, Timelike};
    let d: chrono::DateTime<chrono::Local> = t.into();
    format!("{:04}-{:02}-{:02} {:02}:{:02}", d.year(), d.month(), d.day(), d.hour(), d.minute())
}

// ─── Disk Space Visualizer ────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DiskNode {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub is_dir: bool,
    pub children: Vec<DiskNode>,
}

#[tauri::command]
pub async fn get_disk_tree(path: String, max_depth: u32) -> Result<DiskNode, String> {
    tokio::task::spawn_blocking(move || get_disk_tree_sync(path, max_depth))
        .await
        .map_err(|e| e.to_string())?
}

fn get_disk_tree_sync(path: String, max_depth: u32) -> Result<DiskNode, String> {
    SCAN_CANCEL.store(false, Ordering::SeqCst);
    let p = check_root(&path)?;
    disk_build_node(p, true, 0, max_depth.min(4))
}

fn disk_dir_size(p: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    for e in walk(p) {
        if cancelled() { return Err(CANCELLED.into()); }
        if e.file_type().is_file() { total += file_len(&e); }
    }
    Ok(total)
}

fn disk_build_node(p: &Path, is_dir: bool, depth: u32, max: u32) -> Result<DiskNode, String> {
    if cancelled() { return Err(CANCELLED.into()); }
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.to_string_lossy().to_string());
    let path = p.to_string_lossy().to_string();
    if !is_dir {
        let size = std::fs::symlink_metadata(p).map(|m| m.len()).unwrap_or(0);
        return Ok(DiskNode { name, path, size_bytes: size, is_dir: false, children: vec![] });
    }
    if depth >= max {
        let size = disk_dir_size(p)?;
        return Ok(DiskNode { name, path, size_bytes: size, is_dir: true, children: vec![] });
    }
    let mut children = Vec::new();
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.filter_map(|e| e.ok()) {
            // file_type() ne suit pas les points de réanalyse : un lien ou une
            // jonction n'est ni compté ni parcouru (pas de boucle, pas de
            // double comptage d'un dossier pointé ailleurs).
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() { continue; }
            children.push(disk_build_node(&e.path(), ft.is_dir(), depth + 1, max)?);
        }
    }
    // Somme sur TOUS les enfants AVANT la troncature d'affichage : sinon la
    // taille du dossier n'inclurait que ses 20 plus gros enfants (sous-estimation
    // qui se propage à chaque niveau de l'arbre).
    let size: u64 = children.iter().map(|c| c.size_bytes).sum();
    children.sort_by_key(|a| Reverse(a.size_bytes));
    children.truncate(20);
    Ok(DiskNode { name, path, size_bytes: size, is_dir: true, children })
}

// ─── Big Files Finder ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct BigFile {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub extension: String,
    pub modified: String,
}

// Anti-freeze : parcours disque est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn get_big_files(path: String, count: u32, excluded_folders: Option<Vec<String>>) -> Result<Vec<BigFile>, String> {
    tokio::task::spawn_blocking(move || get_big_files_blocking(path, count, excluded_folders))
        .await
        .map_err(|e| e.to_string())?
}

/// Normalise un dossier d'exclusion : minuscules, `\`, sans `\` final.
fn norm_excl(p: &str) -> String {
    p.trim().to_lowercase().replace('/', "\\").trim_end_matches('\\').to_string()
}

/// `path` est-il le dossier exclu ou un de ses descendants ? Comparaison à la
/// frontière d'un composant : « C:\Temp » n'exclut pas « C:\Temporaire ».
fn is_excluded(path: &Path, exclusions: &[String]) -> bool {
    let p = path.to_string_lossy().to_lowercase();
    exclusions.iter().any(|ex| {
        !ex.is_empty() && p.starts_with(ex.as_str()) && matches!(p.as_bytes().get(ex.len()), None | Some(b'\\'))
    })
}

fn get_big_files_blocking(path: String, count: u32, excluded_folders: Option<Vec<String>>) -> Result<Vec<BigFile>, String> {
    SCAN_CANCEL.store(false, Ordering::SeqCst);
    let root = check_root(&path)?;
    let n = count.clamp(1, 200) as usize;
    let exclusions: Vec<String> = excluded_folders.unwrap_or_default().iter().map(|p| norm_excl(p)).collect();
    // Tas min borné à n : mémoire constante même sur des millions de fichiers.
    let mut heap: BinaryHeap<Reverse<(u64, PathBuf)>> = BinaryHeap::with_capacity(n + 1);
    let walker = walkdir::WalkDir::new(root).follow_links(false).into_iter()
        .filter_entry(|e| !is_excluded(e.path(), &exclusions));
    for e in walker.filter_map(|e| e.ok()) {
        if cancelled() { return Err(CANCELLED.into()); }
        if !e.file_type().is_file() { continue; }
        let len = file_len(&e);
        if heap.len() < n {
            heap.push(Reverse((len, e.into_path())));
        } else if heap.peek().is_some_and(|Reverse((min, _))| len > *min) {
            heap.pop();
            heap.push(Reverse((len, e.into_path())));
        }
    }
    let mut top: Vec<(u64, PathBuf)> = heap.into_iter().map(|Reverse(x)| x).collect();
    top.sort_by_key(|(len, _)| Reverse(*len));
    Ok(top.into_iter().map(|(len, p)| {
        let modified = std::fs::symlink_metadata(&p).and_then(|m| m.modified()).map(local_stamp).unwrap_or_default();
        BigFile {
            name: p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            extension: p.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default(),
            path: p.to_string_lossy().to_string(),
            size_bytes: len,
            modified,
        }
    }).collect())
}

// ─── Duplicate Finder ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub size_bytes: u64,
    pub count: u32,
    pub files: Vec<String>,
    pub wasted_bytes: u64,
}

#[tauri::command]
pub async fn find_duplicates(path: String, min_size_kb: u64, mode: Option<String>) -> Result<Vec<DuplicateGroup>, String> {
    let m = mode.unwrap_or_else(|| "hash".to_string());
    tokio::task::spawn_blocking(move || find_duplicates_sync(path, min_size_kb, m))
        .await
        .map_err(|e| e.to_string())?
}

/// SHA-256 (hex majuscule) du fichier, ou de ses `limit` premiers octets.
/// `None` si illisible (fichier verrouillé, droits) : le fichier est ignoré.
fn hash_prefix(p: &Path, limit: Option<u64>) -> Result<Option<String>, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let Ok(f) = std::fs::File::open(p) else { return Ok(None) };
    let mut r: Box<dyn Read> = match limit { Some(l) => Box::new(f.take(l)), None => Box::new(f) };
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if cancelled() { return Err(CANCELLED.into()); }
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(k) => h.update(&buf[..k]),
            Err(_) => return Ok(None),
        }
    }
    Ok(Some(h.finalize().iter().map(|b| format!("{b:02X}")).collect()))
}

/// Regroupe `paths` par empreinte ; ne garde que les groupes d'au moins 2.
fn group_by_hash(paths: Vec<PathBuf>, limit: Option<u64>) -> Result<Vec<(String, Vec<PathBuf>)>, String> {
    let mut by: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for p in paths {
        if let Some(h) = hash_prefix(&p, limit)? { by.entry(h).or_default().push(p); }
    }
    Ok(by.into_iter().filter(|(_, v)| v.len() > 1).collect())
}

fn find_duplicates_sync(path: String, min_size_kb: u64, mode: String) -> Result<Vec<DuplicateGroup>, String> {
    SCAN_CANCEL.store(false, Ordering::SeqCst);
    let root = check_root(&path)?;
    let min_bytes = min_size_kb.saturating_mul(1024).max(1024);
    let mut files: Vec<(PathBuf, u64)> = Vec::new();
    for e in walk(root) {
        if cancelled() { return Err(CANCELLED.into()); }
        if !e.file_type().is_file() { continue; }
        let len = file_len(&e);
        if len >= min_bytes { files.push((e.into_path(), len)); }
    }
    let as_str = |p: &PathBuf| p.to_string_lossy().to_string();
    let mut groups: Vec<DuplicateGroup> = Vec::new();
    if mode == "name" {
        // Mode nom : les homonymes peuvent avoir des tailles DIFFÉRENTES. Le plus
        // gros (index 0) est « l'original » gardé ; gaspillage = total - plus gros.
        let mut by: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();
        for (p, len) in files {
            let key = p.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            by.entry(key).or_default().push((p, len));
        }
        for (_, mut v) in by.into_iter().filter(|(_, v)| v.len() > 1) {
            v.sort_by_key(|(_, len)| Reverse(*len));
            let keep = v[0].1;
            let total: u64 = v.iter().map(|(_, l)| *l).sum();
            groups.push(DuplicateGroup {
                hash: v[0].0.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                size_bytes: keep,
                count: v.len() as u32,
                wasted_bytes: total - keep,
                files: v.iter().map(|(p, _)| as_str(p)).collect(),
            });
        }
    } else {
        // Mode contenu : même taille, puis même empreinte des 64 premiers Ko,
        // puis même empreinte complète. Seuls les vrais candidats sont lus en entier.
        let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
        for (p, len) in files { by_size.entry(len).or_default().push(p); }
        for (size, paths) in by_size.into_iter().filter(|(_, v)| v.len() > 1) {
            let candidates = if size > 65_536 {
                group_by_hash(paths, Some(65_536))?.into_iter().flat_map(|(_, v)| v).collect()
            } else { paths };
            for (hash, mut v) in group_by_hash(candidates, None)? {
                v.sort();
                groups.push(DuplicateGroup {
                    hash,
                    size_bytes: size,
                    count: v.len() as u32,
                    wasted_bytes: size * (v.len() as u64 - 1),
                    files: v.iter().map(as_str).collect(),
                });
            }
        }
    }
    groups.sort_by_key(|a| Reverse(a.wasted_bytes));
    Ok(groups)
}

// ─── Delete File ──────────────────────────────────────────────────────────────

/// Vérifie si un chemin canonique pointe vers un répertoire système protégé.
/// Gère le préfixe verbatim `\\?\` que `canonicalize()` retourne sur Windows
/// via GetFinalPathNameByHandleW — sans strip, la comparaison échoue toujours.
fn is_system_path_blocked(canonical: &std::path::Path) -> bool {
    let raw = canonical.to_string_lossy().to_lowercase().replace('/', "\\");
    // Strip le préfixe verbatim étendu (\\?\C:\... → C:\...)
    let path_lower = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    let blocked = [
        "c:\\windows\\",
        "c:\\program files\\",
        "c:\\program files (x86)\\",
        "c:\\programdata\\",
        "c:\\system volume information\\",
    ];
    blocked.iter().any(|prefix| path_lower.starts_with(prefix))
}

// Anti-freeze : I/O fichier (canonicalize+remove) est bloquant — jamais
// inline sur le thread de commande.
#[tauri::command]
pub async fn delete_file(path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || delete_file_blocking(path))
        .await
        .map_err(|e| e.to_string())?
}

fn delete_file_blocking(path: String) -> Result<(), String> {
    // Canonicalize first to resolve ".." traversal before checking blocked prefixes.
    let canonical = std::fs::canonicalize(&path)
        .map_err(|e| format!("Chemin inaccessible {}: {}", path, e))?;
    if is_system_path_blocked(&canonical) {
        return Err(format!("Suppression interdite dans les répertoires système: {}", canonical.display()));
    }
    std::fs::remove_file(&canonical).map_err(|e| format!("{}: {}", canonical.display(), e))
}

// Anti-freeze : PowerShell est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn trash_file(path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || trash_file_blocking(path))
        .await
        .map_err(|e| e.to_string())?
}

fn trash_file_blocking(path: String) -> Result<(), String> {
    // Meme garde que delete_file_blocking : trash_file est appele EN PREMIER
    // par le frontend (DuplicateFinderPage/BigFilesFinderPage font trash_file
    // -> fallback delete_file seulement en cas d'echec), donc sans ce check ici
    // la protection de delete_file_blocking n'est jamais atteinte en pratique —
    // un scan pointe sur C:\Windows (scanPath est un champ texte libre cote UI)
    // pouvait envoyer des fichiers systeme a la corbeille sans aucun garde-fou.
    let canonical = std::fs::canonicalize(&path)
        .map_err(|e| format!("Chemin inaccessible {}: {}", path, e))?;
    if is_system_path_blocked(&canonical) {
        return Err(format!("Suppression interdite dans les répertoires système: {}", canonical.display()));
    }
    // Chemin original (pas `canonical`) pour la commande : `canonical` porte le
    // préfixe verbatim étendu `\\?\` sur Windows, mal supporté par
    // Microsoft.VisualBasic.FileIO (contrairement à std::fs, utilisé par
    // delete_file_blocking) — le garde de sécurité ci-dessus reste valide car
    // canonicalize() résout déjà tout `..` avant la comparaison.
    //
    // ps_ok + try/catch/exit 1 : ps() renvoyait Ok même quand DeleteFile levait
    // (fichier verrouillé, accès refusé) — l'UI affichait « Envoyé à la
    // corbeille » et retirait le fichier de la liste alors qu'il était
    // toujours là (faux succès). Vérification finale : le fichier a disparu.
    let safe = path.replace('\'', "''");
    let script = format!(r#"
try {{
  Add-Type -AssemblyName Microsoft.VisualBasic
  [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile(
      '{safe}',
      [Microsoft.VisualBasic.FileIO.UIOption]::OnlyErrorDialogs,
      [Microsoft.VisualBasic.FileIO.RecycleOption]::SendToRecycleBin
  )
}} catch {{ Write-Output $_.Exception.Message; exit 1 }}"#);
    ps_ok(&script)?;
    if std::path::Path::new(&path).exists() {
        return Err(format!("Le fichier est toujours présent : {}", path));
    }
    Ok(())
}

// ─── Browser Cache ────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct BrowserCacheInfo {
    pub browser: String,
    pub path: String,
    pub size_mb: f64,
    pub exists: bool,
}

// Anti-freeze : PowerShell est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn get_browser_cache_info() -> Result<Vec<BrowserCacheInfo>, String> {
    tokio::task::spawn_blocking(get_browser_cache_info_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn get_browser_cache_info_blocking() -> Result<Vec<BrowserCacheInfo>, String> {
    let script = r#"
$localApp = [Environment]::GetFolderPath('LocalApplicationData')
$roaming   = [Environment]::GetFolderPath('ApplicationData')
# Firefox : le dossier "$roaming\Mozilla\Firefox\Profiles" est le PROFIL
# complet (favoris places.sqlite, mots de passe logins.json/key4.db,
# cookies, extensions...), PAS le cache — seul le sous-dossier "cache2" de
# chaque profil (stocké sous LocalAppData, pas Roaming) est du cache
# régénérable. Vider "Profiles" via clean_browser_cache_path effacerait
# tout le profil Firefox. Repli sur le 1er profil trouvé (mêmes hypothèses
# que maintenance/browser_cleanup.rs::get_browser_cache_sizes, déjà correct).
$ffCache = ''
$ffProfilesLocal = "$localApp\Mozilla\Firefox\Profiles"
if (Test-Path $ffProfilesLocal) {
    $ffProf = Get-ChildItem $ffProfilesLocal -Directory -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($ffProf) { $ffCache = Join-Path $ffProf.FullName 'cache2' }
}
$caches = @(
    @{ browser='Google Chrome';    path="$localApp\Google\Chrome\User Data\Default\Cache" },
    @{ browser='Microsoft Edge';   path="$localApp\Microsoft\Edge\User Data\Default\Cache" },
    @{ browser='Firefox';          path=$ffCache },
    @{ browser='Opera';            path="$roaming\Opera Software\Opera Stable\Cache" },
    @{ browser='Brave';            path="$localApp\BraveSoftware\Brave-Browser\User Data\Default\Cache" },
    @{ browser='Vivaldi';          path="$localApp\Vivaldi\User Data\Default\Cache" }
)
$result = $caches | ForEach-Object {
    $p = $_.path
    # Test-Path lève une erreur non-terminante sur une chaine vide ($ffCache
    # quand Firefox/aucun profil n'est trouve) SANS reinitialiser $exists,
    # qui garde alors la valeur du navigateur precedent dans la boucle.
    $exists = if ([string]::IsNullOrEmpty($p)) { $false } else { Test-Path $p }
    $size = 0.0
    if ($exists) {
        $size = [math]::Round((Get-ChildItem $p -Recurse -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum / 1MB, 2)
    }
    [PSCustomObject]@{ browser=$_.browser; path=$p; size_mb=$size; exists=[bool]$exists }
}
$result | ConvertTo-Json -Compress"#;
    let out = ps(script)?;
    if out.is_empty() { return Ok(vec![]); }
    let arr: Vec<serde_json::Value> = parse_json_arr(&out);
    Ok(arr.iter().map(|v| BrowserCacheInfo {
        browser:  v["browser"].as_str().unwrap_or("").to_string(),
        path:     v["path"].as_str().unwrap_or("").to_string(),
        size_mb:  v["size_mb"].as_f64().unwrap_or(0.0),
        exists:   v["exists"].as_bool().unwrap_or(false),
    }).collect())
}

// Anti-freeze : PowerShell (suppression cache) est bloquant — jamais
// inline sur le thread de commande.
#[tauri::command]
pub async fn clean_browser_cache_path(browser_path: String) -> Result<f64, String> {
    tokio::task::spawn_blocking(move || clean_browser_cache_path_blocking(browser_path))
        .await
        .map_err(|e| e.to_string())?
}

/// Vérifie que le chemin correspond à un dossier de cache navigateur connu
/// (mêmes emplacements que get_browser_cache_info_blocking). Ce garde-fou
/// est nécessaire car clean_browser_cache_path est une commande Tauri
/// invocable directement — le check `..` seul ne bloque qu'une traversée
/// relative, pas un chemin absolu arbitraire (ex. "C:\Users\X\Documents"),
/// et clean_browser_cache_path_blocking exécute un `Remove-Item -Recurse
/// -Force` sur TOUT le contenu du dossier fourni. Le frontend actuel
/// (DiagTabCleaner.vue) ne propose que les chemins de get_browser_cache_info,
/// mais le backend est la vraie frontière de confiance, pas l'UI.
fn is_known_browser_cache_path(canonical: &std::path::Path) -> bool {
    let raw = canonical.to_string_lossy().to_lowercase().replace('/', "\\");
    let path_lower = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default().to_lowercase();
    let roaming = std::env::var("APPDATA").unwrap_or_default().to_lowercase();
    if local.is_empty() {
        return false;
    }
    let fixed = [
        format!(r"{}\google\chrome\user data\default\cache", local),
        format!(r"{}\microsoft\edge\user data\default\cache", local),
        format!(r"{}\bravesoftware\brave-browser\user data\default\cache", local),
        format!(r"{}\vivaldi\user data\default\cache", local),
        format!(r"{}\opera software\opera stable\cache", roaming),
    ];
    if fixed.iter().any(|k| path_lower == k.as_str()) {
        return true;
    }
    // Firefox : "<local>\Mozilla\Firefox\Profiles\<profil>\cache2" — le nom
    // de profil varie, donc on vérifie le préfixe + suffixe plutôt qu'une
    // égalité exacte.
    let ff_prefix = format!(r"{}\mozilla\firefox\profiles\", local);
    path_lower.starts_with(&ff_prefix) && path_lower.ends_with(r"\cache2")
}

fn clean_browser_cache_path_blocking(browser_path: String) -> Result<f64, String> {
    if browser_path.is_empty() || browser_path.contains("..") {
        return Err("Chemin invalide".to_string());
    }
    // Canonicalize + whitelist : voir is_known_browser_cache_path. Sans ce
    // garde, un chemin absolu arbitraire (pas besoin de "..") pouvait faire
    // vider n'importe quel dossier via Remove-Item -Recurse -Force.
    let canonical = std::fs::canonicalize(&browser_path)
        .map_err(|e| format!("Chemin inaccessible {}: {}", browser_path, e))?;
    if !is_known_browser_cache_path(&canonical) {
        return Err(format!("Chemin non reconnu comme cache navigateur: {}", canonical.display()));
    }
    let safe = browser_path.replace('\'', "''");
    let script = format!(r#"
$before = (Get-ChildItem '{safe}' -Recurse -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum
Remove-Item '{safe}\*' -Recurse -Force -ErrorAction SilentlyContinue
$after  = (Get-ChildItem '{safe}' -Recurse -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum
[math]::Round(($before - $after) / 1MB, 2)"#);
    let out = ps(&script)?;
    Ok(out.trim().parse::<f64>().unwrap_or(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── Scanners natifs (gros fichiers, doublons, visualiseur) ───────────────
    // Un seul test pour les trois : SCAN_CANCEL est global, des tests
    // parallèles se réinitialiseraient mutuellement le drapeau.

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nitrite_scan_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        d
    }

    /// Mesure manuelle (lecture seule) : `cargo test --lib big_files_on_c_drive -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn big_files_on_c_drive_completes_without_timeout() {
        // Pas de borne de durée : le parcours dépend du disque (≈ 3-4 min en
        // debug pour 1,5 M de fichiers). Ce qui compte : il va au bout, là où
        // l'ancien ps() coupait à 180 s.
        let t = std::time::Instant::now();
        let top = get_big_files_blocking(r"C:\".into(), 20, Some(vec![r"C:\Windows".into()])).unwrap();
        println!("C:\\ : {} fichiers en {:?}, plus gros = {:?}", top.len(), t.elapsed(), top.first().map(|f| &f.path));
        assert_eq!(top.len(), 20);
    }

    #[test]
    fn exclusion_matches_component_boundary_only() {
        let ex = vec![norm_excl(r"C:\Temp\")];
        assert!(is_excluded(Path::new(r"C:\Temp"), &ex));
        assert!(is_excluded(Path::new(r"c:\temp\a.iso"), &ex));
        assert!(!is_excluded(Path::new(r"C:\Temporaire\a.iso"), &ex));
    }

    #[test]
    fn native_scanners_find_sizes_duplicates_and_skip_junction_loops() {
        let d = scratch("all");
        let big = vec![7u8; 200_000];
        std::fs::write(d.join("a.bin"), &big).unwrap();
        std::fs::write(d.join("sub").join("b.bin"), &big).unwrap(); // doublon exact
        let mut other = big.clone();
        other[150_000] = 8; // même taille, même début, contenu différent
        std::fs::write(d.join("sub").join("c.bin"), &other).unwrap();
        std::fs::write(d.join("small.txt"), b"x").unwrap();
        // Jonction qui reboucle sur le dossier racine : ne doit pas être suivie.
        #[cfg(target_os = "windows")]
        {
            let link = d.join("sub").join("boucle");
            let st = std::process::Command::new("cmd")
                .args(["/c", "mklink", "/J", &link.to_string_lossy(), &d.to_string_lossy()])
                .output().unwrap();
            assert!(st.status.success(), "mklink /J a échoué");
        }
        let root = d.to_string_lossy().to_string();

        let top = get_big_files_blocking(root.clone(), 2, None).unwrap();
        assert_eq!(top.len(), 2);
        assert!(top.iter().all(|f| f.size_bytes == 200_000));
        assert!(top.iter().all(|f| !f.path.contains("boucle")));

        let groups = find_duplicates_sync(root.clone(), 1, "hash".into()).unwrap();
        assert_eq!(groups.len(), 1, "seuls a.bin et b.bin sont identiques");
        assert_eq!(groups[0].count, 2);
        assert_eq!(groups[0].wasted_bytes, 200_000);

        let tree = get_disk_tree_sync(root.clone(), 3).unwrap();
        assert_eq!(tree.size_bytes, 600_001, "la jonction ne doit pas être comptée");

        SCAN_CANCEL.store(true, Ordering::SeqCst);
        assert!(disk_dir_size(&d).is_err(), "annulation prise en compte");
        SCAN_CANCEL.store(false, Ordering::SeqCst);

        assert!(get_big_files_blocking(d.join("absent").to_string_lossy().to_string(), 5, None).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    // ── is_system_path_blocked ────────────────────────────────────────────────

    #[test]
    fn blocks_windows_directory() {
        // Chemin normal (sans préfixe verbatim)
        assert!(is_system_path_blocked(Path::new(r"C:\Windows\System32\kernel32.dll")));
    }

    #[test]
    fn blocks_windows_verbatim_prefix() {
        // canonicalize() sur Windows retourne \\?\C:\... via GetFinalPathNameByHandleW
        assert!(is_system_path_blocked(Path::new(r"\\?\C:\Windows\System32\kernel32.dll")));
    }

    #[test]
    fn blocks_program_files() {
        assert!(is_system_path_blocked(Path::new(r"C:\Program Files\SomeApp\file.exe")));
        assert!(is_system_path_blocked(Path::new(r"\\?\C:\Program Files\SomeApp\file.exe")));
    }

    #[test]
    fn blocks_program_files_x86() {
        assert!(is_system_path_blocked(Path::new(r"C:\Program Files (x86)\App\bin.dll")));
    }

    #[test]
    fn blocks_programdata() {
        assert!(is_system_path_blocked(Path::new(r"C:\ProgramData\Microsoft\file.log")));
    }

    #[test]
    fn blocks_system_volume_information() {
        assert!(is_system_path_blocked(Path::new(r"C:\System Volume Information\WPSettings.dat")));
    }

    #[test]
    fn allows_user_documents() {
        assert!(!is_system_path_blocked(Path::new(r"C:\Users\Momo\Documents\report.docx")));
    }

    #[test]
    fn allows_other_drive() {
        // D:\Windows ne doit pas être bloqué (drive cible en WinPE)
        assert!(!is_system_path_blocked(Path::new(r"D:\Windows\System32\ntdll.dll")));
    }

    #[test]
    fn allows_user_downloads() {
        assert!(!is_system_path_blocked(Path::new(r"C:\Users\Momo\Downloads\file.zip")));
        assert!(!is_system_path_blocked(Path::new(r"\\?\C:\Users\Momo\Downloads\file.zip")));
    }

    #[test]
    fn case_insensitive() {
        // La comparaison doit être insensible à la casse
        assert!(is_system_path_blocked(Path::new(r"c:\WINDOWS\system32\notepad.exe")));
        assert!(is_system_path_blocked(Path::new(r"C:\windows\SYSTEM32\notepad.exe")));
    }

    // ── is_known_browser_cache_path ───────────────────────────────────────────

    #[test]
    fn known_chrome_cache_path_allowed() {
        let local = std::env::var("LOCALAPPDATA").unwrap();
        let p = format!(r"{}\Google\Chrome\User Data\Default\Cache", local);
        assert!(is_known_browser_cache_path(Path::new(&p)));
    }

    #[test]
    fn known_firefox_profile_cache2_allowed() {
        let local = std::env::var("LOCALAPPDATA").unwrap();
        let p = format!(r"{}\Mozilla\Firefox\Profiles\abc123.default-release\cache2", local);
        assert!(is_known_browser_cache_path(Path::new(&p)));
    }

    #[test]
    fn firefox_profile_root_not_cache2_is_blocked() {
        // Le dossier profil racine (pas son sous-dossier cache2) contient
        // places.sqlite/logins.json/etc — ne doit PAS être reconnu comme
        // cache. C'est exactement le bug corrigé dans
        // get_browser_cache_info_blocking (pointait sur "Profiles" entier).
        let local = std::env::var("LOCALAPPDATA").unwrap();
        let p = format!(r"{}\Mozilla\Firefox\Profiles\abc123.default-release", local);
        assert!(!is_known_browser_cache_path(Path::new(&p)));
        let profiles_root = format!(r"{}\Mozilla\Firefox\Profiles", local);
        assert!(!is_known_browser_cache_path(Path::new(&profiles_root)));
    }

    #[test]
    fn arbitrary_user_folder_is_blocked() {
        assert!(!is_known_browser_cache_path(Path::new(r"C:\Users\Momo\Documents")));
        assert!(!is_known_browser_cache_path(Path::new(r"C:\Windows")));
        assert!(!is_known_browser_cache_path(Path::new(r"C:\Users\Momo\Desktop")));
    }
}
