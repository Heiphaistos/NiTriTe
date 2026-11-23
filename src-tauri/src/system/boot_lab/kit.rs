//! Trousse hors ligne : copie vérifiée des fichiers choisis, pilotes, liste WinGet, page « Commencer ici ».
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : workshop.py (Workshop.export, capture, checklist).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::labs::{lab_option, vm_labs, LabOption, PLATFORMS};
use super::{check_space, file_input, hashed_copy, is_reparse, new_id, suffix, R, MIB};

#[derive(Debug, Clone, Deserialize)]
pub struct KitRequest {
    pub paths: Vec<String>,
    pub destination: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ventoy: bool,
    #[serde(default)]
    pub drivers: bool,
    #[serde(default)]
    pub apps: bool,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub firmware: String,
    #[serde(default)]
    pub lab: Option<LabOption>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KitFile {
    pub name: String,
    pub filename: String,
    pub size: u64,
    pub install: String,
    pub guidance: String,
    pub location: String,
    pub export_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct KitInfo {
    pub version: u32,
    pub name: String,
    pub date: String,
    pub arch: String,
    pub firmware: String,
    pub notes: Vec<String>,
    pub files: Vec<KitFile>,
    pub lab: Option<LabOption>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KitResult {
    pub folder: String,
    pub count: usize,
    pub notes: Vec<String>,
}

fn guidance(install: &str) -> &'static str {
    match install {
        "boot" => "Média amorçable — vérifie le format",
        _ => "Fichier à emporter",
    }
}

/// Outil Windows lancé avec un délai ; renvoie le message de succès ou d'échec.
fn capture(executable: &str, args: &[String], success: &str, failure: &str) -> String {
    let path = if executable == "pnputil.exe" {
        Some(super::system32().join(executable))
    } else {
        std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(executable)).find(|f| f.is_file()))
    };
    match path.filter(|p| p.is_file()) {
        Some(exe) => match super::run(&exe, args, 105) {
            Ok(out) if out.success => success.into(),
            _ => failure.into(),
        },
        None => failure.into(),
    }
}

fn esc(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#x27;")
}

