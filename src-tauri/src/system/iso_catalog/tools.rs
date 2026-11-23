//! Port de TokaISO 7.9 (MIT, Cédrick Roy)
//! Créneaux de téléchargement, conseiller, suivis post-installation (workflows.py),
//! compatibilité avec ce PC (deployment.py), vérification d'un fichier, d'une
//! signature Authenticode (authenticode.py) et d'un manifeste OpenPGP (manifest.py).

use super::common::*;
use crate::maintenance::commands::execute_system_command;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const STEPS: [&str; 5] = [
    "Vérifier le matériel et installer le pilote réseau.",
    "Installer les mises à jour du système et redémarrer au besoin.",
    "Installer les composants requis et les applications.",
    "Restaurer les fichiers personnels et valider leur accès.",
    "Créer un nouveau point de restauration ou une sauvegarde.",
];

// ─── Créneaux horaires ──────────────────────────────────────────────────────

pub fn default_schedule() -> Value {
    json!({"enabled": false, "start": "22:00", "end": "07:00", "retry": false})
}

fn timecode(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 5 && b[2] == b':' && b[..2].iter().chain(&b[3..]).all(u8::is_ascii_digit)
        && value[..2].parse::<u8>().is_ok_and(|h| h < 24) && value[3..].parse::<u8>().is_ok_and(|m| m < 60)
}

pub fn schedule_data(payload: &Value) -> Res<Value> {
    let start = str_of(payload, "start");
    let end = str_of(payload, "end");
    let enabled = payload.get("enabled") == Some(&json!(true));
    if !timecode(start) || !timecode(end) || (enabled && start == end) {
        return Err("Choisis deux heures différentes pour la plage horaire.".into());
    }
    Ok(json!({"enabled": enabled, "start": start, "end": end, "retry": payload.get("retry") == Some(&json!(true))}))
}

pub fn in_window_at(schedule: &Value, moment: &str) -> bool {
    if schedule.get("enabled") != Some(&json!(true)) {
        return true;
    }
    let (a, b) = (str_of(schedule, "start"), str_of(schedule, "end"));
    if a < b { a <= moment && moment < b } else { moment >= a || moment < b }
}

pub fn in_window(schedule: &Value) -> bool {
    use chrono::Timelike;
    let now = chrono::Local::now();
    in_window_at(schedule, &format!("{:02}:{:02}", now.hour(), now.minute()))
}

// ─── Conseiller ─────────────────────────────────────────────────────────────

fn normalized(value: &str) -> String {
    value.to_lowercase().chars().map(|c| match c {
        'à' | 'â' | 'ä' | 'á' => 'a', 'é' | 'è' | 'ê' | 'ë' => 'e', 'î' | 'ï' | 'í' => 'i',
        'ô' | 'ö' | 'ó' => 'o', 'ù' | 'û' | 'ü' | 'ú' => 'u', 'ç' => 'c', 'ÿ' => 'y', other => other,
    }).collect()
}

