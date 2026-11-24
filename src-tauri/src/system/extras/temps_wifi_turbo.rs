use serde::Serialize;

use super::{parse_json_arr, ps, ps_ok, ps_timeout};

// ─── Températures ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TemperatureReading {
    pub sensor_name: String,
    pub sensor_type: String,
    pub temp_celsius: f32,
    pub source: String,
}

#[tauri::command]
pub async fn get_temperatures() -> Result<Vec<TemperatureReading>, String> {
    tokio::task::spawn_blocking(get_temperatures_sync)
        .await
        .map_err(|e| e.to_string())?
}

fn get_temperatures_sync() -> Result<Vec<TemperatureReading>, String> {
    let mut temps: Vec<TemperatureReading> = Vec::new();

    // 1. LibreHardwareMonitor WMI
    let script_lhm = r#"
try {
    $sensors = Get-CimInstance -Namespace root/LibreHardwareMonitor -ClassName Sensor -ErrorAction SilentlyContinue |
        Where-Object { $_.SensorType -eq 'Temperature' -and $_.Value -gt 0 -and $_.Value -lt 120 -and $_.Name -notmatch 'TjMax' } |
        Select-Object Name, Value, Hardware
    if ($sensors) { $sensors | ConvertTo-Json -Compress -Depth 2 }
} catch {}
"#;
    let mut lhm_found = false;
    if let Ok(out) = ps_timeout(script_lhm, 20) {
        if !out.is_empty() {
            let arr: Vec<serde_json::Value> = parse_json_arr(&out);
            for v in &arr {
                let t = v["Value"].as_f64().unwrap_or(0.0) as f32;
                let name = v["Name"].as_str().unwrap_or("?").to_string();
                let hw = v["Hardware"].as_str().unwrap_or("").to_string();
                if t > 0.0 && t < 120.0 {
                    let sensor_type = if hw.to_lowercase().contains("cpu") || name.to_lowercase().contains("core") || name.to_lowercase().contains("package") { "CPU" }
                        else if hw.to_lowercase().contains("gpu") || hw.to_lowercase().contains("video") { "GPU" }
                        else if hw.to_lowercase().contains("ssd") || hw.to_lowercase().contains("hdd") || hw.to_lowercase().contains("nvme") { "Storage" }
                        else { "Autre" };
                    temps.push(TemperatureReading { sensor_name: name, sensor_type: sensor_type.to_string(), temp_celsius: t, source: "LibreHardwareMonitor".to_string() });
                    lhm_found = true;
                }
            }
        }
    }

    // 2. OpenHardwareMonitor WMI
    if !lhm_found {
        let script_ohm = r#"
try {
    $sensors = Get-CimInstance -Namespace root/OpenHardwareMonitor -ClassName Sensor -ErrorAction SilentlyContinue |
        Where-Object { $_.SensorType -eq 'Temperature' -and $_.Value -gt 0 -and $_.Name -notmatch 'TjMax' }
    if ($sensors) { $sensors | Select-Object Name, Value | ConvertTo-Json -Compress }
} catch {}
"#;
        if let Ok(out) = ps_timeout(script_ohm, 20) {
            if !out.is_empty() {
                let arr: Vec<serde_json::Value> = parse_json_arr(&out);
                for v in &arr {
                    let t = v["Value"].as_f64().unwrap_or(0.0) as f32;
                    let name = v["Name"].as_str().unwrap_or("?").to_string();
                    if t > 0.0 && t < 120.0 {
                        let st = if name.to_lowercase().contains("core") || name.to_lowercase().contains("package") { "CPU" }
                            else if name.to_lowercase().contains("gpu") { "GPU" } else { "Autre" };
                        temps.push(TemperatureReading { sensor_name: name, sensor_type: st.to_string(), temp_celsius: t, source: "OpenHardwareMonitor".to_string() });
                    }
                }
            }
        }
    }

    // 3. NVIDIA GPU via nvidia-smi
    if let Ok(out) = crate::maintenance::commands::execute_system_command(
        "nvidia-smi", &["--query-gpu=name,temperature.gpu,pci.sub_device_id", "--format=csv,noheader"], 10,
    ) {
        let text = out.stdout;
        for (i, line) in text.lines().enumerate() {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 2 {
                if let Ok(t) = parts[1].parse::<f32>() {
                    if t > 0.0 {
                        let name = if i == 0 { format!("GPU — {}", parts[0]) } else { format!("GPU {} — {}", i + 1, parts[0]) };
                        if !temps.iter().any(|r| r.sensor_name == name) {
                            temps.push(TemperatureReading { sensor_name: name, sensor_type: "GPU".to_string(), temp_celsius: t, source: "nvidia-smi".to_string() });
                        }
                    }
                }
            }
        }
    }

    // 4. Disques SMART
    let script_disk = r#"
try {
    Get-PhysicalDisk -ErrorAction SilentlyContinue | ForEach-Object {
        $rel = $_ | Get-StorageReliabilityCounter -ErrorAction SilentlyContinue
        if ($rel -and $rel.Temperature -gt 0) {
            [PSCustomObject]@{ name=$_.FriendlyName; temp=$rel.Temperature; media=$_.MediaType }
        }
    } | ConvertTo-Json -Compress
} catch {}
"#;
    if let Ok(out) = ps_timeout(script_disk, 20) {
        if !out.is_empty() {
            let arr: Vec<serde_json::Value> = parse_json_arr(&out);
            for v in &arr {
                let t = v["temp"].as_f64().unwrap_or(0.0) as f32;
                let name = v["name"].as_str().unwrap_or("Disque").to_string();
                if t > 0.0 && !temps.iter().any(|r| r.sensor_name == name && r.sensor_type == "Storage") {
                    temps.push(TemperatureReading { sensor_name: name, sensor_type: "Storage".to_string(), temp_celsius: t, source: "SMART".to_string() });
                }
            }
        }
    }

    // 5. Fallback ACPI thermal zones
    if temps.iter().filter(|r| r.sensor_type == "CPU").count() == 0 {
        let script_acpi = r#"
try {
    $tz = Get-CimInstance -Namespace root/wmi -ClassName MSAcpi_ThermalZoneTemperature -ErrorAction SilentlyContinue
    if ($tz) { $tz | ForEach-Object { [PSCustomObject]@{ temp=[math]::Round(($_.CurrentTemperature/10)-273.15,1) } } | ConvertTo-Json -Compress }
} catch {}
"#;
        if let Ok(out) = ps_timeout(script_acpi, 20) {
            if !out.is_empty() {
                let arr: Vec<serde_json::Value> = parse_json_arr(&out);
                for (i, v) in arr.iter().enumerate() {
                    let t = v["temp"].as_f64().unwrap_or(0.0) as f32;
                    if t > 0.0 && t < 120.0 {
                        temps.push(TemperatureReading {
                            // Pas « CPU Package » : valeur firmware ACPI, souvent figée.
                            sensor_name: format!("Zone thermique ACPI {} (approximatif)", i + 1),
                            sensor_type: "CPU".to_string(), temp_celsius: t, source: "ACPI WMI".to_string(),
                        });
                    }
                }
            }
        }
    }

    if temps.is_empty() {
        temps.push(TemperatureReading {
            sensor_name: "Aucun capteur accessible".to_string(),
            sensor_type: "Info".to_string(), temp_celsius: -1.0,
            source: "Installez LibreHardwareMonitor et lancez-le en administrateur".to_string(),
        });
    }

    Ok(temps)
}

