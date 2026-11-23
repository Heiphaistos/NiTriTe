//! Préparer, inspecter et vérifier la clé sélectionnée, avec aperçu avant toute opération.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : usb_media.py, media_tools.media_paths.

use super::disks::{self, UsbDisk};
use super::ventoy;
use super::{file_input, file_name, has_suffix, hex, is_reparse, sha256_bytes, stable_file_hash, stamp, Res, TempDir, MEDIA_SUFFIXES};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const OPERATIONS: &[&str] = &["install", "copy", "update", "storage", "verify-raw", "verify-ventoy"];
const RAW_SUFFIXES: &[&str] = &[".img", ".iso", ".bin", ".gz", ".xz", ".bz2", ".zip"];
const RAW_IMAGES: &[&str] = &[".img", ".iso", ".bin", ".img.gz", ".img.xz", ".img.bz2", ".iso.gz", ".iso.xz", ".iso.bz2", ".zip"];
/// ponytail: .gz/.xz/.bz2 demandent flate2/xz2/bzip2 (crates absentes du projet) : refusées à l'aperçu.
const COMPRESSED: &[&str] = &[".gz", ".xz", ".bz2"];
const COPY_CHUNK: usize = 4 * 1024 * 1024;
const USB_TIMEOUT: u64 = 14100;

/// Lecture brute en lecture seule de `\\.\PhysicalDriveN` (aucune écriture possible : GENERIC_READ).
pub const RAW_DISK_CSHARP: &str = r#"
using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

public sealed class TokaISORawDisk : IDisposable
{
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, ExactSpelling=true, SetLastError=true)]
    private static extern SafeFileHandle CreateFileW(string path, uint access, uint share,
        IntPtr security, uint mode, uint flags, IntPtr template);
    [DllImport("kernel32.dll", SetLastError=true)]
    private static extern bool ReadFile(SafeFileHandle handle, IntPtr buffer, uint length,
        out uint received, IntPtr overlapped);
    [DllImport("kernel32.dll", SetLastError=true)]
    private static extern IntPtr VirtualAlloc(IntPtr address, UIntPtr size, uint allocationType, uint protect);
    [DllImport("kernel32.dll", SetLastError=true)]
    private static extern bool VirtualFree(IntPtr address, UIntPtr size, uint freeType);
    private readonly SafeFileHandle handle;

    public TokaISORawDisk(int number)
    {
        if (number < 0) throw new ArgumentOutOfRangeException("number");
        handle = CreateFileW(@"\\.\PhysicalDrive" + number, 0x80000000, 3,
            IntPtr.Zero, 3, 0x20000000, IntPtr.Zero);
        if (handle.IsInvalid) {
            int error = Marshal.GetLastWin32Error();
            handle.Dispose();
            throw new Win32Exception(error);
        }
    }

    public void CopyTo(Stream destination, long count, int sectorSize)
    {
        const int blockSize = 4 * 1024 * 1024;
        if (count <= 0 || sectorSize < 512 || sectorSize > 65536 ||
            (sectorSize & (sectorSize - 1)) != 0) throw new ArgumentOutOfRangeException("sectorSize");
        var bytes = new byte[blockSize];
        IntPtr buffer = VirtualAlloc(IntPtr.Zero, new UIntPtr((uint)blockSize), 0x3000, 0x04);
        if (buffer == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
        try {
            while (count > 0) {
                var requested = (int)Math.Min(count, blockSize);
                uint sectors = (uint)((requested + sectorSize - 1) / sectorSize * sectorSize);
                uint received;
                if (!ReadFile(handle, buffer, sectors, out received, IntPtr.Zero))
                    throw new Win32Exception(Marshal.GetLastWin32Error());
                if (received < requested) throw new EndOfStreamException();
                Marshal.Copy(buffer, bytes, 0, requested);
                destination.Write(bytes, 0, requested);
                count -= requested;
            }
        } finally {
            VirtualFree(buffer, UIntPtr.Zero, 0x8000);
        }
    }

    public void Dispose() { handle.Dispose(); }
}
"#;

