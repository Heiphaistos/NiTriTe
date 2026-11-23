//! Disques USB amovibles, identité stricte et exécution élevée vérifiée.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : media_tools.py, winops.py.

use super::{sha256_bytes, Res};
use crate::maintenance::commands::execute_system_command;
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Liste les disques USB en ligne, ni système, ni démarrage, ni lecture seule.
const USB_REPORT: &str = r#"
$ErrorActionPreference='Stop'
$rows=@(Get-Disk | Where-Object {$_.BusType -eq 'USB' -and !$_.IsBoot -and !$_.IsSystem -and !$_.IsReadOnly -and !$_.IsOffline} | ForEach-Object {
  $disk=$_
  $parts=@(Get-Partition -DiskNumber $disk.Number -ErrorAction SilentlyContinue)
  $vol=@($parts | Get-Volume -ErrorAction SilentlyContinue)
  $ventoy=(@($vol | Where-Object {$_.FileSystemLabel -eq 'VTOYEFI' -and (($_.Size -ge 20MB) -and ($_.Size -le 64MB))}).Count -gt 0)
  $data=@($vol | Where-Object {$_.DriveLetter -and $_.FileSystemLabel -ne 'VTOYEFI'} | Sort-Object Size -Descending | Select-Object -First 1)
  $diskID=([string]$disk.UniqueId).Trim()
  $hardware=([string]$disk.SerialNumber).Trim()
  if (!$diskID) { $diskID=$hardware }
  @{number=$disk.Number;name=[string]$disk.FriendlyName;serial=$diskID.Trim();size=$disk.Size;
    hardwareSerial=$hardware;diskPath=[string]$disk.Path;partitionStyle=[string]$disk.PartitionStyle;
    logicalSector=[int]$disk.LogicalSectorSize;
    accessPaths=@($parts | ForEach-Object {$_.AccessPaths} | Sort-Object -Unique);
    volumes=@($vol | ForEach-Object {
      @{drive=if($_.DriveLetter){[string]$_.DriveLetter+':'}else{''};label=[string]$_.FileSystemLabel;
        format=[string]$_.FileSystem;size=$_.Size;free=$_.SizeRemaining}
    });
    drive=if($data.Count){[string]$data[0].DriveLetter+':'}else{''};
    format=if($data.Count){[string]$data[0].FileSystem}else{''};
    free=if($data.Count){$data[0].SizeRemaining}else{0};ventoy=$ventoy}
})
ConvertTo-Json -InputObject $rows -Depth 5 -Compress
"#;

/// Revérifie l'identité exacte du disque (numéro + série + taille + chemin…) sous le jeton élevé.
/// Code de sortie 3 = la clé a changé.
pub const USB_ELEVATED_CHECK: &str = r#"
$ErrorActionPreference='Stop'
function Assert-TokaDisk([switch]$AfterWrite) {
  $rows=@(Get-Disk -Number ([int]$config.number) -ErrorAction SilentlyContinue)
  if($rows.Count -ne 1){exit 3}
  $disk=$rows[0]
  $diskPath=[string]$disk.Path
  $hardware=([string]$disk.SerialNumber).Trim()
  $id=([string]$disk.UniqueId).Trim()
  if(!$id){$id=$hardware}
  if($disk.BusType -ne 'USB' -or $disk.IsBoot -or $disk.IsSystem -or $disk.IsReadOnly -or $disk.IsOffline -or
     $disk.Size -ne $config.size -or ([string]$disk.FriendlyName) -cne $config.name -or
     ($config.diskPath -and $diskPath -ine $config.diskPath) -or
     ($config.hardwareSerial -and $hardware -cne $config.hardwareSerial) -or
     ($config.logicalSector -and $disk.LogicalSectorSize -ne $config.logicalSector)){exit 3}
  if($id -cne $config.serial -and (!$AfterWrite -or !$config.diskPath -or !$config.hardwareSerial)){exit 3}
  if(!$AfterWrite){
    $parts=@($disk | Get-Partition -ErrorAction SilentlyContinue)
    $vol=@($parts | Get-Volume -ErrorAction SilentlyContinue)
    $data=@($vol | Where-Object {$_.DriveLetter -and $_.FileSystemLabel -ne 'VTOYEFI'} | Sort-Object Size -Descending | Select-Object -First 1)
    $drive=if($data.Count){[string]$data[0].DriveLetter+':'}else{''}
    $ventoy=(@($vol | Where-Object {$_.FileSystemLabel -eq 'VTOYEFI' -and (($_.Size -ge 20MB) -and ($_.Size -le 64MB))}).Count -gt 0)
    if($drive -cne $config.drive -or $ventoy -ne $config.ventoy -or
       ($config.partitionStyle -and ([string]$disk.PartitionStyle) -ne $config.partitionStyle)){exit 3}
    if($null -ne $config.accessPaths){
      $accessPaths=@($parts | ForEach-Object {$_.AccessPaths} | Sort-Object -Unique)
      if(($accessPaths -join '|') -ine ($config.accessPaths -join '|')){exit 3}
    }
  }
  return $disk
}
$device=Assert-TokaDisk
"#;