#[derive(Serialize)]
pub struct CoreTemp {
    pub core: u32,
    pub label: String,
    pub temp_celsius: f32,
}

// Anti-freeze : WMI/CIM est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn get_cpu_core_temps() -> Result<Vec<CoreTemp>, String> {
    tokio::task::spawn_blocking(get_cpu_core_temps_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn get_cpu_core_temps_blocking() -> Result<Vec<CoreTemp>, String> {
    let script = r#"
$result = @()
try {
    $zones = Get-CimInstance -Namespace root/wmi -ClassName MSAcpi_ThermalZoneTemperature -ErrorAction SilentlyContinue
    if ($zones) {
        $i = 0
        foreach ($z in $zones) {
            $c = [math]::Round(($z.CurrentTemperature / 10) - 273.15, 1)
            if ($c -gt 0 -and $c -lt 120) {
                $result += [PSCustomObject]@{ core=$i; label=$z.InstanceName; temp=$c }
                $i++
            }
        }
    }
} catch {}
$result | ConvertTo-Json -Compress"#;
    let out = ps(script)?;
    if out.is_empty() { return Ok(vec![]); }
    let arr: Vec<serde_json::Value> = parse_json_arr(&out);
    Ok(arr.iter().filter_map(|v| {
        let t = v["temp"].as_f64().unwrap_or(0.0) as f32;
        if t > 0.0 && t < 120.0 {
            Some(CoreTemp {
                core: v["core"].as_u64().unwrap_or(0) as u32,
                label: v["label"].as_str().unwrap_or("").to_string(),
                temp_celsius: t,
            })
        } else { None }
    }).collect())
}

#[derive(Serialize)]
pub struct GpuTemp {
    pub name: String,
    pub temp_celsius: f32,
    pub source: String,
}

// Anti-freeze : PowerShell est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn get_gpu_temps() -> Result<Vec<GpuTemp>, String> {
    tokio::task::spawn_blocking(get_gpu_temps_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn get_gpu_temps_blocking() -> Result<Vec<GpuTemp>, String> {
    let script = r#"
$result = @()
$seen = @{}

function Add-Entry($name, $temp, $src) {
    $k = "$name|$src"
    if (-not $seen.ContainsKey($k) -and $temp -gt 1 -and $temp -lt 120) {
        $seen[$k] = $true
        $script:result += [PSCustomObject]@{ name=$name; temp=[math]::Round($temp,1); src=$src }
    }
}

# ── 1. nvidia-smi ──────────────────────────────────────────────────────────
try {
    $nsmi = Get-Command nvidia-smi -ErrorAction SilentlyContinue
    if ($nsmi) {
        $raw = & nvidia-smi --query-gpu=name,temperature.gpu --format=csv,noheader,nounits 2>$null
        if ($raw) {
            $raw -split "`n" | Where-Object { $_.Trim() } | ForEach-Object {
                $parts = $_ -split ',\s*'
                if ($parts.Count -ge 2) {
                    $t = [double]$parts[-1].Trim()
                    $n = ($parts[0..($parts.Count-2)] -join ', ').Trim()
                    Add-Entry $n $t 'nvidia-smi'
                }
            }
        }
    }
} catch {}

# ── 2. OpenHardwareMonitor WMI ──────────────────────────────────────────
try {
    $ohm = Get-CimInstance -Namespace root/OpenHardwareMonitor -ClassName Sensor -ErrorAction SilentlyContinue |
           Where-Object { $_.SensorType -eq 'Temperature' -and $_.Name -match 'GPU|Core|Hot' -and $_.Name -notmatch 'TjMax' }
    if ($ohm) {
        $ohm | ForEach-Object { Add-Entry $_.Name $_.Value 'OpenHardwareMonitor' }
    }
} catch {}

# ── 3. LibreHardwareMonitor WMI ──────────────────────────────────────────
try {
    $lhm = Get-CimInstance -Namespace root/LibreHardwareMonitor -ClassName Sensor -ErrorAction SilentlyContinue |
           Where-Object { $_.SensorType -eq 'Temperature' -and ($_.Name -match 'GPU' -or $_.HardwareName -match 'GPU|Radeon|GeForce|RX |RTX |GTX ') }
    if ($lhm) {
        $lhm | ForEach-Object { Add-Entry "$($_.HardwareName) / $($_.Name)" $_.Value 'LibreHardwareMonitor' }
    }
} catch {}

# ── 4. ACPI Thermal Zones ────────────────────────────────────────────────
try {
    $zones = Get-CimInstance -Namespace root/wmi -ClassName MSAcpi_ThermalZoneTemperature -ErrorAction SilentlyContinue
    if ($zones) {
        $zones | ForEach-Object {
            $c = ($_.CurrentTemperature / 10) - 273.15
            $inst = $_.InstanceName.ToLower()
            if ($inst -match 'gpu|disp|vid|vga|thm1|thm2') {
                Add-Entry "GPU Thermal ($($_.InstanceName))" $c 'ACPI WMI'
            }
        }
    }
} catch {}

# ── 5. Win32_VideoController (legacy) ────────────────────────────────────
try {
    Get-WmiObject Win32_VideoController -ErrorAction SilentlyContinue | ForEach-Object {
        if ($_.CurrentTemperature -and $_.CurrentTemperature -gt 0) {
            $c = ($_.CurrentTemperature / 10) - 273.15
            Add-Entry $_.Name $c 'WMI Win32'
        }
    }
} catch {}

if ($result.Count -eq 0) {
    try {
        Get-WmiObject Win32_VideoController -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -and -not ($_.Name -match 'Microsoft Basic') } |
            ForEach-Object {
                $script:result += [PSCustomObject]@{ name=$_.Name; temp=-1; src='unavailable' }
            }
    } catch {}
}

$result | ConvertTo-Json -Compress -Depth 2"#;
    let out = ps(script)?;
    if out.is_empty() { return Ok(vec![]); }
    let arr: Vec<serde_json::Value> = parse_json_arr(&out);
    Ok(arr.iter().map(|v| {
        let t = v["temp"].as_f64().unwrap_or(-1.0) as f32;
        GpuTemp {
            name: v["name"].as_str().unwrap_or("GPU").to_string(),
            temp_celsius: t,
            source: v["src"].as_str().unwrap_or("unavailable").to_string(),
        }
    }).collect())
}

// ─── WiFi Analyzer ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct WifiNetwork {
    pub ssid: String,
    pub bssid: String,
    pub signal_percent: u32,
    pub channel: u32,
    pub band: String,
    pub authentication: String,
    pub network_type: String,
    pub radio_type: String,
}