fn raw_verify_script() -> String {
    format!(
        "\nAdd-Type -TypeDefinition @'\n{RAW_DISK_CSHARP}\n'@\n{}",
        r#"$device=Assert-TokaDisk
if([long]$config.length -le 0 -or [long]$config.length -gt $device.Size){exit 4}
$raw=[TokaISORawDisk]::new([int]$device.Number)
$sha=[Security.Cryptography.SHA256]::Create()
$sink=[Security.Cryptography.CryptoStream]::new([IO.Stream]::Null,$sha,[Security.Cryptography.CryptoStreamMode]::Write)
try {
  $raw.CopyTo($sink,[long]$config.length,[int]$device.LogicalSectorSize)
  $device=Assert-TokaDisk
  $sink.FlushFinalBlock()
  $sum=[BitConverter]::ToString($sha.Hash).Replace('-','').ToLowerInvariant()
  Set-Content -LiteralPath $config.output -Encoding Ascii -Value $sum
} finally {
  $sink.Dispose()
  $sha.Dispose()
  $raw.Dispose()
}
exit 0
"#
    )
}

/// Remise en stockage : efface, GPT, une partition exFAT/NTFS « TOKAISO », identité revérifiée à chaque étape.
const STORAGE: &str = r#"
if(@('exFAT','NTFS') -cnotcontains $config.filesystem){exit 4}
$ErrorActionPreference='Stop'
$device=Assert-TokaDisk
if($device.PartitionStyle -ne 'RAW'){
  $device | Clear-Disk -RemoveData -RemoveOEM -Confirm:$false
}
$device=Assert-TokaDisk -AfterWrite
$device | Initialize-Disk -PartitionStyle GPT -Confirm:$false
$device=Assert-TokaDisk -AfterWrite
$partition=$device | New-Partition -UseMaximumSize -AssignDriveLetter
if($partition.DiskNumber -ne $config.number){exit 3}
$device=Assert-TokaDisk -AfterWrite
$partition | Format-Volume -FileSystem $config.filesystem -NewFileSystemLabel 'TOKAISO' -Force -Confirm:$false | Out-Null
exit 0
"#;

/// media_paths : 1 à 24 fichiers locaux d'un format pris en charge, sans doublon ni redirection.
pub fn media_paths(paths: &Value, suffixes: &[&str]) -> Res<Vec<PathBuf>> {
    let list = paths.as_array().filter(|l| (1..=24).contains(&l.len())).ok_or("Choisis de 1 à 24 fichiers.")?;
    let mut result: Vec<PathBuf> = vec![];
    for choice in list {
        let path = file_input(choice.as_str().ok_or("Choisis un fichier local.")?)?;
        let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy().to_lowercase())).unwrap_or_default();
        if !suffixes.contains(&ext.as_str()) || is_reparse(&path) {
            return Err("Le fichier choisi n’est pas un média pris en charge.".into());
        }
        if !result.contains(&path) {
            result.push(path);
        }
    }
    Ok(result)
}

pub fn file_stamp(path: &Path) -> Res<Value> {
    let (size, mtime) = stamp(path)?;
    Ok(json!({"path": path.to_string_lossy(), "size": size, "mtime": u64::try_from(mtime).unwrap_or(u64::MAX)}))
}

fn device_details(d: &UsbDisk) -> Value {
    json!({"id": d.id, "name": d.name, "number": d.number, "drive": d.drive, "size": d.size, "free": d.free,
           "format": d.format, "ventoy": d.ventoy, "partitionStyle": d.partition_style,
           "hardwareSerial": d.hardware_serial, "logicalSector": d.logical_sector, "volumes": d.volumes})
}

pub struct Plan {
    pub device: UsbDisk,
    pub token: String,
    pub confirm: String,
    pub operation: String,
    pub paths: Vec<PathBuf>,
    pub files: Vec<Value>,
    pub erases: bool,
    pub options: Value,
}

fn total_size(paths: &[PathBuf]) -> u64 {
    paths.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum()
}