fn href(location: &str) -> String {
    location
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

const NETWORK_LAB_STEPS: [&str; 4] = [
    "Créer un réseau privé dans l’hyperviseur.",
    "Relier l’interface LAN d’OPNsense et le client Debian à ce réseau. Laisser le WAN déconnecté pendant la configuration initiale.",
    "Configurer le compte et le réseau du disque Debian GenericCloud avec cloud-init.",
    "Vérifier les adresses sur le réseau privé puis tester l’accès au pare-feu.",
];
const REINSTALL_STEPS: [&str; 4] = [
    "Sauvegarder les fichiers personnels. Si le disque est chiffré, conserver la clé de récupération en lieu sûr.",
    "Vérifier le média de démarrage choisi. Pour une clé Ventoy déjà préparée, copier les images comme fichiers ordinaires.",
    "Réinstaller sur le disque prévu et remettre les pilotes si nécessaire.",
    "Installer les applications et restaurer les fichiers personnels.",
];
const STYLE: &str = "body{font:17px/1.55 system-ui;max-width:55rem;margin:2rem auto;padding:0 1rem;background:#f1f4ef;color:#142820}h1{font-size:2.7rem;line-height:1}section{background:#fff;border-radius:1rem;padding:1.1rem 1.5rem;margin:1rem 0}li{margin:.5rem 0}a{color:#165e45}small{color:#3f5851}";

/// Page HTML autonome cochable (langue : français). Toutes les valeurs sont échappées.
pub fn checklist(info: &KitInfo) -> String {
    let mut files_html = String::new();
    for f in &info.files {
        files_html += "<li><label><input type=\"checkbox\"> <strong><a href=\"";
        files_html += &href(&f.location);
        files_html += "\">";
        files_html += &esc(&f.name);
        files_html += "</a></strong> · Présent<br><small>";
        files_html += &esc(&f.guidance);
        files_html += " · ";
        files_html += &esc(&f.filename);
        files_html += "</small></label></li>";
    }
    let notes: String = info.notes.iter().map(|n| ["<li>", &esc(n), "</li>"].concat()).collect();
    let labs = vm_labs();
    let lab = info.lab.as_ref().and_then(|l| labs.get(&l.id));
    let (heading, steps): (&str, Vec<String>) = if let Some(lab) = lab {
        ("Premier démarrage", lab.steps.clone())
    } else if info.name == "Mon premier labo réseau" {
        ("Configurer le labo", NETWORK_LAB_STEPS.map(String::from).to_vec())
    } else {
        ("Préparer et réinstaller", REINSTALL_STEPS.map(String::from).to_vec())
    };
    let link = lab
        .and_then(|l| l.reference.as_ref())
        .map(|r| ["<p><a href=\"", &esc(r), "\">Guide d’installation Gitea</a></p>"].concat())
        .unwrap_or_default();
    let ordered: String =
        steps.iter().map(|s| ["<li><label><input type=\"checkbox\"> ", &esc(s), "</label></li>"].concat()).collect();
    let target = match &info.lab {
        Some(l) => PLATFORMS.iter().find(|(k, _)| *k == l.platform).map(|(_, v)| v.to_string()).unwrap_or_default(),
        None => [info.arch.as_str(), " · ", info.firmware.as_str()].concat(),
    };
    let name = esc(&info.name);
    [
        "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\">\n<title>",
        &name, " · NiTriTe</title><style>", STYLE, "</style>\n<h1>", &name, "</h1><p>Trousse NiTriTe · ", &esc(&info.date),
        " · ", &esc(&target), "</p>\n<section><h2>", heading, "</h2><ol>", &ordered, "</ol>", &link,
        "</section>\n<section><h2>Fichiers</h2><ul>", &files_html,
        "</ul><p>Le fichier SHA256.txt contient les empreintes des fichiers réellement copiés.</p><ul>", &notes,
        "</ul></section>\n<footer>Port de TokaISO 7.9 (MIT, Cédrick Roy)</footer></html>",
    ]
    .concat()
}

fn safe_label(label: &str) -> String {
    let s: String = label.chars().map(|c| if c.is_alphanumeric() || c == '_' || " .-".contains(c) { c } else { '-' }).collect();
    let s = s.trim_matches([' ', '.', '-']).to_string();
    if s.is_empty() { "Trousse".into() } else { s }
}

/// Exporte la trousse dans un dossier « en cours » renommé seulement une fois complet.
pub fn export(req: &KitRequest) -> R<KitResult> {
    static BUSY: Mutex<()> = Mutex::new(());
    let _guard = BUSY.try_lock().map_err(|_| "Un export est déjà en cours.".to_string())?;
    if !(1..=24).contains(&req.paths.len()) {
        return Err("Choisis de 1 à 24 fichiers.".into());
    }
    let destination = PathBuf::from(&req.destination);
    if !destination.is_absolute() || !destination.is_dir() || is_reparse(&destination) {
        return Err("Choisis un dossier sur le support de destination.".into());
    }
    let mut files: Vec<PathBuf> = Vec::new();
    for p in &req.paths {
        let path = file_input(p)?;
        if is_reparse(&path) {
            return Err("Le fichier choisi est incompatible.".into());
        }
        if !files.contains(&path) {
            files.push(path);
        }
    }
    let sizes: Vec<u64> = files.iter().map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)).collect();
    check_space(&destination, &sizes, MIB, "Espace insuffisant sur le support pour la trousse.")?;
    let label: String = req.name.chars().take(48).collect::<String>().trim().to_string();
    let label = if label.is_empty() { "Trousse".to_string() } else { label };
    let id = new_id();
    let folder = destination.join(["NiTriTe - ", &safe_label(&label), " - ", &id[..8]].concat());
    let stage = destination.join(["NiTriTe - export en cours - ", &id[..8]].concat());
    std::fs::create_dir(&stage).map_err(|e| e.to_string())?;
    // Un export interrompu reste identifiable (dossier « export en cours »), comme dans TokaISO.
    let mut copied = Vec::new();
    for (index, (path, size)) in files.iter().zip(&sizes).enumerate() {
        let ext = suffix(path);
        let install = if ext == ".iso" || ext == ".img" { "boot" } else { "local" };
        let media = req.ventoy && install == "boot";
        let number = (index + 1).to_string();
        let number = if number.len() < 2 { ["0", &number].concat() } else { number };
        let sub = Path::new(if media { "Ventoy" } else { "Fichiers" }).join(number);
        std::fs::create_dir_all(stage.join(&sub)).map_err(|e| e.to_string())?;
        let filename = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let target = stage.join(&sub).join(&filename);
        let digest = hashed_copy(path, &target)?;
        if std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0) != *size {
            return Err(["Le fichier ", &filename, " a changé pendant l’export."].concat());
        }
        let location = [sub.to_string_lossy().replace('\\', "/").as_str(), "/", &filename].concat();
        copied.push(KitFile {
            name: path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            filename,
            size: *size,
            install: install.into(),
            guidance: guidance(install).into(),
            location,
            export_sha256: digest,
        });
    }
    let mut notes = Vec::new();
    if req.drivers {
        let dir = stage.join("Pilotes");
        std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
        notes.push(capture(
            "pnputil.exe",
            &["/export-driver".into(), "*".into(), dir.to_string_lossy().to_string()],
            "Pilotes tiers exportés dans Pilotes.",
            "Export des pilotes indisponible. Lance NiTriTe en administrateur si Windows refuse cette opération.",
        ));
    }
    if req.apps {
        let out = stage.join("Applications-WinGet.json").to_string_lossy().to_string();
        notes.push(capture(
            "winget.exe",
            &["export", "--output", &out, "--include-versions", "--accept-source-agreements", "--disable-interactivity"].map(String::from),
            "Liste des applications WinGet exportée. Leur réinstallation via WinGet demande Internet.",
            "WinGet indisponible ou export incomplet sur ce PC.",
        ));
    }
    let info = KitInfo {
        version: 1,
        name: label,
        date: chrono::Local::now().date_naive().to_string(),
        arch: req.arch.chars().take(16).collect(),
        firmware: req.firmware.chars().take(16).collect(),
        notes: notes.clone(),
        files: copied,
        lab: lab_option(req.lab.as_ref()),
    };
    let write = |name: &str, text: String| std::fs::write(stage.join(name), text).map_err(|e| e.to_string());
    write("NiTriTe-hors-ligne.json", serde_json::to_string_pretty(&info).map_err(|e| e.to_string())?)?;
    write("SHA256.txt", info.files.iter().map(|f| [f.export_sha256.as_str(), "  ", &f.location, "\n"].concat()).collect())?;
    write("Commencer-ici.html", checklist(&info))?;
    std::fs::rename(&stage, &folder).map_err(|e| e.to_string())?;
    Ok(KitResult { folder: folder.to_string_lossy().to_string(), count: info.files.len(), notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_echappee_et_labo() {
        let info = KitInfo {
            version: 1,
            name: "<script>".into(),
            date: "2026-11-23".into(),
            arch: "x64".into(),
            firmware: "UEFI".into(),
            notes: vec![],
            files: vec![KitFile {
                name: "a&b".into(), filename: "a b.iso".into(), size: 1, install: "boot".into(),
                guidance: guidance("boot").into(), location: "Ventoy/01/a b.iso".into(), export_sha256: String::new(),
            }],
            lab: Some(LabOption { id: "web-lab".into(), platform: "vmware".into() }),
        };
        let html = checklist(&info);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("href=\"Ventoy/01/a%20b.iso\""));
        assert!(html.contains("a&amp;b"));
        assert!(html.contains("VMware Workstation"));
        assert!(html.contains("Guide d’installation Gitea"));
        assert_eq!(safe_label("a/b:c"), "a-b-c");
    }
}