// Anti-freeze : scan WiFi via PowerShell est bloquant — jamais inline sur
// le thread de commande.
#[tauri::command]
pub async fn get_nearby_wifi() -> Result<Vec<WifiNetwork>, String> {
    tokio::task::spawn_blocking(get_nearby_wifi_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn get_nearby_wifi_blocking() -> Result<Vec<WifiNetwork>, String> {
    // Délai borné : un pilote Wi-Fi figé ne bloque plus le thread indéfiniment.
    let out = crate::maintenance::commands::execute_system_command(
        "netsh", &["wlan", "show", "networks", "mode=bssid"], 30,
    ).map_err(|e| e.to_string())?;
    // Échec réel (pas d'adaptateur Wi-Fi, service WLAN arrêté, et depuis
    // Windows 11 24H2 : autorisation « Localisation » refusée) : netsh sort en
    // code non nul avec l'explication sur stdout. Avant, on renvoyait une liste
    // vide et l'UI disait juste « Aucun réseau ».
    if !out.success {
        let msg = [out.stdout.trim(), out.stderr.trim()].into_iter().find(|s| !s.is_empty()).unwrap_or("netsh wlan a échoué");
        return Err(msg.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" "));
    }
    let mut networks = parse_netsh_networks(&out.stdout);
    networks.sort_by_key(|a| std::cmp::Reverse(a.signal_percent));
    Ok(networks)
}

fn strip_accents(s: &str) -> String {
    s.chars().map(|c| match c {
        'é'|'è'|'ê'|'ë' => 'e', 'à'|'â'|'ä' => 'a', 'î'|'ï' => 'i',
        'ô'|'ö' => 'o', 'û'|'ü'|'ù' => 'u', 'ç' => 'c', _ => c,
    }).collect()
}

/// « 2,4 GHz » (FR) / « 2.4 GHz » (EN) / « 5 GHz » / « 6 GHz » → libellé normalisé.
fn normalize_band(raw: &str) -> String {
    let t = raw.trim();
    if t.starts_with('2') { "2.4 GHz".into() }
    else if t.starts_with('5') { "5 GHz".into() }
    else if t.starts_with('6') { "6 GHz".into() }
    else { t.to_string() }
}

/// Repli quand netsh n'affiche pas la bande (Windows 10) : 1-14 = 2,4 GHz,
/// 32-177 = 5 GHz. Le 6 GHz réutilise les numéros 1-233 : indécidable sans la ligne « Bande ».
fn band_from_channel(ch: u32) -> String {
    match ch { 1..=14 => "2.4 GHz".into(), 32..=177 => "5 GHz".into(), _ => String::new() }
}

/// Analyse `netsh wlan show networks mode=bssid`, en français ou en anglais.
/// Une entrée PAR BSSID : une box bi-bande publie le même SSID sur 2,4 et 5 GHz
/// (canaux, signaux différents) ; l'ancien parseur gardait seulement le dernier
/// BSSID de chaque SSID. Libellés comparés EXACTEMENT : en FR, « Utilisation du
/// canal : 57 (22 %) » (charge BSS) contient « canal » et écrasait le vrai canal par 0.
fn parse_netsh_networks(text: &str) -> Vec<WifiNetwork> {
    #[derive(Default, Clone)]
    struct Ssid { ssid: String, network_type: String, authentication: String }
    let mut out: Vec<WifiNetwork> = Vec::new();
    let mut ssid = Ssid::default();
    let mut cur: Option<WifiNetwork> = None;
    let finish = |cur: &mut Option<WifiNetwork>, out: &mut Vec<WifiNetwork>| {
        if let Some(mut n) = cur.take() {
            if n.band.is_empty() { n.band = band_from_channel(n.channel); }
            out.push(n);
        }
    };
    for line in text.lines() {
        let Some((label, val)) = line.split_once(':') else { continue };
        let l = strip_accents(&label.trim().to_lowercase());
        let val = val.trim();
        let word = l.split_whitespace().next().unwrap_or("");
        if word == "ssid" {
            finish(&mut cur, &mut out);
            // SSID masqué : netsh affiche une valeur vide
            let name = if val.is_empty() { "(réseau masqué)" } else { val };
            ssid = Ssid { ssid: name.to_string(), ..Default::default() };
        } else if word == "bssid" {
            finish(&mut cur, &mut out);
            cur = Some(WifiNetwork {
                ssid: ssid.ssid.clone(), bssid: val.to_string(), signal_percent: 0, channel: 0,
                band: String::new(), authentication: ssid.authentication.clone(),
                network_type: ssid.network_type.clone(), radio_type: String::new(),
            });
        } else {
            match l.as_str() {
                "type de reseau" | "network type" => ssid.network_type = val.to_string(),
                "authentification" | "authentication" => ssid.authentication = val.to_string(),
                _ => {}
            }
            if let Some(n) = cur.as_mut() {
                match l.as_str() {
                    "signal" => n.signal_percent = val.trim_end_matches('%').trim().parse::<u32>().unwrap_or(0).min(100),
                    "type de radio" | "radio type" => n.radio_type = val.to_string(),
                    "bande" | "band" => n.band = normalize_band(val),
                    "canal" | "channel" => n.channel = val.parse().unwrap_or(0),
                    _ => {}
                }
            }
        }
    }
    finish(&mut cur, &mut out);
    out
}

// ─── Mode Turbo / Profils ─────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TurboResult {
    pub actions_done: Vec<String>,
    pub errors: Vec<String>,
}