pub fn advisor(items: &[Value], payload: &Value) -> Res<Value> {
    let get = |k: &str, d: &'static str| payload.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
    let (goal, arch, language, firmware) = (get("goal", "windows"), get("arch", "all"), get("language", "all"), get("firmware", "all"));
    let families: &[&str] = match goal.as_str() {
        "windows" => &["Windows"],
        "linux" => &["Linux", "Linux Gaming", "Archives et systèmes légers"],
        "rescue" => &["Live PE et récupération", "Secours et diagnostic", "USB, démarrage et déploiement"],
        "server" => &["Serveurs et stockage", "Hyperviseur"],
        _ => &[],
    };
    if families.is_empty() || !["all", "x64", "x86", "ARM64"].contains(&arch.as_str())
        || !["all", "fr", "en"].contains(&language.as_str()) || !["all", "BIOS", "UEFI"].contains(&firmware.as_str()) {
        return Err("Vérifie les filtres de compatibilité.".into());
    }
    let fr = regex::Regex::new(r"francais|french|fr-fr|fr-ca|(?:^|[ /_])fr_").map_err(|e| e.to_string())?;
    let en = regex::Regex::new(r"anglais|english|en-us|en-gb|(?:^|[ /_])en_").map_err(|e| e.to_string())?;
    let mut matches: Vec<(i32, Value)> = Vec::new();
    for item in items.iter().filter(|i| families.contains(&str_of(i, "family"))) {
        let legacy = item.get("tags").and_then(Value::as_array).is_some_and(|t| t.iter().any(|v| v == "legacy"));
        for (index, source) in sources_of(item).iter().enumerate() {
            let kind = str_of(source, "kind");
            let portal = kind == "portal" && str_of(source, "label").contains("ISO") && str_of(item, "name").contains("ISO");
            let format = str_of(source, "format").to_uppercase();
            if !portal && (kind == "portal" || !["ISO", "IMG", "ISO.XZ", "IMG.XZ"].contains(&format.as_str())) {
                continue;
            }
            let text = normalized(&["label", "language", "filename", "url"].iter().map(|k| str_of(source, k)).collect::<Vec<_>>().join(" "));
            let known_arch: Vec<&str> = str_of(source, "arch").split('/').map(str::trim).collect();
            if arch != "all" && !known_arch.contains(&arch.as_str()) {
                continue;
            }
            let lang = if fr.is_match(&text) { "fr" } else if en.is_match(&text) { "en" } else { "" };
            if language != "all" && lang != language {
                continue;
            }
            let boot = if legacy || text.contains("bios seul") { "BIOS" } else if text.contains("uefi") { "UEFI" } else { "" };
            if firmware != "all" && !boot.is_empty() && boot != firmware {
                continue;
            }
            let score = (if kind == "official" { 3 } else if portal { -1 } else { 0 })
                + (if !boot.is_empty() && boot == firmware { 3 } else { 0 })
                + i32::from(!str_of(source, "sha256").is_empty());
            matches.push((score, json!({"id": item["id"], "name": item["name"], "source": index, "label": str_of(source, "label"),
                "boot": boot, "arch": str_of(source, "arch"), "format": str_of(source, "format"), "lang": lang, "portal": portal, "kind": kind})));
        }
    }
    matches.sort_by_key(|(score, _)| -score);
    let count = matches.len();
    Ok(json!({"results": matches.into_iter().take(100).map(|(_, r)| r).collect::<Vec<_>>(), "count": count}))
}

// ─── Suivis post-installation ───────────────────────────────────────────────

pub struct Checklists {
    path: PathBuf,
    values: Mutex<Map<String, Value>>,
}

impl Checklists {
    pub fn new(home: &Path) -> Self {
        let path = home.join("checklists.json");
        let values = json_read(&path).and_then(|v| v.as_object().cloned()).unwrap_or_default();
        Self { path, values: Mutex::new(values) }
    }

    fn steps_from(requested: Option<&Value>) -> Res<Vec<Value>> {
        let list = requested.and_then(Value::as_array).filter(|l| (1..=40).contains(&l.len()))
            .filter(|l| l.iter().all(|s| s.is_object() && !label(s.get("label"), 200).is_empty()))
            .ok_or("Étapes de suivi invalides.")?;
        Ok(list.iter().map(|s| json!({"label": label(s.get("label"), 200), "done": s.get("done") == Some(&json!(true))})).collect())
    }