/// Relance l'encodage élevé via l'UAC et renvoie son code de sortie (1223 = UAC refusée).
const ELEVATE: &str = r#"
$ErrorActionPreference='Stop'
$argsForAdmin=@('-NoLogo','-NoProfile','-NonInteractive','-EncodedCommand',$elevated)
try {
  $p=Start-Process -FilePath $shell -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ArgumentList $argsForAdmin
  @{exit=$p.ExitCode}|ConvertTo-Json -Compress
} catch {
  if($_.Exception.GetBaseException().NativeErrorCode -eq 1223){
    @{exit=1223}|ConvertTo-Json -Compress
  } else { throw }
}
"#;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Volume {
    pub drive: String,
    pub label: String,
    pub format: String,
    pub size: u64,
    pub free: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsbDisk {
    pub id: String,
    pub number: u32,
    pub name: String,
    pub serial: String,
    pub size: u64,
    pub hardware_serial: String,
    pub disk_path: String,
    pub partition_style: String,
    pub logical_sector: u32,
    pub access_paths: Vec<String>,
    pub volumes: Vec<Volume>,
    pub drive: String,
    pub format: String,
    pub free: u64,
    pub ventoy: bool,
}

impl UsbDisk {
    /// Champs d'identité comparés avant chaque écriture (usb_identity).
    pub fn identity(&self) -> Value {
        json!({
            "number": self.number, "serial": self.serial, "size": self.size, "name": self.name,
            "hardwareSerial": self.hardware_serial, "diskPath": self.disk_path,
            "logicalSector": self.logical_sector, "partitionStyle": self.partition_style,
            "drive": self.drive, "ventoy": self.ventoy, "accessPaths": self.access_paths,
        })
    }

    /// Lettre de la partition de données (« E: »), ou numéro du disque : texte à taper pour confirmer.
    pub fn confirmation(&self) -> String {
        if self.drive.is_empty() { self.number.to_string() } else { self.drive.clone() }
    }

    /// Racine de la partition de données d'une clé Ventoy (`E:\`).
    pub fn ventoy_root(&self) -> Res<PathBuf> {
        if !self.ventoy || !is_drive_letter(&self.drive) {
            return Err("Choisis la partition de données d’une clé Ventoy.".into());
        }
        Ok(PathBuf::from(format!("{}\\", self.drive)))
    }
}

pub fn is_drive_letter(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 2 && b[0].is_ascii_uppercase() && b[1] == b':'
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn num(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// Tableau JSON tolérant (PowerShell peut sérialiser un élément unique sans crochets).
fn list(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::Null) | None => vec![],
        Some(other) => vec![other.clone()],
    }
}

