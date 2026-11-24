//! Tests réseau (ping, DNS, port TCP, traceroute, vue d'ensemble), inventaire des disques en lecture seule
//! et contrôle de place.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : diagnostics.py (host, port, probe, network, drive_report, disk_check).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::{IpAddr, Ipv6Addr};
use std::path::PathBuf;
use std::time::Instant;

use super::{powershell_json, run, system32, R, FAT32_MAX, TIMEOUT};

const PING: &str = r#"
$ping=New-Object System.Net.NetworkInformation.Ping
$rows=for($i=0;$i -lt [int]$p.count;$i++){
  try{
    $reply=$ping.Send([string]$p.host,1000)
    @{status=$reply.Status.ToString();ms=$reply.RoundtripTime;address=[string]$reply.Address}
  }catch{
    $err=$_.Exception.GetBaseException()
    $status=if($err -is [Net.Sockets.SocketException]){$err.SocketErrorCode.ToString()}elseif(
      $err -is [ComponentModel.Win32Exception] -and $err.NativeErrorCode -eq 5){'AccessDenied'}else{'Error'}
    @{status=$status;ms=$null;address='';detail=$err.Message}
  }
}
ConvertTo-Json -InputObject @($rows) -Compress
"#;
const DNS: &str = r#"
try{
  $task=[Net.Dns]::GetHostAddressesAsync([string]$p.host)
  if(!$task.Wait(4000)){throw 'timeout'}
  $addresses=@($task.Result | ForEach-Object {$_.IPAddressToString})
  @{addresses=$addresses;ok=$true}|ConvertTo-Json -Compress
}catch{@{addresses=@();ok=$false}|ConvertTo-Json -Compress}
"#;
const TCP: &str = r#"
$check=& {
$client=[Net.Sockets.TcpClient]::new()
try{
  $task=$client.ConnectAsync([string]$p.host,[int]$p.port)
  if($task.Wait(3200) -and $client.Connected){@{status='open';address=$client.Client.RemoteEndPoint.Address.ToString()}}
  else{@{status='timeout'}}
}catch{
  $reason=$_.Exception.GetBaseException()
  $status=if($reason -is [Net.Sockets.SocketException]){$reason.SocketErrorCode.ToString()}else{'error'}
  @{status=$status}
}finally{$client.Dispose()}
}
$check | ConvertTo-Json -Compress
"#;
const GATEWAY: &str = r#"
$route=Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
  Where-Object {$_.NextHop -ne '0.0.0.0'} | Sort-Object -Property @{Expression={$_.RouteMetric+$_.InterfaceMetric}} | Select-Object -First 1
if(!$route){$route=Get-NetRoute -DestinationPrefix '::/0' -ErrorAction SilentlyContinue |
  Where-Object {$_.NextHop -ne '::'} | Sort-Object -Property RouteMetric | Select-Object -First 1}
if($route){
  $next=[string]$route.NextHop
  if($next.StartsWith('fe80:')){$next+="%"+$route.InterfaceIndex}
  ConvertTo-Json -InputObject $next -Compress
}
"#;
const DRIVE_REPORT: &str = r#"
$ErrorActionPreference='SilentlyContinue'
$drives=@(Get-CimInstance Win32_LogicalDisk | Select-Object DeviceID,VolumeName,FileSystem,Size,FreeSpace,DriveType)
$disks=@(Get-Disk | Select-Object Number,FriendlyName,@{Name='BusType';Expression={[string]$_.BusType}},Size,
  @{Name='PartitionStyle';Expression={[string]$_.PartitionStyle}},@{Name='HealthStatus';Expression={[string]$_.HealthStatus}},
  @{Name='OperationalStatus';Expression={[string]$_.OperationalStatus}})
$partitions=@(Get-Partition | Select-Object DiskNumber,PartitionNumber,@{Name='DriveLetter';Expression={if($_.DriveLetter -and [int][char]$_.DriveLetter -ne 0){[string]$_.DriveLetter}else{''}}},Size,
  @{Name='Type';Expression={[string]$_.Type}})
$health=@(Get-PhysicalDisk | ForEach-Object {
  $disk=$_
  $counter=$disk | Get-StorageReliabilityCounter -ErrorAction SilentlyContinue
  @{name=$disk.FriendlyName;bus=[string]$disk.BusType;health=[string]$disk.HealthStatus;
    temperature=$counter.Temperature;wear=$counter.Wear;hours=$counter.PowerOnHours;
    readErrors=$counter.ReadErrorsUncorrected;writeErrors=$counter.WriteErrorsUncorrected}
})
@{drives=$drives;disks=$disks;partitions=$partitions;health=$health}|ConvertTo-Json -Depth 5 -Compress
"#;