    pub fn action(&self, action: &str, payload: &Value) -> Res<Value> {
        let mut values = self.values.lock().unwrap_or_else(|p| p.into_inner());
        let mut rows = values.clone();
        if action == "list" {
            return Ok(json!({"lists": rows.values().collect::<Vec<_>>(), "steps": STEPS}));
        }
        let key;
        if action == "create" || action == "import" {
            let data = if action == "import" {
                let file = file_input(payload.get("path"))?;
                if file.metadata().map(|m| m.len()).unwrap_or(u64::MAX) > 65536 {
                    return Err("Le fichier de suivi est trop volumineux.".into());
                }
                let data = json_read(&file).unwrap_or(Value::Null);
                if str_of(&data, "type") != "tokaiso-checklist" || data.get("version").and_then(Value::as_i64) != Some(1) {
                    return Err("Fichier de suivi non reconnu.".into());
                }
                data
            } else {
                payload.clone()
            };
            let name = label(data.get("name"), 90);
            if name.is_empty() || rows.len() >= 100 {
                return Err("Donne un nom au suivi. La limite est de 100 suivis.".into());
            }
            key = uuid::Uuid::new_v4().simple().to_string();
            let steps = if action == "import" || data.get("steps").is_some() {
                Self::steps_from(data.get("steps"))?
            } else {
                STEPS.iter().map(|s| json!({"label": s, "done": false})).collect()
            };
            rows.insert(key.clone(), json!({"id": key, "name": name, "steps": steps}));
        } else {
            key = payload.get("id").and_then(Value::as_str).filter(|k| rows.contains_key(*k))
                .ok_or("Suivi de réinstallation introuvable.")?.to_string();
            match action {
                "delete" => {
                    rows.remove(&key);
                }
                "toggle" => {
                    let index = payload.get("index").and_then(Value::as_u64).map(|i| i as usize);
                    let steps = rows.get_mut(&key).and_then(|r| r.get_mut("steps")).and_then(Value::as_array_mut).ok_or("Étape introuvable.")?;
                    let step = index.and_then(|i| steps.get_mut(i)).ok_or("Étape introuvable.")?;
                    step["done"] = json!(payload.get("done") == Some(&json!(true)));
                }
                _ => return Err("Commande de suivi inconnue.".into()),
            }
        }
        json_write(&self.path, &Value::Object(rows.clone()))?;
        *values = rows.clone();
        Ok(json!({"lists": rows.values().collect::<Vec<_>>(), "id": key}))
    }

    /// Exporte un suivi vers un fichier JSON choisi par l'utilisateur.
    pub fn export(&self, id: &str, target: &str) -> Res<()> {
        let values = self.values.lock().unwrap_or_else(|p| p.into_inner());
        let row = values.get(id).ok_or("Suivi de réinstallation introuvable.")?;
        let data = json!({"type": "tokaiso-checklist", "version": 1, "name": row["name"], "steps": row["steps"]});
        json_write(&output_path(target)?, &data)
    }
}

/// Destination d'export : chemin absolu en .json dans un dossier existant.
pub fn output_path(target: &str) -> Res<PathBuf> {
    let path = Path::new(target);
    let parent = path.parent().filter(|p| p.is_dir());
    let json_ext = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if !path.is_absolute() || parent.is_none() || !json_ext {
        return Err("Choisis un fichier .json dans un dossier existant.".into());
    }
    Ok(path.to_path_buf())
}

// ─── Profil machine et compatibilité ────────────────────────────────────────

pub fn machine_profile() -> Value {
    let arch = std::env::var("PROCESSOR_ARCHITEW6432").or_else(|_| std::env::var("PROCESSOR_ARCHITECTURE")).unwrap_or_default();
    let arch = match arch.to_uppercase().as_str() { "AMD64" | "X86_64" => "x64", "ARM64" | "AARCH64" => "ARM64", "X86" => "x86", _ => "Inconnu" };
    #[cfg(windows)]
    let firmware = {
        use windows::Win32::System::SystemInformation::{GetFirmwareType, FIRMWARE_TYPE};
        let mut kind = FIRMWARE_TYPE::default();
        // SAFETY : `kind` est une variable locale valide pour toute la durée de l'appel.
        match unsafe { GetFirmwareType(&mut kind) } {
            Ok(()) if kind.0 == 1 => "BIOS",
            Ok(()) if kind.0 == 2 => "UEFI",
            _ => "Inconnu",
        }
    };
    #[cfg(not(windows))]
    let firmware = "Inconnu";
    let winget = std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("winget.exe").is_file()));
    json!({"arch": arch, "firmware": firmware, "winget": winget})
}

pub fn physical_memory() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory()
}