/// Parse le rapport USB_REPORT ; écarte les disques sans série ni taille.
pub fn parse_report(raw: &Value) -> Vec<UsbDisk> {
    list(Some(raw))
        .iter()
        .filter_map(|row| {
            let serial = text(row, "serial");
            let size = num(row, "size");
            let number = u32::try_from(row.get("number")?.as_u64()?).ok()?;
            if serial.is_empty() || size == 0 {
                return None;
            }
            let name = text(row, "name");
            let id = sha256_bytes(format!("{number}|{serial}|{name}|{size}").as_bytes());
            Some(UsbDisk {
                id,
                number,
                name,
                serial,
                size,
                hardware_serial: text(row, "hardwareSerial"),
                disk_path: text(row, "diskPath"),
                partition_style: text(row, "partitionStyle"),
                logical_sector: u32::try_from(num(row, "logicalSector")).unwrap_or(0),
                access_paths: list(row.get("accessPaths")).iter().filter_map(|p| p.as_str().map(String::from)).collect(),
                volumes: list(row.get("volumes"))
                    .iter()
                    .map(|v| Volume {
                        drive: text(v, "drive"),
                        label: text(v, "label"),
                        format: text(v, "format"),
                        size: num(v, "size"),
                        free: num(v, "free"),
                    })
                    .collect(),
                drive: text(row, "drive"),
                format: text(row, "format"),
                free: num(row, "free"),
                ventoy: row.get("ventoy").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

/// Message renvoyé quand une commande Windows dépasse son délai.
pub const TIMEOUT_MSG: &str = "Windows n’a pas confirmé la fin de l’opération USB dans le délai prévu.";

pub fn powershell_exe() -> PathBuf {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
}

pub fn system32() -> PathBuf {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("System32")
}

/// Littéral PowerShell qui reconstruit `$config` depuis du JSON encodé en base64
/// (aucune valeur utilisateur n'apparaît en clair dans le script).
pub fn config_prelude(config: &Value) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(config.to_string());
    format!("$config=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded}'))|ConvertFrom-Json;\n")
}

/// Exécute un script PowerShell (sortie UTF-8) avec timeout et renvoie son JSON.
pub fn powershell_json(script: &str, timeout_secs: u64) -> Res<Value> {
    let full = format!("[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); {script}");
    let exe = powershell_exe();
    let result = execute_system_command(
        &exe.to_string_lossy(),
        &["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", &full],
        timeout_secs,
    )
    .map_err(|e| match e {
        crate::error::NiTriTeError::Timeout(_) => TIMEOUT_MSG.to_string(),
        other => other.to_string(),
    })?;
    if !result.success {
        let detail = result.stderr.trim();
        return Err(if detail.is_empty() { "Windows n’a pas pu exécuter la commande.".into() } else { detail.chars().take(400).collect() });
    }
    let out = result.stdout.trim_start_matches('\u{feff}').trim();
    if out.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(out).map_err(|_| "Réponse de Windows illisible.".to_string())
}

/// Disques USB éligibles (liste vide si Windows ne répond pas, comme TokaISO).
pub fn usb_disks() -> Vec<UsbDisk> {
    match powershell_json(USB_REPORT, 18) {
        Ok(raw) => parse_report(&raw),
        Err(e) => {
            tracing::warn!("usb_workshop: liste des disques USB indisponible : {e}");
            vec![]
        }
    }
}

pub fn pick_usb(identity: &str) -> Res<UsbDisk> {
    usb_disks()
        .into_iter()
        .find(|d| d.id == identity)
        .ok_or_else(|| "La clé USB n’a pas pu être identifiée avec certitude.".to_string())
}

/// Refuse si l'identité du disque a changé depuis la sélection.
pub fn unchanged(initial: &UsbDisk) -> Res<()> {
    if pick_usb(&initial.id)?.identity() != initial.identity() {
        return Err("La clé a changé. Sélectionne-la de nouveau.".into());
    }
    Ok(())
}

/// Même clé après réinstallation (son identifiant Windows peut changer).
pub fn same_prepared_device(row: &UsbDisk, device: &UsbDisk) -> bool {
    if row.disk_path != device.disk_path {
        return false;
    }
    if row.id == device.id {
        return true;
    }
    !device.disk_path.is_empty()
        && !device.hardware_serial.is_empty()
        && row.hardware_serial == device.hardware_serial
        && row.number == device.number
        && row.name == device.name
        && row.size == device.size
}

/// Refuse une image stockée sur la clé à préparer.
pub fn outside_target(paths: &[PathBuf], device: &UsbDisk) -> Res<()> {
    let mut mounts: Vec<String> = device
        .access_paths
        .iter()
        .filter(|p| p.len() >= 3 && p.as_bytes()[0].is_ascii_alphabetic() && &p[1..3] == ":\\")
        .cloned()
        .collect();
    mounts.extend(device.volumes.iter().filter(|v| !v.drive.is_empty()).map(|v| format!("{}\\", v.drive)));
    if !device.drive.is_empty() {
        mounts.push(format!("{}\\", device.drive));
    }
    for path in paths {
        let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        let spelling = strip_verbatim(&resolved).replace('/', "\\").to_lowercase();
        if mounts.iter().any(|m| spelling.starts_with(&format!("{}\\", m.trim_end_matches('\\').to_lowercase()))) {
            return Err("Déplace d’abord l’image vers un autre support que la clé à préparer.".into());
        }
    }
    Ok(())
}

fn strip_verbatim(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.strip_prefix(r"\\?\").map(String::from).unwrap_or_else(|| s.into_owned())
}

/// Script élevé complet : `$config`, contrôle d'identité, puis le corps.
pub fn elevated_script(config: &Value, body: &str) -> String {
    format!("{}{}\n{}", config_prelude(config), USB_ELEVATED_CHECK, body)
}

/// elevated_usb : exécute `body` sous UAC après contrôle d'identité ; renvoie le code de sortie.
/// Erreurs : clé changée (3), UAC annulée (1223), délai dépassé.
pub fn elevated_usb(config: &Value, body: &str, timeout_secs: u64) -> Res<i64> {
    let script = elevated_script(config, body);
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    let shell = powershell_exe();
    // Le chemin de powershell.exe et l'encodage base64 ne contiennent ni ' ni saisie utilisateur.
    let outer = format!(
        "$shell='{}'; $elevated='{}';\n{}",
        shell.to_string_lossy().replace('\'', "''"),
        encoded,
        ELEVATE
    );
    let result = powershell_json(&outer, timeout_secs)?;
    match result.get("exit").and_then(Value::as_i64) {
        Some(3) => Err("La clé a changé. Sélectionne-la de nouveau.".into()),
        Some(1223) => Err("La demande d’autorisation Windows a été annulée.".into()),
        Some(code) => Ok(code),
        None => Err("Windows n’a pas confirmé la fin de l’opération USB.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        json!([{
            "number": 3, "name": "SanDisk Ultra", "serial": "USBSTOR\\ABC", "size": 32_000_000_000u64,
            "hardwareSerial": "4C530001", "diskPath": "\\\\?\\usbstor#disk", "partitionStyle": "MBR",
            "logicalSector": 512, "accessPaths": "E:\\",
            "volumes": {"drive": "E:", "label": "Ventoy", "format": "exFAT", "size": 31_000_000_000u64, "free": 10},
            "drive": "E:", "format": "exFAT", "free": 10, "ventoy": true
        }, {"number": 4, "name": "Vide", "serial": "", "size": 0}])
    }

    #[test]
    fn report_parsing_keeps_valid_disks_and_single_values() {
        let disks = parse_report(&sample());
        assert_eq!(disks.len(), 1);
        let d = &disks[0];
        assert_eq!(d.access_paths, vec!["E:\\".to_string()]);
        assert_eq!(d.volumes.len(), 1);
        assert_eq!(d.confirmation(), "E:");
        assert_eq!(d.id, sha256_bytes(b"3|USBSTOR\\ABC|SanDisk Ultra|32000000000"));
        assert!(d.ventoy_root().is_ok());
    }

    #[test]
    fn single_object_report_is_accepted() {
        let raw = sample()[0].clone();
        assert_eq!(parse_report(&raw).len(), 1);
    }

    #[test]
    fn identity_changes_with_drive_letter() {
        let a = parse_report(&sample()).remove(0);
        let mut b = a.clone();
        b.drive = "F:".into();
        assert_ne!(a.identity(), b.identity());
        assert_eq!(b.confirmation(), "F:");
        b.drive.clear();
        assert_eq!(b.confirmation(), "3");
        assert!(b.ventoy_root().is_err());
    }

    #[test]
    fn prepared_device_matches_by_hardware_serial() {
        let a = parse_report(&sample()).remove(0);
        let mut b = a.clone();
        b.id = "autre".into();
        assert!(same_prepared_device(&b, &a));
        b.size += 1;
        assert!(!same_prepared_device(&b, &a));
    }

    #[test]
    fn images_on_target_are_refused() {
        let a = parse_report(&sample()).remove(0);
        assert!(outside_target(&[PathBuf::from(r"E:\iso\x.iso")], &a).is_err());
        assert!(outside_target(&[PathBuf::from(r"D:\iso\x.iso")], &a).is_ok());
    }

    #[test]
    fn prelude_hides_user_text_in_base64() {
        let prelude = config_prelude(&json!({"path": "C:\\x'; Remove-Item *"}));
        assert!(!prelude.contains("Remove-Item"));
        assert!(prelude.contains("FromBase64String"));
    }

    #[test]
    fn drive_letter_validation() {
        assert!(is_drive_letter("E:"));
        assert!(!is_drive_letter("e:"));
        assert!(!is_drive_letter("E:\\"));
    }
}