// Protocole de succès réel : chaque script prouve son résultat — try/catch
// avec -ErrorAction Stop puis `exit 1`, ou test de $LASTEXITCODE pour les
// exécutables natifs — et est lancé via ps_ok() qui vérifie le code de sortie.
// Avant, le succès était jugé par ps().is_ok() (vrai dès que powershell.exe
// démarre) et les -ErrorAction SilentlyContinue rendaient invisibles tous les
// échecs, notamment sans droits admin : l'UI affichait « fait » à tort.

const PS_HIGH_PERF: &str = "powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c 2>&1 | Out-Null; if ($LASTEXITCODE -ne 0) { Write-Output 'powercfg a échoué (plan absent ?)'; exit 1 }";
const PS_BALANCED: &str = "powercfg /setactive 381b4222-f694-41f0-9685-ff5bb260df2e 2>&1 | Out-Null; if ($LASTEXITCODE -ne 0) { Write-Output 'powercfg a échoué (plan absent ?)'; exit 1 }";
const PS_POWER_SAVER: &str = "powercfg /setactive a1841308-3541-4fab-bc81-f71556f20b4a 2>&1 | Out-Null; if ($LASTEXITCODE -ne 0) { Write-Output 'powercfg a échoué (plan absent ?)'; exit 1 }";
const PS_HW_SCH: &str = r#"try {
    Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers' -Name 'HwSchMode' -Value 2 -Type DWord -ErrorAction Stop
} catch { Write-Output $_.Exception.Message; exit 1 }"#;