fn host_part(part: &str) -> bool {
    let b = part.as_bytes();
    !b.is_empty()
        && b.len() <= 63
        && b[0].is_ascii_alphanumeric()
        && b[b.len() - 1].is_ascii_alphanumeric()
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'-')
}

/// Adresse IP (IPv6 avec `%interface` numérique) ou nom DNS ASCII ; ni URL ni port.
pub fn host(value: &str) -> R<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || value.len() > 255 {
        return Err("Entre une adresse IP ou un nom de serveur.".into());
    }
    let mut v = trimmed.strip_suffix('.').unwrap_or(trimmed);
    if let Some(inner) = v.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
        v = inner;
    }
    if let Ok(ip) = v.parse::<IpAddr>() {
        return Ok(ip.to_string());
    }
    if let Some((addr, scope)) = v.split_once('%') {
        if let Ok(ip) = addr.parse::<Ipv6Addr>() {
            if scope.is_empty() || !scope.bytes().all(|b| b.is_ascii_digit()) {
                return Err("Entre le numéro de l’interface IPv6 après le signe %.".into());
            }
            return Ok(format!("{ip}%{scope}"));
        }
    }
    if v.is_ascii() && v.len() <= 253 && v.split('.').all(host_part) {
        return Ok(v.to_ascii_lowercase());
    }
    Err("Entre une adresse IP ou un nom de serveur, sans URL ni port.".into())
}

pub fn port(value: i64) -> R<u16> {
    if (1..=65535).contains(&value) {
        Ok(value as u16)
    } else {
        Err("Entre un port TCP entre 1 et 65535.".into())
    }
}