pub fn disk_space(folder: &Path) -> Option<u64> {
    let existing = folder.ancestors().find(|p| p.is_dir())?;
    let real = display_path(&existing.canonicalize().ok()?).to_lowercase();
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks.list().iter()
        .filter(|d| real.starts_with(&d.mount_point().to_string_lossy().to_lowercase()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.available_space())
}

/// Architectures annoncées par une source (port de `architectures()`).
pub fn architectures(value: &str) -> Vec<&'static str> {
    let text = value.to_uppercase();
    if ["MULTI", "TOUTES", "ALL"].contains(&text.as_str()) {
        return Vec::new();
    }
    let has = |pattern: &str| regex::Regex::new(pattern).is_ok_and(|re| re.is_match(&text));
    // Le motif x86 de TokaISO exclut « x86_64 » / « x86-64 » ; la crate regex n'a pas
    // d'assertion négative, on retire donc ces formes avant de chercher.
    let without64 = regex::Regex::new(r"X86[_-]64").map(|re| re.replace_all(&text, "").to_string()).unwrap_or_default();
    let x86 = regex::Regex::new(r"\bX86\b|I[3-6]86|IA32").is_ok_and(|re| re.is_match(&without64));
    let mut out = Vec::new();
    if has(r"ARM64|AARCH64") { out.push("ARM64"); }
    if has(r"X64|AMD64|X86[_-]64") { out.push("x64"); }
    if x86 { out.push("x86"); }
    if has(r"ARMHF|ARM32|ARMV[67]|ARM\s*\(32 BITS\)") { out.push("ARM32"); }
    out
}

fn requirements(item: &Value) -> Value {
    let name = str_of(item, "name");
    let win11 = name.strip_prefix("Windows 11").is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace));
    if str_of(item, "family") == "Windows" && win11 {
        return json!({"ram": 4_000_000_000u64, "storage": 64_000_000_000u64, "firmware": "UEFI",
                      "url": "https://learn.microsoft.com/en-us/windows/whats-new/windows-11-requirements"});
    }
    if str_of(item, "id") == "19" {
        return json!({"ram": 6_000_000_000u64, "storage": 25_000_000_000u64, "url": "https://ubuntu.com/download/desktop"});
    }
    json!({})
}

pub fn compatibility(item: &Value, slot: usize, folder: &Path, local: Option<&Value>) -> Res<Value> {
    let source = source_at(item, Some(slot))?;
    let profile = machine_profile();
    let (ram, free) = (physical_memory(), disk_space(folder));
    let arch = architectures(str_of(source, "arch"));
    let need = requirements(item);
    let mut boot: Vec<String> = Vec::new();
    if item.get("tags").and_then(Value::as_array).is_some_and(|t| t.iter().any(|v| v == "legacy")) {
        boot = vec!["BIOS".into()];
    }
    if matches!(str_of(source, "boot"), "BIOS" | "UEFI") {
        boot = vec![str_of(source, "boot").into()];
    }
    if let Some(path) = local.filter(|v| v.as_str().is_some_and(|s| !s.is_empty())) {
        let file = file_input(Some(path))?;
        if file.extension().is_some_and(|e| e.eq_ignore_ascii_case("iso")) {
            let found: Vec<String> = super::library::inspect(&file)?["boot"].as_array().into_iter().flatten()
                .filter_map(Value::as_str).map(str::to_string).collect();
            if !found.is_empty() {
                boot = found;
            }
        }
    }
    if boot.is_empty() && !str_of(&need, "firmware").is_empty() {
        boot = vec![str_of(&need, "firmware").into()];
    }
    let native = str_of(&profile, "arch").to_string();
    let mode = str_of(&profile, "firmware").to_string();
    let size = u64_of(source, "size");
    let need_ram = u64_of(&need, "ram");
    let check = |known: bool, ok: bool| if known { json!(ok) } else { Value::Null };
    Ok(json!({"name": item["name"], "source": str_of(source, "label"), "profile": profile, "ram": ram, "free": free,
        "arch": arch, "boot": boot, "size": size, "requirements": need,
        "checks": {
            "architecture": check(!arch.is_empty() && native != "Inconnu", arch.contains(&native.as_str())),
            "firmware": check(!boot.is_empty() && mode != "Inconnu", boot.contains(&mode)),
            "memory": check(ram > 0 && need_ram > 0, ram >= need_ram),
            "space": check(free.is_some() && size > 0, free.unwrap_or(0) >= size),
        }}))
}