// Anti-freeze : PowerShell est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn apply_turbo_mode(mode: String) -> Result<TurboResult, String> {
    tokio::task::spawn_blocking(move || apply_turbo_mode_blocking(mode))
        .await
        .map_err(|e| e.to_string())?
}

fn apply_turbo_mode_blocking(mode: String) -> Result<TurboResult, String> {
    let mut done: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    fn run(script: &str, ok_msg: &str, err_label: &str, done: &mut Vec<String>, errors: &mut Vec<String>) {
        match ps_ok(script) {
            Ok(_) => done.push(ok_msg.to_string()),
            Err(e) => errors.push(format!("{err_label} : {e}")),
        }
    }

    match mode.as_str() {
        "gaming" => {
            run(PS_HIGH_PERF, "Plan d'alimentation : Haute performance", "Plan d'alimentation", &mut done, &mut errors);
            // Disable-AppxPackage n'existe pas (l'ancien script échouait toujours
            // en silence) → désactivation Game Bar/DVR via registre HKCU, réversible.
            run(r#"try {
    foreach ($p in 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR', 'HKCU:\System\GameConfigStore') {
        if (-not (Test-Path $p)) { New-Item -Path $p -Force -ErrorAction Stop | Out-Null }
    }
    Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR' -Name 'AppCaptureEnabled' -Value 0 -Type DWord -ErrorAction Stop
    Set-ItemProperty -Path 'HKCU:\System\GameConfigStore' -Name 'GameDVR_Enabled' -Value 0 -Type DWord -ErrorAction Stop
} catch { Write-Output $_.Exception.Message; exit 1 }"#,
                "Xbox Game Bar / DVR désactivé (réversible)", "Xbox Game Bar", &mut done, &mut errors);
            run(PS_HW_SCH, "GPU Hardware Scheduling activé", "GPU Hardware Scheduling", &mut done, &mut errors);
            run(r#"try {
    $p = 'HKCU:\Software\Microsoft\GameBar'
    if (-not (Test-Path $p)) { New-Item -Path $p -Force -ErrorAction Stop | Out-Null }
    Set-ItemProperty -Path $p -Name 'AllowAutoGameMode' -Value 1 -Type DWord -ErrorAction Stop
    Set-ItemProperty -Path $p -Name 'AutoGameModeEnabled' -Value 1 -Type DWord -ErrorAction Stop
} catch { Write-Output $_.Exception.Message; exit 1 }"#,
                "Game Mode Windows activé", "Game Mode", &mut done, &mut errors);
        }
        "work" => {
            run(PS_BALANCED, "Plan d'alimentation : Équilibré", "Plan d'alimentation", &mut done, &mut errors);
            run(r#"try {
    $p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\VisualEffects'
    if (-not (Test-Path $p)) { New-Item -Path $p -Force -ErrorAction Stop | Out-Null }
    Set-ItemProperty -Path $p -Name 'VisualFXSetting' -Value 2 -Type DWord -ErrorAction Stop
} catch { Write-Output $_.Exception.Message; exit 1 }"#,
                "Effets visuels optimisés", "Effets visuels", &mut done, &mut errors);
            // Clear-Clipboard n'existe pas en PowerShell 5.1 → WinForms.
            run(r#"try {
    Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
    [System.Windows.Forms.Clipboard]::Clear()
} catch { Write-Output $_.Exception.Message; exit 1 }"#,
                "Presse-papiers vidé", "Presse-papiers", &mut done, &mut errors);
        }
        "eco" => {
            run(PS_POWER_SAVER, "Plan d'alimentation : Économie d'énergie", "Plan d'alimentation", &mut done, &mut errors);
            run(r#"try {
    $m = Get-CimInstance -Namespace root/wmi -ClassName WmiMonitorBrightnessMethods -ErrorAction Stop | Select-Object -First 1
    if (-not $m) { throw 'Aucun écran à luminosité pilotable (poste fixe ?)' }
    Invoke-CimMethod -InputObject $m -MethodName WmiSetBrightness -Arguments @{ Timeout = [uint32]1; Brightness = [byte]50 } -ErrorAction Stop | Out-Null
} catch { Write-Output $_.Exception.Message; exit 1 }"#,
                "Luminosité réduite à 50%", "Luminosité", &mut done, &mut errors);
        }
        _ => {
            run(PS_HIGH_PERF, "Plan d'alimentation : Haute performance", "Plan d'alimentation", &mut done, &mut errors);
            run("try { Clear-DnsClientCache -ErrorAction Stop } catch { Write-Output $_.Exception.Message; exit 1 }",
                "Cache DNS vidé", "Cache DNS", &mut done, &mut errors);
            // L'ancien script (AllocHGlobal(0) + GC.Collect dans le process
            // PowerShell) ne libérait rien : EmptyWorkingSet réel + comptage.
            match ps_ok(r#"try {
    $sig = '[System.Runtime.InteropServices.DllImport("psapi.dll")] public static extern int EmptyWorkingSet(System.IntPtr h);'
    Add-Type -MemberDefinition $sig -Name 'MemUtil' -Namespace 'Nitrite' -ErrorAction Stop
    $n = 0
    Get-Process | ForEach-Object { try { if ([Nitrite.MemUtil]::EmptyWorkingSet($_.Handle) -ne 0) { $n++ } } catch {} }
    Write-Output $n
} catch { Write-Output $_.Exception.Message; exit 1 }"#) {
                Ok(n) => done.push(format!("Mémoire libérée ({} processus allégés)", n)),
                Err(e) => errors.push(format!("Libération mémoire : {e}")),
            }
            // Retiré sur demande de Momo : le mode turbo ne tue plus les processus
            // « inutiles » (WorkingSet > 1 Go, CPU < 0,1 %). Le critère était une
            // heuristique dangereuse — un gros logiciel légitime momentanément
            // inactif (éditeur, machine virtuelle, jeu en pause) pouvait être tué
            // de force, avec perte de données. Vider la RAM standby (EmptyWorkingSet
            // ci-dessus) et le plan haute performance suffisent, sans rien fermer.
            run(PS_HW_SCH, "GPU Hardware Scheduling activé", "GPU Hardware Scheduling", &mut done, &mut errors);
        }
    }

    Ok(TurboResult { actions_done: done, errors })
}