/// UsbMedia.preview : « ce qui va se passer », avec un jeton lié à l'identité et aux fichiers.
pub fn preview(operation: &str, payload: &Value) -> Res<Plan> {
    if !OPERATIONS.contains(&operation) {
        return Err("Opération USB inconnue.".into());
    }
    let device = disks::pick_usb(payload["id"].as_str().unwrap_or(""))?;
    if operation == "install" && device.ventoy {
        return Err("Cette clé contient déjà Ventoy. Utilise la mise à jour ou ajoute des images.".into());
    }
    if matches!(operation, "copy" | "update" | "verify-ventoy") && !device.ventoy {
        return Err("Choisis une clé Ventoy détectée par Windows.".into());
    }
    if matches!(operation, "copy" | "verify-ventoy") && !disks::is_drive_letter(&device.drive) {
        return Err("Choisis la partition de données d’une clé Ventoy.".into());
    }
    let requested = &payload["paths"];
    let has_paths = requested.as_array().is_some_and(|l| !l.is_empty());
    let mut paths = vec![];
    let mut options = json!({});
    match operation {
        "install" => {
            if has_paths {
                paths = media_paths(requested, MEDIA_SUFFIXES)?;
            }
            disks::outside_target(&paths, &device)?;
            if total_size(&paths) + 64 * super::MIB > device.size {
                return Err("Les images sélectionnées dépassent la capacité de la clé.".into());
            }
            options["gpt"] = json!(payload["gpt"].as_bool() == Some(true));
        }
        "copy" => {
            paths = media_paths(requested, MEDIA_SUFFIXES)?;
            disks::outside_target(&paths, &device)?;
            if total_size(&paths) > device.free {
                return Err("Espace libre insuffisant sur la clé USB.".into());
            }
        }
        "verify-ventoy" if has_paths => {
            paths = media_paths(requested, MEDIA_SUFFIXES)?;
            disks::outside_target(&paths, &device)?;
        }
        "verify-raw" => {
            paths = media_paths(requested, RAW_SUFFIXES)?;
            if paths.len() != 1 || !has_suffix(&file_name(&paths[0]), RAW_IMAGES) {
                return Err("Choisis une seule image disque brute ou son archive.".into());
            }
            if has_suffix(&file_name(&paths[0]), COMPRESSED) {
                return Err("Les images .gz, .xz et .bz2 ne sont pas encore relues par NiTriTe : décompresse l’image d’abord.".into());
            }
            disks::outside_target(&paths, &device)?;
            let mode = payload["mode"].as_str().unwrap_or("");
            if !matches!(mode, "etcher" | "rufus-dd") {
                return Err("Choisis le mode d’écriture brute utilisé.".into());
            }
            options["mode"] = json!(mode);
        }
        "storage" => {
            let fs = payload["filesystem"].as_str().unwrap_or("");
            if !matches!(fs, "exFAT" | "NTFS") {
                return Err("Choisis exFAT ou NTFS pour remettre la clé en stockage.".into());
            }
            options["filesystem"] = json!(fs);
        }
        _ => {}
    }
    let mut names: Vec<String> = paths.iter().map(|p| file_name(p).to_lowercase()).collect();
    names.sort();
    names.dedup();
    if names.len() != paths.len() {
        return Err("Deux images sélectionnées portent le même nom.".into());
    }
    let files = paths.iter().map(|p| file_stamp(p)).collect::<Res<Vec<_>>>()?;
    let token_data = json!([operation, device.identity(), files, options]);
    Ok(Plan {
        token: sha256_bytes(token_data.to_string().as_bytes()),
        confirm: device.confirmation(),
        operation: operation.into(),
        erases: matches!(operation, "install" | "storage"),
        device,
        paths,
        files,
        options,
    })
}

/// Résumé de l'aperçu renvoyé à l'interface.
pub fn preview_summary(payload: &Value) -> Res<Value> {
    let plan = preview(payload["operation"].as_str().unwrap_or(""), payload)?;
    let paths: Vec<Value> = plan
        .paths
        .iter()
        .zip(&plan.files)
        .map(|(p, row)| json!({"name": file_name(p), "size": row["size"]}))
        .collect();
    Ok(json!({"token": plan.token, "confirm": plan.confirm, "operation": plan.operation, "erases": plan.erases,
              "options": plan.options, "device": device_details(&plan.device), "paths": paths}))
}

/// UsbMedia.action : le jeton et la confirmation tapée doivent correspondre à un aperçu frais.
pub fn run(payload: &Value) -> Res<Value> {
    let operation = payload["operation"].as_str().unwrap_or("");
    let _guard = super::MediaGuard::acquire()?;
    let plan = preview(operation, payload)?;
    if payload["token"].as_str() != Some(plan.token.as_str()) {
        return Err("La clé ou les fichiers ont changé. Recommence la comparaison.".into());
    }
    if matches!(operation, "install" | "update" | "storage") && payload["confirm"].as_str() != Some(plan.confirm.as_str()) {
        return Err("Confirme la lettre ou le numéro exact de la clé USB.".into());
    }
    execute(&plan)
}