// ─── Vérification d'un fichier et signatures ────────────────────────────────

fn ps_literal(value: &str) -> String {
    // JSON dans une chaîne PowerShell entre apostrophes, décodé par ConvertFrom-Json.
    let json = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into());
    let mut out = String::from("'");
    out.push_str(&json.replace('\'', "''"));
    out.push('\'');
    out
}

/// Signature Authenticode via Get-AuthenticodeSignature (la crate windows du projet
/// n'active pas WinTrust ; Cargo.toml n'est pas modifié).
pub fn authenticode(path: &Path) -> Value {
    let mut script = String::from("[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); $ErrorActionPreference='Stop'; $p = ");
    script.push_str(&ps_literal(&display_path(path)));
    script.push_str(" | ConvertFrom-Json; $s = Get-AuthenticodeSignature -LiteralPath $p; $c = $s.SignerCertificate; \
        [pscustomobject]@{status=[string]$s.Status; signer=$(if($c){$c.GetNameInfo('SimpleName',$false)}else{''}); \
        thumbprint=$(if($c){$c.Thumbprint}else{''})} | ConvertTo-Json -Compress");
    let unavailable = json!({"status": "Unavailable", "signer": "", "thumbprint": ""});
    let Ok(out) = execute_system_command("powershell", &["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", &script], 30) else {
        return unavailable;
    };
    let Ok(row) = serde_json::from_str::<Value>(out.stdout.trim().trim_start_matches('\u{feff}')) else { return unavailable };
    let status = match str_of(&row, "status") { s @ ("Valid" | "NotSigned" | "HashMismatch" | "NotTrusted") => s, _ => "Unavailable" };
    json!({"status": status, "signer": str_of(&row, "signer"), "thumbprint": str_of(&row, "thumbprint")})
}

pub fn verify_file(items: &[Value], payload: &Value) -> Res<Value> {
    let path = file_input(payload.get("path"))?;
    let before = path.metadata().map_err(|e| e.to_string())?;
    let digest = file_digest(&path, None)?.unwrap_or_default();
    let after = path.metadata().map_err(|e| e.to_string())?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        return Err("Le fichier a changé pendant la vérification.".into());
    }
    let expected = checksum(payload.get("sha256"))?;
    let matches: Vec<&Value> = items.iter().filter(|i| sources_of(i).iter().any(|s| str_of(s, "sha256") == digest)).map(|i| &i["name"]).collect();
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let signature = ["exe", "dll", "msi", "ps1", "sys", "cat", "cab"].contains(&ext.as_str()).then(|| authenticode(&path));
    Ok(json!({"name": path.file_name().map(|n| n.to_string_lossy().to_string()), "size": before.len(), "sha256": digest,
        "expected": expected, "matches": matches, "match": (!expected.is_empty()).then(|| digest == expected), "signature": signature}))
}

fn gnupg() -> Option<PathBuf> {
    let roots = ["ProgramFiles", "ProgramFiles(x86)"].iter().filter_map(std::env::var_os).map(PathBuf::from);
    for root in roots {
        for relative in ["GnuPG/bin/gpg.exe", "GnuPG/bin/gpg2.exe", "Gpg4win/bin/gpg.exe", "Git/usr/bin/gpg.exe"] {
            if root.join(relative).is_file() {
                return Some(root.join(relative));
            }
        }
    }
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join("gpg.exe")).find(|p| p.is_file())
}