// ─── Hidden Power Plans ───────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct PowerPlanResult {
    pub name: String,
    pub guid: String,
    pub success: bool,
    pub message: String,
}

// Anti-freeze : powercfg est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn enable_hidden_power_plans() -> Result<Vec<PowerPlanResult>, String> {
    tokio::task::spawn_blocking(enable_hidden_power_plans_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn enable_hidden_power_plans_blocking() -> Result<Vec<PowerPlanResult>, String> {
    let hidden_plans = vec![
        ("Performances maximales", "e9a42b02-d5df-448d-aa00-03f14749eb61"),
    ];
    let mut results = Vec::new();
    for (name, guid) in &hidden_plans {
        // Succès jugé sur $LASTEXITCODE — l'ancien sniffing de « error »/« erreur »
        // dans stdout dépendait de la langue de Windows.
        let script = format!(
            "$out = powercfg /duplicatescheme {} 2>&1; if ($LASTEXITCODE -ne 0) {{ Write-Output $out; exit 1 }}",
            guid
        );
        let (success, message) = match ps_ok(&script) {
            Ok(_) => (true, "Plan ajouté / déjà disponible".to_string()),
            Err(e) => (false, e),
        };
        results.push(PowerPlanResult {
            name: name.to_string(),
            guid: guid.to_string(),
            success,
            message,
        });
    }
    Ok(results)
}

// ─── Quick Optimization Runner ────────────────────────────────────────────────

// Anti-freeze : PowerShell est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn run_quick_optimization(opt_id: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || run_quick_optimization_blocking(opt_id))
        .await
        .map_err(|e| e.to_string())?
}

