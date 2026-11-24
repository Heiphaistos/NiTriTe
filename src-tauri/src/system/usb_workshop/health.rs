//! Diagnostic en lecture seule de la clé sélectionnée (SMART Windows, smartctl s'il est installé).
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : usb_health.py.

use super::disks::{self, UsbDisk};
use super::media::RAW_DISK_CSHARP;
use super::{is_reparse, Res};
use crate::maintenance::commands::execute_system_command;
use serde_json::{json, Map, Value};
use std::path::PathBuf;

const READ_LIMIT: u64 = 8 * 1024 * 1024;
const COUNTERS: &[(&str, i64, i64)] = &[
    ("temperature", 1, 150),
    ("wear", 0, 100),
    ("hours", 0, 10_000_000),
    ("readErrors", 0, (1i64 << 32) - 2),
    ("writeErrors", 0, (1i64 << 32) - 2),
];

const WINDOWS_PROBE: &str = r#"
$WarningPreference='SilentlyContinue'
$data=@{windowsHealth=[string]$device.HealthStatus;counters=@{}}
try {
  $results=@(Get-StorageReliabilityCounter -Disk $device -ErrorAction Stop)
  if($results.Count -eq 1){
    $fields=@{temperature='Temperature';wear='Wear';hours='PowerOnHours';
              readErrors='ReadErrorsUncorrected';writeErrors='WriteErrorsUncorrected'}
    foreach($key in $fields.Keys){
      $value=$results[0].PSObject.Properties[$fields[$key]]
      if($null -ne $value -and $null -ne $value.Value){$data.counters[$key]=$value.Value}
    }
  }
} catch { $data.countersError=$true }
try {
  $drives=@(Get-CimInstance -ClassName Win32_DiskDrive -Filter ('Index = '+[int]$device.Number) -ErrorAction Stop)
  if($drives.Count -eq 1 -and $drives[0].PNPDeviceID){
    $pnp='^'+[regex]::Escape([string]$drives[0].PNPDeviceID)+'(?:_[0-9]+)?$'
    $predictions=@(Get-CimInstance -Namespace 'root/wmi' -ClassName MSStorageDriver_FailurePredictStatus -ErrorAction Stop |
      Where-Object {$_.Active -eq $true -and $_.InstanceName -match $pnp})
    if($predictions.Count -eq 1 -and $predictions[0].PredictFailure -is [bool]){
      $data.predictFailure=$predictions[0].PredictFailure
    }
  }
} catch { $data.predictError=$true }
$device=Assert-TokaDisk
ConvertTo-Json -InputObject $data -Depth 4 -Compress
"#;

fn read_probe() -> String {
    format!(
        "\nAdd-Type -TypeDefinition @'\n{RAW_DISK_CSHARP}\n'@\n{}",
        r#"$device=Assert-TokaDisk
$length=[long]$config.length
if($length -le 0 -or $length -gt $device.Size -or $length -gt 8MB){exit 4}
try {
  $reader=[TokaISORawDisk]::new([int]$device.Number)
} catch {exit 6}
try {
  $reader.CopyTo([IO.Stream]::Null,$length,[int]$device.LogicalSectorSize)
} catch {exit 7}
finally {$reader.Dispose()}
$device=Assert-TokaDisk
exit 0
"#
    )
}

/// Requête Windows non élevée (Get-Disk est lisible sans droits administrateur).
fn query_windows(device: &UsbDisk) -> Value {
    let script = format!("{}{}", disks::elevated_script(&device.identity(), ""), WINDOWS_PROBE);
    match disks::powershell_json(&script, 22) {
        Ok(v) if v.is_object() => v,
        Ok(_) => json!({}),
        Err(e) => {
            tracing::info!("usb_workshop: santé Windows indisponible : {e}");
            json!({})
        }
    }
}