pub fn probe(kind: &str, target: &str, tcp_port: u16, count: u32) -> Value {
    let started = Instant::now();
    let params = json!({"host": target, "count": count, "port": tcp_port});
    let data: Result<Value, String> = match kind {
        "ping" => powershell_json(PING, params, 12).map(|responses| {
            let rows = responses.as_array().cloned().unwrap_or_default();
            let times: Vec<f64> = rows.iter().filter(|r| r["status"] == "Success").filter_map(|r| r["ms"].as_f64()).collect();
            let average = if times.is_empty() { Value::Null } else { json!(((times.iter().sum::<f64>() / times.len() as f64) * 10.0).round() / 10.0) };
            let mut data = json!({"ok": !times.is_empty(), "replies": rows, "sent": count, "received": times.len(), "average": average});
            if times.is_empty() && !rows.is_empty() && rows.iter().all(|r| r["detail"].as_str().is_some_and(|d| !d.is_empty())) {
                data["status"] = rows[0]["status"].clone();
                data["output"] = rows[0]["detail"].clone();
            }
            data
        }),
        "dns" => powershell_json(DNS, params, 8),
        "tcp" => powershell_json(TCP, params, 8).map(|mut d| {
            d["ok"] = json!(d["status"] == "open");
            d
        }),
        _ => {
            let args = ["-d", "-h", "20", "-w", "400", target].map(String::from);
            run(&system32().join("tracert.exe"), &args, 34).map(|out| {
                json!({"ok": out.success, "output": format!("{}{}", out.stdout, out.stderr).trim(), "code": out.exit_code})
            })
        }
    };
    let data = match data {
        Ok(Value::Object(map)) => Value::Object(map),
        Ok(_) => json!({"ok": false, "status": "error"}),
        Err(e) if e == TIMEOUT => json!({"ok": false, "status": "timeout", "output": ""}),
        Err(e) => json!({"ok": false, "status": "error", "output": e}),
    };
    let mut result = json!({
        "kind": kind, "host": target, "port": if kind == "tcp" { json!(tcp_port) } else { Value::Null },
        "elapsed": (started.elapsed().as_secs_f64() * 100.0).round() / 100.0,
    });
    if let (Some(out), Value::Object(map)) = (result.as_object_mut(), data) {
        out.extend(map);
    }
    result
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkRequest {
    pub test: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: i64,
    #[serde(default)]
    pub internet: String,
}

fn default_port() -> i64 {
    443
}

pub fn network(req: &NetworkRequest) -> R<Value> {
    if !["ping", "dns", "tcp", "traceroute", "overview"].contains(&req.test.as_str()) {
        return Err("Choisis un test réseau.".into());
    }
    let target = host(&req.host)?;
    let tcp = if req.test == "tcp" || req.test == "overview" { port(req.port)? } else { 443 };
    if req.test != "overview" {
        return Ok(json!({"checks": [probe(&req.test, &target, tcp, 4)]}));
    }
    let internet = host(if req.internet.trim().is_empty() { "1.1.1.1" } else { &req.internet })?;
    let gateway = powershell_json(GATEWAY, json!({}), 7).ok().and_then(|v| v.as_str().map(String::from));
    let mut tests: Vec<(&str, &str, String, u16)> = vec![
        ("server", "ping", target.clone(), tcp),
        ("server", "tcp", target.clone(), tcp),
        ("server", "dns", target, tcp),
        ("internet", "tcp", internet, 443),
    ];
    if let Some(gw) = gateway.as_deref().and_then(|g| host(g).ok()) {
        tests.insert(0, ("gateway", "ping", gw, 443));
    }
    let checks: Vec<Value> = std::thread::scope(|scope| {
        let handles: Vec<_> = tests
            .iter()
            .map(|(label, kind, address, value)| {
                let label = label.to_string();
                scope.spawn(move || {
                    let mut row = probe(kind, address, *value, 3);
                    row["label"] = json!(label);
                    row
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| json!({"ok": false, "status": "error"}))).collect()
    });
    Ok(json!({"gateway": gateway, "checks": checks}))
}

/// Volumes, disques, partitions et état SMART (lecture seule).
pub fn drive_report() -> Value {
    let raw = powershell_json(DRIVE_REPORT, json!({}), 20).unwrap_or(Value::Null);
    let mut result = json!({});
    for key in ["drives", "disks", "partitions", "health"] {
        result[key] = match &raw[key] {
            Value::Array(a) => Value::Array(a.clone()),
            Value::Object(o) => json!([o]),
            _ => json!([]),
        };
    }
    if result["drives"].as_array().is_some_and(|a| a.is_empty()) {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        result["drives"] = disks
            .list()
            .iter()
            .map(|d| {
                let id: String = d.mount_point().to_string_lossy().chars().take(2).collect();
                json!({"DeviceID": id, "VolumeName": "", "FileSystem": d.file_system().to_string_lossy(),
                       "Size": d.total_space(), "FreeSpace": d.available_space(), "DriveType": if d.is_removable() { 2 } else { 3 }})
            })
            .collect();
    }
    result
}

#[derive(Debug, Clone, Deserialize)]
pub struct Estimate {
    #[serde(default)]
    pub name: String,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileRow {
    pub name: String,
    pub size: u64,
}

/// Les fichiers (réels ou estimés) tiennent-ils sur le volume, et lesquels dépassent la limite FAT32 ?
pub fn disk_check(drive: &str, paths: &[String], sizes: &[Estimate]) -> R<Value> {
    let report = drive_report();
    let entry = report["drives"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["DeviceID"] == drive).cloned())
        .ok_or("Choisis un volume accessible.")?;
    if paths.len() + sizes.len() > 24 {
        return Err("Choisis au plus 24 fichiers pour ce contrôle.".into());
    }
    let mut rows = Vec::new();
    for value in paths {
        let path = PathBuf::from(value);
        if !path.is_absolute() || !path.is_file() {
            return Err("Le fichier choisi est introuvable.".into());
        }
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        rows.push(FileRow { name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), size });
    }
    for e in sizes {
        if !(0..=(1i64 << 54)).contains(&e.size) {
            return Err("Taille de fichier invalide.".into());
        }
        rows.push(FileRow { name: e.name.chars().take(200).collect(), size: e.size as u64 });
    }
    let total: u64 = rows.iter().map(|r| r.size).sum();
    let free = entry["FreeSpace"].as_u64().unwrap_or(0);
    let fat32 = entry["FileSystem"] == "FAT32";
    let too_big: Vec<&String> = rows.iter().filter(|r| fat32 && r.size > FAT32_MAX).map(|r| &r.name).collect();
    Ok(json!({"drive": entry, "bytes": total, "files": rows, "fits": total <= free, "fat32": too_big}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotes() {
        assert_eq!(host(" 192.168.1.1 ").ok().as_deref(), Some("192.168.1.1"));
        assert_eq!(host("[::1]").ok().as_deref(), Some("::1"));
        assert_eq!(host("fe80::1%12").ok().as_deref(), Some("fe80::1%12"));
        assert!(host("fe80::1%eth0").is_err());
        assert_eq!(host("Example.COM.").ok().as_deref(), Some("example.com"));
        for bad in ["", "http://x", "a b", "x:80", "-a.com", "a-.com", "a..b", "a;rm -rf", "$(calc)"] {
            assert!(host(bad).is_err(), "{bad}");
        }
        assert!(host(&format!("{}.com", "a".repeat(64))).is_err());
    }

    #[test]
    fn ports() {
        assert_eq!(port(443).ok(), Some(443));
        assert!(port(0).is_err());
        assert!(port(65536).is_err());
    }

    #[test]
    fn test_inconnu() {
        let req = NetworkRequest { test: "nmap".into(), host: "127.0.0.1".into(), port: 443, internet: String::new() };
        assert!(network(&req).is_err());
    }
}