fn run_quick_optimization_blocking(opt_id: String) -> Result<String, String> {
    // Même protocole que apply_turbo_mode : les actions qui exigent l'élévation
    // (journaux, SysMain, télémétrie HKLM, TRIM) sortent `exit 1` si rien n'a
    // réellement été appliqué — l'UI affiche alors le toast d'erreur au lieu
    // d'un faux succès. Lancé via ps_ok() (code de sortie vérifié).
    let script = match opt_id.as_str() {
        // Suppression best-effort : les fichiers verrouillés par des process
        // vivants sont normaux, on ne les compte pas comme échec.
        "clean_temp" => r#"
Get-ChildItem "$env:TEMP" -ErrorAction SilentlyContinue | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
Get-ChildItem "C:\Windows\Temp" -ErrorAction SilentlyContinue | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
"Fichiers temporaires supprimés (fichiers verrouillés ignorés)"
"#,
        "flush_dns" => "ipconfig /flushdns 2>&1 | Out-Null; if ($LASTEXITCODE -ne 0) { Write-Output 'ipconfig /flushdns a échoué'; exit 1 }; 'Cache DNS vidé'",
        "clean_eventlog" => r#"
$logs = @("Application","System","Security","Setup")
$n = 0
foreach ($l in $logs) { try { Clear-EventLog -LogName $l -ErrorAction Stop; $n++ } catch {} }
if ($n -eq 0) { Write-Output "Aucun journal vidé — élévation administrateur requise"; exit 1 }
"$n journaux d'événements vidés"
"#,
        "disable_prefetch" => r#"
try {
    Stop-Service -Name SysMain -Force -ErrorAction Stop
    Set-Service -Name SysMain -StartupType Disabled -ErrorAction Stop
} catch { Write-Output $_.Exception.Message; exit 1 }
"Superfetch/SysMain désactivé pour les SSD"
"#,
        "disable_telemetry" => r#"
try {
    $p = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection'
    if (-not (Test-Path $p)) { New-Item -Path $p -Force -ErrorAction Stop | Out-Null }
    Set-ItemProperty $p -Name AllowTelemetry -Value 1 -Type DWord -Force -ErrorAction Stop
} catch { Write-Output $_.Exception.Message; exit 1 }
"Télémétrie réduite au minimum"
"#,
        "visual_perf" => r#"
try {
    $p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\VisualEffects'
    if (-not (Test-Path $p)) { New-Item -Path $p -Force -ErrorAction Stop | Out-Null }
    Set-ItemProperty $p -Name VisualFXSetting -Value 2 -Type DWord -ErrorAction Stop
} catch { Write-Output $_.Exception.Message; exit 1 }
"Effets visuels en mode performance"
"#,
        "optimize_drives" => r#"
$count = 0
Get-Volume | Where-Object { $_.DriveLetter -and $_.DriveType -eq 'Fixed' } | ForEach-Object {
    try { Optimize-Volume -DriveLetter $_.DriveLetter -ReTrim -ErrorAction Stop; $count++ } catch {}
}
if ($count -eq 0) { Write-Output "Aucun volume optimisé — élévation administrateur requise ou volumes non compatibles TRIM"; exit 1 }
"Optimisation TRIM effectuée sur $count volume(s)"
"#,
        "clear_clipboard" => r#"
try {
    Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
    [System.Windows.Forms.Clipboard]::Clear()
} catch { Write-Output $_.Exception.Message; exit 1 }
"Presse-papiers vidé"
"#,
        _ => return Err(format!("Optimisation '{}' inconnue", opt_id)),
    };
    ps_ok(script)
}