/// smartctl à son emplacement d'installation standard (jamais via PATH).
fn installed_smartctl() -> Option<PathBuf> {
    ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|root| PathBuf::from(root).join("smartmontools"))
        .map(|product| (product.join(r"bin\smartctl.exe"), product))
        .find(|(binary, product)| {
            binary.is_absolute()
                && binary.is_file()
                && ![product.clone(), product.join("bin"), binary.clone()].iter().any(|p| is_reparse(p))
        })
        .map(|(binary, _)| binary)
}

fn serial_key(value: &str) -> String {
    value.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase()
}

/// Données smartctl acceptées seulement si série et capacité correspondent au disque choisi.
pub fn accept_smartctl(device: &UsbDisk, mut data: Value, exit_status: i64) -> Value {
    let serial = serial_key(&device.hardware_serial);
    if !data.is_object() || serial_key(data["serial_number"].as_str().unwrap_or("")) != serial {
        return json!({});
    }
    if data["user_capacity"]["bytes"].as_u64() != Some(device.size) || exit_status & 3 != 0 {
        return json!({});
    }
    data["exitStatus"] = json!(exit_status);
    data
}

fn query_smartctl(device: &UsbDisk) -> Value {
    if serial_key(&device.hardware_serial).is_empty() || device.number > 255 {
        return json!({});
    }
    let Some(binary) = installed_smartctl() else { return json!({}) };
    let target = format!("/dev/pd{}", device.number);
    match execute_system_command(&binary.to_string_lossy(), &["-j", "-i", "-H", "-A", &target], 12) {
        Ok(out) => match serde_json::from_str::<Value>(&out.stdout) {
            Ok(data) => accept_smartctl(device, data, i64::from(out.exit_code)),
            Err(_) => json!({}),
        },
        Err(e) => {
            tracing::info!("usb_workshop: smartctl indisponible : {e}");
            json!({})
        }
    }
}

fn collect_counters(values: &Value) -> Map<String, Value> {
    COUNTERS
        .iter()
        .filter_map(|(key, min, max)| {
            let v = values.get(*key)?.as_i64()?;
            (*min <= v && v <= *max).then(|| (key.to_string(), json!(v)))
        })
        .collect()
}

/// decode_status : synthèse SMART (Windows d'abord, smartctl en appui) et alertes.
pub fn decode_status(windows: &Value, smart: &Value) -> Value {
    let health = windows["windowsHealth"].as_str().unwrap_or("").trim().to_lowercase();
    let windows_health = match health.as_str() {
        "healthy" | "0" => "normal",
        "warning" | "1" => "warning",
        "unhealthy" | "2" => "failed",
        _ => "unavailable",
    };
    let (mut source, mut status) = ("", "unavailable");
    if let Some(fail) = windows["predictFailure"].as_bool() {
        source = "Windows";
        status = if fail { "failed" } else { "passed" };
    }
    if let Some(passed) = smart["smart_status"]["passed"].as_bool() {
        if status == "unavailable" || !passed {
            source = "smartctl";
            status = if passed { "passed" } else { "failed" };
        }
    }
    let extra = collect_counters(&json!({
        "temperature": smart["temperature"]["current"],
        "hours": smart["power_on_time"]["hours"],
        "wear": smart["nvme_smart_health_information_log"]["percentage_used"],
    }));
    let mut counters = extra;
    counters.extend(collect_counters(&windows["counters"]));
    let count = |k: &str| counters.get(k).and_then(Value::as_i64).unwrap_or(0);
    let alerts = matches!(windows_health, "warning" | "failed")
        || count("wear") >= 100
        || smart["exitStatus"].as_i64().unwrap_or(0) & 0xF0 != 0
        || count("readErrors") > 0
        || count("writeErrors") > 0;
    json!({"smart": status, "source": source, "windowsHealth": windows_health, "counters": counters, "alerts": alerts})
}

fn smart(device: &UsbDisk) -> Value {
    let windows = query_windows(device);
    let smart = if windows["predictFailure"].is_boolean() { json!({}) } else { query_smartctl(device) };
    decode_status(&windows, &smart)
}