fn prepared_volume(device: &UsbDisk, mut found: Vec<UsbDisk>) -> Res<UsbDisk> {
    for attempt in 0..6 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            found = disks::usb_disks();
        }
        if let Some(row) = found
            .iter()
            .find(|r| disks::same_prepared_device(r, device) && r.ventoy && disks::is_drive_letter(&r.drive))
        {
            return Ok(row.clone());
        }
    }
    Err("Partition de données non disponible.".into())
}

fn execute(plan: &Plan) -> Res<Value> {
    let device = &plan.device;
    match plan.operation.as_str() {
        "install" | "update" => {
            let confirm_images = || -> Res<()> {
                let same = plan.paths.iter().zip(&plan.files).all(|(p, s)| file_stamp(p).ok().as_ref() == Some(s));
                if same { Ok(()) } else { Err("Les images sélectionnées ont changé. Vérifie de nouveau le plan.".into()) }
            };
            let update = plan.operation == "update";
            let gpt = plan.options["gpt"].as_bool() == Some(true);
            let result = ventoy::initialize_usb(&device.id, &plan.confirm, gpt, update, &confirm_images)?;
            if update {
                return Ok(result);
            }
            let found = disks::usb_disks();
            if plan.paths.is_empty() {
                let prepared = found.iter().find(|r| disks::same_prepared_device(r, device) && r.ventoy);
                let mut out = result;
                out["files"] = json!([]);
                out["prepared_id"] = json!(prepared.map(|p| p.id.clone()).unwrap_or_default());
                return Ok(out);
            }
            let copied = prepared_volume(device, found).and_then(|prepared| {
                confirm_images()?;
                let mut copy = ventoy::copy_to_usb(&prepared.id, &plan.paths)?;
                copy["prepared_id"] = json!(prepared.id);
                Ok(copy)
            });
            match copied {
                Ok(mut copy) => {
                    copy["disks"] = result["disks"].clone();
                    copy["prepared"] = json!(true);
                    copy["updated"] = json!(false);
                    Ok(copy)
                }
                Err(_) => Err("Ventoy est installé. Sélectionne la clé pour reprendre la copie des images.".into()),
            }
        }
        "storage" => {
            disks::unchanged(device)?;
            let mut config = device.identity();
            config["filesystem"] = plan.options["filesystem"].clone();
            if disks::elevated_usb(&config, STORAGE, 290)? != 0 {
                return Err("Windows n’a pas confirmé la remise en stockage de la clé.".into());
            }
            Ok(json!({"disks": disks::usb_disks(), "restored": true}))
        }
        "copy" => {
            disks::unchanged(device)?;
            ventoy::copy_to_usb(&device.id, &plan.paths)
        }
        "verify-ventoy" => {
            let target = device.ventoy_root()?.join("TokaISO");
            verify_ventoy(&target, &plan.paths, &|| disks::unchanged(device))
        }
        _ => verify_raw(plan),
    }
}

fn verify_raw(plan: &Plan) -> Res<Value> {
    let device = &plan.device;
    let (length, expected) = image_hash(&plan.paths[0], device.size)?;
    disks::unchanged(device)?;
    let temp = TempDir::new("usb-verification-")?;
    let output = temp.0.join("sha256.txt");
    let mut config = device.identity();
    config["output"] = json!(output.to_string_lossy());
    config["length"] = json!(length);
    let code = disks::elevated_usb(&config, &raw_verify_script(), USB_TIMEOUT)?;
    let actual = std::fs::read_to_string(&output).unwrap_or_default().trim().to_lowercase();
    if code != 0 || actual.len() != 64 || !actual.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Windows n’a pas pu relire les données brutes de la clé.".into());
    }
    Ok(json!({"size": length, "expected": expected, "actual": actual, "match": actual == expected}))
}

fn hash_stream(stream: &mut dyn Read, limit: u64) -> Res<(u64, String)> {
    let mut hasher = Sha256::new();
    let mut length = 0u64;
    let mut buf = vec![0u8; COPY_CHUNK];
    loop {
        let n = stream.read(&mut buf).map_err(|_| "Impossible de lire l’image brute ou son archive.".to_string())?;
        if n == 0 {
            break;
        }
        length += n as u64;
        if length > limit {
            return Err("L’image décompressée dépasse la capacité de la clé.".into());
        }
        hasher.update(&buf[..n]);
    }
    Ok((length, hex(&hasher.finalize())))
}