#[cfg(test)]
mod wifi_tests {
    use super::*;

    // Sortie réelle relevée sur Windows 11 FR (2026-11-23), abrégée.
    const FR: &str = "Nom de l\u{2019}interface : Wi-Fi \r\nActuellement 2 réseaux sont visibles. \r\n\r\nSSID 1 : SFR_BCB0\r\n    Type de réseau          : Infrastructure\r\n    Authentification        : WPA2 - Personnel\r\n    Chiffrement             : CCMP \r\n    BSSID 1                 : cc:2d:1b:b5:bc:b6\r\n         Signal             : 43%  \r\n         Type de radio         : 802.11n\r\n         Bande               : 2,4 GHz\r\n         Canal            : 11 \r\n         Charge BSS :\r\n             Stations connectées :         1\r\n             Utilisation du canal :        57 (22 %)\r\n         Taux de base (Mbits/s) : 1 2 5.5 11\r\n    BSSID 2                 : 6a:2d:1b:b5:bc:b7\r\n         Signal             : 31%  \r\n         Type de radio         : 802.11ac\r\n         Bande               : 5 GHz\r\n         Canal            : 100 \r\n             Utilisation du canal :        4 (1 %)\r\n\r\nSSID 2 : \r\n    Type de réseau             : Infrastructure\r\n    Authentification           : Ouvrir\r\n    BSSID 1                 : 3a:07:16:3b:7b:24\r\n         Signal             : 70%  \r\n         Type de radio         : 802.11be\r\n         Bande               : 6 GHz\r\n         Canal            : 37 \r\n";

    const EN: &str = "SSID 1 : Home:Net\n    Network type            : Infrastructure\n    Authentication          : WPA3-Personal\n    Encryption              : CCMP\n    BSSID 1                 : aa:bb:cc:dd:ee:ff\n         Signal             : 88%\n         Radio type         : 802.11ax\n         Channel            : 6\n         Channel Utilization:        10 (4 %)\n";

    #[test]
    fn parses_french_output_one_entry_per_bssid() {
        let n = parse_netsh_networks(FR);
        assert_eq!(n.len(), 3);
        assert_eq!((n[0].ssid.as_str(), n[0].channel, n[0].band.as_str(), n[0].signal_percent), ("SFR_BCB0", 11, "2.4 GHz", 43));
        assert_eq!(n[0].authentication, "WPA2 - Personnel");
        assert_eq!(n[0].network_type, "Infrastructure");
        assert_eq!((n[1].ssid.as_str(), n[1].channel, n[1].band.as_str(), n[1].radio_type.as_str()), ("SFR_BCB0", 100, "5 GHz", "802.11ac"));
        assert_eq!((n[2].ssid.as_str(), n[2].band.as_str(), n[2].authentication.as_str()), ("(réseau masqué)", "6 GHz", "Ouvrir"));
    }

    #[test]
    fn parses_english_output_and_infers_band() {
        let n = parse_netsh_networks(EN);
        assert_eq!(n.len(), 1);
        // Pas de ligne Band (Windows 10) : déduite du canal ; « Channel Utilization » ignoré
        assert_eq!((n[0].ssid.as_str(), n[0].channel, n[0].band.as_str()), ("Home:Net", 6, "2.4 GHz"));
        assert_eq!(n[0].authentication, "WPA3-Personal");
    }
}