/// Empreinte du manifeste (formats BSD « SHA256 (nom) = … » et GNU « … nom »).
pub fn manifest_entry(text: &str, name: &str) -> Res<String> {
    let bsd = regex::Regex::new(r"(?i)^SHA256 \((.+)\) = ([0-9a-f]{64})$").map_err(|e| e.to_string())?;
    let gnu = regex::Regex::new(r"^([0-9a-fA-F]{64})[ \t]+\*?(.+)$").map_err(|e| e.to_string())?;
    let mut matches = std::collections::HashSet::new();
    for line in text.lines() {
        let (filename, digest) = if let Some(c) = bsd.captures(line) {
            (c[1].to_string(), c[2].to_string())
        } else if let Some(c) = gnu.captures(line) {
            (c[2].to_string(), c[1].to_string())
        } else {
            continue;
        };
        if filename.replace('\\', "/").rsplit('/').next() == Some(name) {
            matches.insert(digest.to_lowercase());
        }
    }
    match (matches.len(), matches.into_iter().next()) {
        (1, Some(d)) => Ok(d),
        _ => Err("Le manifeste doit contenir une seule empreinte SHA-256 pour ce nom de fichier.".into()),
    }
}

fn run_gpg(gpg: &str, args: &[&str]) -> Res<crate::maintenance::commands::CommandResult> {
    execute_system_command(gpg, args, 15)
        .map_err(|_| "La vérification OpenPGP a dépassé le délai prévu ou GnuPG n’a pas pu démarrer.".to_string())
}