/// image_hash : longueur et SHA-256 des octets que l'outil d'écriture brute a copiés.
pub fn image_hash(path: &Path, limit: u64) -> Res<(u64, String)> {
    let before = stamp(path)?;
    let unreadable = || "Impossible de lire l’image brute ou son archive.".to_string();
    let file = std::fs::File::open(path).map_err(|_| unreadable())?;
    let (length, digest) = if file_name(path).to_lowercase().ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(file).map_err(|_| unreadable())?;
        let mut contents = 0usize;
        let mut images = vec![];
        for i in 0..zip.len() {
            let entry = zip.by_index(i).map_err(|_| unreadable())?;
            if entry.is_dir() {
                continue;
            }
            contents += 1;
            if entry.name().is_ok_and(|n| has_suffix(&n, &[".iso", ".img", ".bin"])) {
                images.push(i);
            }
        }
        if images.len() != 1 || contents > 500 {
            return Err("L’archive doit contenir une seule image disque pour la vérification brute.".into());
        }
        let mut entry = zip.by_index(images[0]).map_err(|_| unreadable())?;
        hash_stream(&mut entry, limit)?
    } else {
        let mut file = file;
        hash_stream(&mut file, limit)?
    };
    if stamp(path)? != before {
        return Err("Le fichier a changé pendant la vérification.".into());
    }
    if length == 0 {
        return Err("L’image brute est vide.".into());
    }
    Ok((length, digest))
}

fn valid_report_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.len() == "tokaiso-sha256-".len() + 8 + 4
        && lower.starts_with("tokaiso-sha256-")
        && lower.ends_with(".txt")
        && lower["tokaiso-sha256-".len()..lower.len() - 4].bytes().all(|b| b.is_ascii_hexdigit())
}

fn valid_media_name(name: &str) -> bool {
    let reserved = ["con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "lpt1", "lpt2", "lpt3"];
    let stem = name.split('.').next().unwrap_or("").to_lowercase();
    !name.is_empty()
        && !name.chars().any(|c| (c as u32) < 32 || "<>:\"/\\|?*".contains(c))
        && !reserved.contains(&stem.as_str())
        && has_suffix(name, MEDIA_SUFFIXES)
}

/// Lit un rapport `TokaISO-SHA256-*.txt` (« <sha256>  <nom> » par ligne).
pub fn parse_manifest(text: &str, expected: &mut Vec<(String, String)>) {
    for line in text.lines() {
        let Some((digest, name)) = line.split_once("  ") else { continue };
        if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) || !valid_media_name(name) {
            continue;
        }
        if expected.iter().any(|(n, _)| n.to_lowercase() == name.to_lowercase()) {
            continue;
        }
        expected.push((name.to_string(), digest.to_lowercase()));
    }
}

/// manifest_hashes : empreintes des 100 rapports de copie les plus récents (le plus récent prime).
fn manifest_hashes(target: &Path) -> Vec<(String, String)> {
    let mut reports: Vec<(u128, PathBuf)> = std::fs::read_dir(target)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| valid_report_name(&file_name(p)) && !is_reparse(p) && p.is_file())
        .filter_map(|p| stamp(&p).ok().filter(|(size, _)| *size <= 65536).map(|(_, m)| (m, p)))
        .collect();
    reports.sort_by(|a, b| b.0.cmp(&a.0));
    let mut expected = vec![];
    for (_, report) in reports.into_iter().take(100) {
        if let Ok(raw) = std::fs::read(&report) {
            parse_manifest(&String::from_utf8_lossy(&raw), &mut expected);
        }
    }
    expected
}

fn incompatible(target: &Path) -> bool {
    target.exists() && (!target.is_dir() || is_reparse(target))
}