/// Taille de l'échantillon lu : 8 Mio au plus, multiple du secteur logique.
pub fn sample_length(sector: u32, size: u64) -> Option<u64> {
    if !(512..=65536).contains(&sector) || !sector.is_power_of_two() {
        return None;
    }
    let length = size.min(READ_LIMIT) / u64::from(sector) * u64::from(sector);
    (length > 0).then_some(length)
}

fn read_sample(device: &UsbDisk) -> Value {
    let Some(length) = sample_length(device.logical_sector, device.size) else {
        return json!({"read": "unavailable", "bytes": 0});
    };
    let mut config = device.identity();
    config["length"] = json!(length);
    let read = match disks::elevated_usb(&config, &read_probe(), 110) {
        Ok(0) => "passed",
        Ok(7) => "failed",
        Ok(_) => "unavailable",
        Err(e) if e.contains("La clé a changé") || e.contains("annulée") => return json!({"error": e}),
        Err(_) => "unavailable",
    };
    json!({"read": read, "bytes": length})
}

/// UsbHealth.action : `kind` = "smart" ou "read" ; l'identité est revérifiée après coup.
pub fn run(id: &str, kind: &str) -> Res<Value> {
    if !matches!(kind, "smart" | "read") {
        return Err("Opération de diagnostic USB inconnue.".into());
    }
    let _guard = super::MediaGuard::acquire()?;
    let device = disks::pick_usb(id)?;
    let result = if kind == "smart" { smart(&device) } else { read_sample(&device) };
    if let Some(e) = result["error"].as_str() {
        return Err(e.to_string());
    }
    disks::unchanged(&device)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_prefers_windows_and_flags_errors() {
        let windows = json!({"windowsHealth": "Healthy", "predictFailure": false,
                             "counters": {"temperature": 40, "readErrors": 2, "wear": 500}});
        let out = decode_status(&windows, &json!({}));
        assert_eq!(out["smart"], "passed");
        assert_eq!(out["source"], "Windows");
        assert_eq!(out["windowsHealth"], "normal");
        assert_eq!(out["counters"]["temperature"], 40);
        assert!(out["counters"].get("wear").is_none(), "valeur hors bornes ignorée");
        assert_eq!(out["alerts"], true);
    }

    #[test]
    fn smartctl_failure_overrides_pass() {
        let smart = json!({"smart_status": {"passed": false}, "temperature": {"current": 35}});
        let out = decode_status(&json!({"predictFailure": false}), &smart);
        assert_eq!(out["smart"], "failed");
        assert_eq!(out["source"], "smartctl");
        assert_eq!(out["counters"]["temperature"], 35);
    }

    #[test]
    fn unknown_health_is_unavailable() {
        let out = decode_status(&json!({}), &json!({}));
        assert_eq!(out["smart"], "unavailable");
        assert_eq!(out["alerts"], false);
    }

    #[test]
    fn sample_is_sector_aligned() {
        assert_eq!(sample_length(512, 100 * 1024 * 1024), Some(8 * 1024 * 1024));
        assert_eq!(sample_length(4096, 10_000), Some(8192));
        assert_eq!(sample_length(500, 10_000), None);
        assert_eq!(sample_length(512, 100), None);
    }

    #[test]
    fn smartctl_requires_matching_serial_and_size() {
        let disk = disks::parse_report(&json!({"number": 1, "name": "X", "serial": "S", "size": 1000, "hardwareSerial": "AB 12"}))
            .remove(0);
        let ok = json!({"serial_number": "ab12", "user_capacity": {"bytes": 1000}});
        assert_eq!(accept_smartctl(&disk, ok.clone(), 0)["exitStatus"], 0);
        assert_eq!(accept_smartctl(&disk, ok, 2), json!({}));
        let other = json!({"serial_number": "zz", "user_capacity": {"bytes": 1000}});
        assert_eq!(accept_smartctl(&disk, other, 0), json!({}));
    }
}