pub fn verify_manifest(payload: &Value, home: &Path) -> Res<Value> {
    let binary = gnupg().ok_or("GnuPG est requis pour vérifier la signature. Installe Gpg4win, puis réessaie.")?;
    let fingerprint: String = str_of(payload, "fingerprint").split_whitespace().collect::<String>().to_uppercase();
    if !((fingerprint.len() == 40 || fingerprint.len() == 64) && fingerprint.bytes().all(|b| b.is_ascii_hexdigit())) {
        return Err("Entre l’empreinte complète de la clé publiée par l’éditeur.".into());
    }
    let media = file_input(payload.get("media"))?;
    let temp = home.join(format!("pgp-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let outcome = (|| -> Res<String> {
        for (field, limit) in [("manifest", 8u64 << 20), ("signature", 1 << 20), ("key", 1 << 20)] {
            let path = file_input(payload.get(field))?;
            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
            if size == 0 || size > limit {
                return Err("Le manifeste, la signature ou la clé est vide ou trop volumineux.".into());
            }
            std::fs::copy(&path, temp.join(field)).map_err(|e| e.to_string())?;
        }
        let gpg = binary.to_string_lossy().to_string();
        let file = |n: &str| temp.join(n).to_string_lossy().to_string();
        let home_dir = temp.to_string_lossy().to_string();
        let base = ["--no-options", "--homedir", home_dir.as_str(), "--batch", "--no-tty", "--no-auto-key-retrieve", "--status-fd", "1"];
        let key = file("key");
        if !run_gpg(&gpg, &[&base[..], &["--import", key.as_str()]].concat())?.success {
            return Err("La clé publique OpenPGP n’a pas pu être importée.".into());
        }
        let (sig, manifest) = (file("signature"), file("manifest"));
        let checked = run_gpg(&gpg, &[&base[..], &["--verify", sig.as_str(), manifest.as_str()]].concat())?;
        let invalid = ["BADSIG", "ERRSIG", "REVKEYSIG", "EXPKEYSIG", "EXPSIG", "KEYREVOKED", "SIGEXPIRED", "KEYEXPIRED"];
        let lines: Vec<&str> = checked.stdout.lines().collect();
        let bad = lines.iter().filter_map(|l| l.strip_prefix("[GNUPG:] "))
            .any(|rest| invalid.iter().any(|w| rest.split_whitespace().next() == Some(*w)));
        let valid = lines.iter().filter_map(|l| l.strip_prefix("[GNUPG:] VALIDSIG ")).any(|rest| {
            let fields: Vec<&str> = rest.split_whitespace().collect();
            fields.first() == Some(&fingerprint.as_str()) || fields.get(9) == Some(&fingerprint.as_str())
        });
        if !checked.success || bad || !valid {
            return Err("Signature invalide ou différente de la clé de l’éditeur indiquée.".into());
        }
        let text = String::from_utf8_lossy(&std::fs::read(temp.join("manifest")).map_err(|e| e.to_string())?).to_string();
        manifest_entry(text.trim_start_matches('\u{feff}'), &media.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
    })();
    let _ = std::fs::remove_dir_all(&temp);
    let expected = outcome?;
    let before = media.metadata().map_err(|e| e.to_string())?;
    let digest = file_digest(&media, None)?.unwrap_or_default();
    let after = media.metadata().map_err(|e| e.to_string())?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        return Err("Le fichier a changé pendant la vérification.".into());
    }
    Ok(json!({"name": media.file_name().map(|n| n.to_string_lossy().to_string()), "fingerprint": fingerprint, "sha256": digest, "expected": expected, "match": digest == expected}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_rules() {
        assert!(schedule_data(&json!({"enabled": true, "start": "22:00", "end": "22:00"})).is_err());
        assert!(schedule_data(&json!({"enabled": false, "start": "25:00", "end": "07:00"})).is_err());
        let night = schedule_data(&json!({"enabled": true, "start": "22:00", "end": "07:00", "retry": true})).unwrap();
        assert!(in_window_at(&night, "23:30") && in_window_at(&night, "06:59"));
        assert!(!in_window_at(&night, "12:00"));
        let day = schedule_data(&json!({"enabled": true, "start": "09:00", "end": "17:00"})).unwrap();
        assert!(in_window_at(&day, "09:00") && !in_window_at(&day, "17:00"));
        assert!(in_window_at(&default_schedule(), "12:00"));
    }

    #[test]
    fn arch_parsing() {
        assert_eq!(architectures("x86 / x64"), ["x64", "x86"]);
        assert_eq!(architectures("x86_64"), ["x64"]);
        assert_eq!(architectures("ARMv6 / ARMv7 / ARM64"), ["ARM64", "ARM32"]);
        assert!(architectures("Multi").is_empty());
    }

    #[test]
    fn manifest_parsing() {
        let h = "b".repeat(64);
        let text = ["SHA256 (debian.iso) = ", &h, "\n", &"c".repeat(64), " *autre.iso\n"].concat();
        assert_eq!(manifest_entry(&text, "debian.iso").unwrap(), h);
        assert_eq!(manifest_entry(&text, "autre.iso").unwrap(), "c".repeat(64));
        assert!(manifest_entry(&text, "absent.iso").is_err());
        let twice = [h.as_str(), "  a.iso\n", &"d".repeat(64), "  a.iso\n"].concat();
        assert!(manifest_entry(&twice, "a.iso").is_err());
    }

    #[test]
    fn advisor_filters() {
        let items = super::super::catalog::base_items().unwrap();
        let out = advisor(&items, &json!({"goal": "windows", "arch": "x64", "language": "all", "firmware": "all"})).unwrap();
        assert!(u64_of(&out, "count") > 0);
        assert!(advisor(&items, &json!({"goal": "mars"})).is_err());
    }

    #[test]
    fn checklist_cycle() {
        let dir = std::env::temp_dir().join(["nitrite-iso-chk-", &uuid::Uuid::new_v4().simple().to_string()].concat());
        std::fs::create_dir_all(&dir).unwrap();
        let lists = Checklists::new(&dir);
        let created = lists.action("create", &json!({"name": "PC salon"})).unwrap();
        let id = str_of(&created, "id").to_string();
        assert_eq!(created["lists"][0]["steps"].as_array().unwrap().len(), 5);
        let toggled = lists.action("toggle", &json!({"id": id, "index": 1, "done": true})).unwrap();
        assert_eq!(toggled["lists"][0]["steps"][1]["done"], true);
        assert!(lists.action("toggle", &json!({"id": id, "index": 99, "done": true})).is_err());
        lists.action("delete", &json!({"id": id})).unwrap();
        assert!(lists.action("list", &json!({})).unwrap()["lists"].as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ps_literal_is_quoted() {
        assert_eq!(ps_literal("C:\\a'b"), "'\"C:\\\\a''b\"'");
    }
}