/// verify_ventoy : compare `X:\TokaISO\` aux images choisies ou aux rapports de copie.
pub fn verify_ventoy(target: &Path, paths: &[PathBuf], inspect: &dyn Fn() -> Res<()>) -> Res<Value> {
    if incompatible(target) {
        return Err("Le dossier TokaISO sur la clé est incompatible.".into());
    }
    let candidates: Vec<(String, String)> = if paths.is_empty() {
        manifest_hashes(target)
    } else {
        paths.iter().map(|p| Ok((file_name(p), stable_file_hash(p)?))).collect::<Res<_>>()?
    };
    if candidates.is_empty() {
        return Err("Sélectionne les images d’origine ou copie d’abord des images avec NiTriTe.".into());
    }
    let mut files = vec![];
    for (name, expected) in candidates {
        inspect()?;
        let path = target.join(&name);
        if incompatible(target) || (path.exists() && is_reparse(&path)) {
            return Err("Le dossier TokaISO sur la clé est incompatible.".into());
        }
        let actual = if path.is_file() { Some(stable_file_hash(&path)?) } else { None };
        let status = match &actual {
            None => "missing",
            Some(a) if *a == expected => "match",
            Some(_) => "different",
        };
        files.push(json!({"name": name, "expected": expected, "actual": actual, "status": status}));
    }
    inspect()?;
    let all = files.iter().all(|r| r["status"] == "match");
    Ok(json!({"files": files, "match": all}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parsing_keeps_first_and_rejects_bad_names() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let text = format!("{a}  ubuntu.iso\n{b}  UBUNTU.iso\n{a}  ..\\evil.iso\n{a}  con.iso\n{a}  notes.txt\nbad line\n");
        let mut out = vec![];
        parse_manifest(&text, &mut out);
        assert_eq!(out, vec![("ubuntu.iso".to_string(), a)]);
    }

    #[test]
    fn report_names_are_strict() {
        assert!(valid_report_name("TokaISO-SHA256-0a1b2c3d.txt"));
        assert!(!valid_report_name("TokaISO-SHA256-zzzzzzzz.txt"));
        assert!(!valid_report_name("TokaISO-SHA256-0a1b2c3d4.txt"));
    }

    #[test]
    fn media_paths_validates_count_and_type() {
        assert!(media_paths(&json!([]), MEDIA_SUFFIXES).is_err());
        assert!(media_paths(&json!(["C:\\absent.iso"]), MEDIA_SUFFIXES).is_err());
        let dir = std::env::temp_dir().join(format!("nitrite-usb-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).expect("dossier");
        let iso = dir.join("a.iso");
        let exe = dir.join("a.exe");
        std::fs::write(&iso, b"x").expect("iso");
        std::fs::write(&exe, b"x").expect("exe");
        let s = iso.to_string_lossy().to_string();
        assert_eq!(media_paths(&json!([s, s]), MEDIA_SUFFIXES).expect("liste").len(), 1);
        assert!(media_paths(&json!([exe.to_string_lossy()]), MEDIA_SUFFIXES).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn image_hash_reads_plain_and_zip_images() {
        let dir = std::env::temp_dir().join(format!("nitrite-usb-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).expect("dossier");
        let img = dir.join("disk.img");
        std::fs::write(&img, b"abc").expect("img");
        assert_eq!(image_hash(&img, 10).expect("hash"), (3, sha256_bytes(b"abc")));
        assert!(image_hash(&img, 2).is_err());
        let archive = dir.join("disk.zip");
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).expect("zip"));
            zip.start_file("disk.img", zip::write::SimpleFileOptions::default()).expect("entrée");
            zip.write_all(b"abc").expect("écriture");
            zip.finish().expect("fin");
        }
        assert_eq!(image_hash(&archive, 10).expect("hash zip").1, sha256_bytes(b"abc"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_ventoy_reports_statuses() {
        let dir = std::env::temp_dir().join(format!("nitrite-usb-{}", uuid::Uuid::new_v4().simple()));
        let target = dir.join("TokaISO");
        std::fs::create_dir_all(&target).expect("dossier");
        std::fs::write(target.join("a.iso"), b"a").expect("a");
        std::fs::write(target.join("b.iso"), b"x").expect("b");
        let report = format!("{}  a.iso\n{}  b.iso\n{}  c.iso\n", sha256_bytes(b"a"), sha256_bytes(b"b"), sha256_bytes(b"c"));
        std::fs::write(target.join("TokaISO-SHA256-01234567.txt"), report).expect("rapport");
        let result = verify_ventoy(&target, &[], &|| Ok(())).expect("vérification");
        let statuses: Vec<&str> = result["files"].as_array().expect("liste").iter().filter_map(|r| r["status"].as_str()).collect();
        assert_eq!(statuses, vec!["match", "different", "missing"]);
        assert_eq!(result["match"], false);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
