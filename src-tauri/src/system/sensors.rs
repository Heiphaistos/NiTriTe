use serde::Serialize;
use crate::utils::ps::ps;

#[derive(Serialize, Default, Clone)]
pub struct SensorReading {
    pub hardware: String,
    pub hardware_type: String,
    pub name: String,
    pub sensor_type: String,
    pub value: f64,
    pub unit: String,
    pub min: f64,
    pub max: f64,
    pub source: String,
}

fn unit_for(sensor_type: &str) -> &'static str {
    match sensor_type {
        "Temperature" => "°C",
        "Fan"         => "RPM",
        "Voltage"     => "V",
        "Load"        => "%",
        "Clock"       => "MHz",
        "Power"       => "W",
        "Control"     => "%",
        "TjMaxMargin" => "°C",
        "Data"        => "GB",
        "SmallData"   => "MB",
        _             => "",
    }
}

fn hw_type_label(hw_type: &str) -> &'static str {
    // Case-insensitive + les deux nomenclatures : OpenHardwareMonitor (CPU,
    // GpuAti, RAM, Mainboard, HDD) ET LibreHardwareMonitor (Cpu, GpuAmd, Memory,
    // Motherboard, Storage). Sans ça, les capteurs LHM tombaient tous en « Autre ».
    match hw_type.to_lowercase().as_str() {
        "cpu"                       => "CPU",
        "gpunvidia"                 => "GPU NVIDIA",
        "gpuamd" | "gpuati"         => "GPU AMD",
        "gpuintel"                  => "GPU Intel",
        "motherboard" | "mainboard" => "Carte mère",
        "superio"                   => "SuperIO",
        "ram" | "memory"            => "RAM",
        "hdd" | "ssd" | "storage"   => "Stockage",
        "nvme"                      => "Stockage NVMe",
        "network"                   => "Réseau",
        "embeddedcontroller"        => "EC",
        _                           => "Autre",
    }
}

/// LHM/OHM publient « CPU Core #n Distance to TjMax » avec SensorType
/// `Temperature` : c'est une MARGE (degrés restants avant le throttling), pas
/// une température. Affichée comme telle, une marge de 60 °C passait pour un
/// cœur à 60 °C (« Chaud »), et un cœur proche du throttling semblait froid.
/// On la reclasse dans un type à part, avec un libellé clair.
fn reclassify_sensor(name: &str, sensor_type: &str) -> (String, String) {
    if sensor_type == "Temperature" && name.to_lowercase().contains("tjmax") {
        let base = name
            .replace("Distance to TjMax", "")
            .replace("distance to TjMax", "")
            .trim()
            .to_string();
        let label = if base.is_empty() {
            "Marge avant TjMax".to_string()
        } else {
            format!("{base} : marge avant TjMax")
        };
        return (label, "TjMaxMargin".to_string());
    }
    (name.to_string(), sensor_type.to_string())
}

/// Convertit une ligne JSON LHM/OHM en lecture (None = lecture à ignorer).
fn parse_hw_monitor_row(v: &serde_json::Value, source: &str) -> Option<SensorReading> {
    let value = v["value"].as_f64().unwrap_or(0.0);
    // Une tension de 0.0 V est une lecture valide (rail non alimenté).
    if value == 0.0 && v["sensor_type"].as_str().unwrap_or("") != "Voltage" { return None; }
    let (name, st) = reclassify_sensor(
        v["name"].as_str().unwrap_or(""),
        v["sensor_type"].as_str().unwrap_or(""),
    );
    let hw_type = v["hardware_type"].as_str().unwrap_or("");
    Some(SensorReading {
        hardware:      v["hardware"].as_str().unwrap_or("").to_string(),
        hardware_type: hw_type_label(hw_type).to_string(),
        name,
        unit:          unit_for(&st).to_string(),
        min:           v["min"].as_f64().unwrap_or(0.0),
        max:           v["max"].as_f64().unwrap_or(0.0),
        sensor_type:   st,
        value,
        source:        source.to_string(),
    })
}

fn query_lhm() -> Vec<SensorReading> {
    let script = r#"
$ns = 'root/LibreHardwareMonitor'
$sensors = @()
try {
    $hwList = Get-WmiObject -Namespace $ns -Class Hardware -ErrorAction Stop
    $hwMap  = @{}
    foreach ($h in $hwList) { $hwMap[$h.Identifier] = $h }
    $sensorList = Get-WmiObject -Namespace $ns -Class Sensor -ErrorAction Stop
    foreach ($s in $sensorList) {
        $hw = $hwMap[$s.Parent]
        $hwName = if ($hw) { $hw.Name } else { $s.Parent }
        $hwType = if ($hw) { $hw.HardwareType } else { '' }
        $sensors += [PSCustomObject]@{
            hardware     = $hwName
            hardware_type= $hwType
            name         = $s.Name
            sensor_type  = $s.SensorType
            value        = [double]$s.Value
            min          = [double]$s.Min
            max          = [double]$s.Max
        }
    }
} catch {}
@($sensors) | ConvertTo-Json -Compress -Depth 2
"#;
    let out = ps(script).unwrap_or_default();
    if out.trim().is_empty() || out.trim() == "null" { return vec![]; }
    let arr: Vec<serde_json::Value> = serde_json::from_str(out.trim()).unwrap_or_default();
    arr.iter().filter_map(|v| parse_hw_monitor_row(v, "LibreHardwareMonitor")).collect()
}

fn query_ohm() -> Vec<SensorReading> {
    let script = r#"
$ns = 'root/OpenHardwareMonitor'
$sensors = @()
try {
    $hwList = Get-WmiObject -Namespace $ns -Class Hardware -ErrorAction Stop
    $hwMap  = @{}
    foreach ($h in $hwList) { $hwMap[$h.Identifier] = $h }
    $sensorList = Get-WmiObject -Namespace $ns -Class Sensor -ErrorAction Stop
    foreach ($s in $sensorList) {
        $hw = $hwMap[$s.Parent]
        $hwName = if ($hw) { $hw.Name } else { $s.Parent }
        $hwType = if ($hw) { $hw.HardwareType } else { '' }
        $sensors += [PSCustomObject]@{
            hardware=$hwName; hardware_type=$hwType; name=$s.Name;
            sensor_type=$s.SensorType; value=[double]$s.Value; min=[double]$s.Min; max=[double]$s.Max
        }
    }
} catch {}
@($sensors) | ConvertTo-Json -Compress -Depth 2
"#;
    let out = ps(script).unwrap_or_default();
    if out.trim().is_empty() || out.trim() == "null" { return vec![]; }
    let arr: Vec<serde_json::Value> = serde_json::from_str(out.trim()).unwrap_or_default();
    arr.iter().filter_map(|v| parse_hw_monitor_row(v, "OpenHardwareMonitor")).collect()
}

fn query_nvidia_smi() -> Vec<SensorReading> {
    // Chemins absolus connus uniquement — pas de fallback PATH pour éviter le PATH hijacking
    let known_paths = [
        r"C:\Program Files\NVIDIA Corporation\NVSMI\nvidia-smi.exe",
        r"C:\Windows\System32\nvidia-smi.exe",
    ];
    let paths: Vec<&str> = known_paths.iter()
        .filter(|p| std::path::Path::new(p).exists())
        .copied()
        .collect();
    if paths.is_empty() {
        return vec![];
    }
    for path in &paths {
        // Timeout : nvidia-smi peut se figer (pilote planté, GPU en reset) ;
        // `.output()` brut bloquait alors la page (sondée toutes les 3 s).
        if let Ok(out) = crate::maintenance::commands::execute_system_command(
            path,
            &[
                "--query-gpu=name,temperature.gpu,fan.speed,power.draw,clocks.gr,clocks.mem,memory.used,memory.total,utilization.gpu",
                "--format=csv,noheader,nounits",
            ],
            10,
        ) {
            let text = out.stdout;
            let mut result = vec![];
            for line in text.lines() {
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() < 9 { continue; }
                let hw = parts[0].to_string();
                let parse = |s: &str| s.trim().parse::<f64>().unwrap_or(0.0);
                let sensors = [
                    ("GPU Température",  "Temperature", parse(parts[1]), "°C"),
                    // fan.speed est un POURCENTAGE de la vitesse max, pas des tr/min.
                    ("GPU Ventilateur",  "Control",     parse(parts[2]), "%"),
                    ("GPU Puissance",    "Power",       parse(parts[3]), "W"),
                    ("GPU Core Clock",   "Clock",       parse(parts[4]), "MHz"),
                    ("GPU Mem Clock",    "Clock",       parse(parts[5]), "MHz"),
                    ("GPU Mémoire utilisée", "Data",    parse(parts[6]), "MB"),
                    ("GPU Utilisation",  "Load",        parse(parts[8]), "%"),
                ];
                for (name, st, value, unit) in sensors {
                    if value > 0.0 {
                        result.push(SensorReading {
                            hardware: hw.clone(),
                            hardware_type: "GPU NVIDIA".to_string(),
                            name: name.to_string(),
                            sensor_type: st.to_string(),
                            value,
                            unit: unit.to_string(),
                            min: 0.0, max: 0.0,
                            source: "nvidia-smi".to_string(),
                        });
                    }
                }
            }
            if !result.is_empty() { return result; }
        }
    }
    vec![]
}

fn query_storage_temps() -> Vec<SensorReading> {
    let script = r#"
$res = @()
try {
    Get-PhysicalDisk -ErrorAction Stop | ForEach-Object {
        $disk = $_
        $rel = Get-StorageReliabilityCounter -PhysicalDisk $disk -ErrorAction SilentlyContinue
        # TemperatureMax n'est PAS une lecture actuelle (seuil / maximum relevé) :
        # affichée comme une carte à part, elle passait pour un disque
        # « Critique » à 84 °C. Elle sert désormais de borne `max`.
        if ($rel -and $rel.Temperature -gt 0) {
            $res += [PSCustomObject]@{
                hardware="$($disk.FriendlyName)"; name='Température'; value=[double]$rel.Temperature
                max=[double]$rel.TemperatureMax
            }
        }
    }
} catch {}
@($res) | ConvertTo-Json -Compress -Depth 2
"#;
    let out = ps(script).unwrap_or_default();
    if out.trim().is_empty() || out.trim() == "null" { return vec![]; }
    let arr: Vec<serde_json::Value> = serde_json::from_str(out.trim()).unwrap_or_default();
    arr.iter().filter_map(|v| {
        let value = v["value"].as_f64().unwrap_or(0.0);
        if value == 0.0 { return None; }
        Some(SensorReading {
            hardware:      v["hardware"].as_str().unwrap_or("Disque").to_string(),
            hardware_type: "Stockage".to_string(),
            name:          v["name"].as_str().unwrap_or("Température").to_string(),
            sensor_type:   "Temperature".to_string(),
            value,
            unit:          "°C".to_string(),
            min: 0.0,
            max: v["max"].as_f64().unwrap_or(0.0),
            source:        "SMART".to_string(),
        })
    }).collect()
}

/// Groupe commun aux zones thermiques ACPI (MSAcpi_ThermalZoneTemperature et
/// compteurs « Thermal Zone Information », qui exposent la même donnée) :
/// valeur du firmware, souvent figée (ex. 27,8 °C) ou mesurée loin du cœur.
/// Libellée « approximatif » pour ne pas la confondre avec une sonde CPU.
const ACPI_HARDWARE: &str = "Zones thermiques ACPI (approximatif)";

fn query_acpi_temps() -> Vec<SensorReading> {
    let script = r#"
$res = @()
$zoneIdx = 0
function Label-Zone($raw) {
    switch -Wildcard ($raw.ToUpper()) {
        '*CPU*'  { return 'Processeur' }
        '*GPU*'  { return 'GPU intégré' }
        '*THM0*' { return 'Zone CPU' }
        '*THM1*' { return 'Zone GPU/PCH' }
        '*TZ0*'  { return 'Zone 0' }
        '*TZ1*'  { return 'Zone 1' }
        '*TZ2*'  { return 'Zone 2' }
        '*THRM*' { return 'Processeur (THRM)' }
        '*SKIN*' { return 'Boîtier' }
        '*AMB*'  { return 'Ambiance' }
        default  { $z = "Zone " + ($raw -replace '.*\\',''); return $z.Substring(0,[math]::Min(25,$z.Length)) }
    }
}
# Essaie d'abord Get-CimInstance (plus permissif sur certains systèmes)
$items = $null
try { $items = @(Get-CimInstance -Namespace 'root/wmi' -ClassName 'MSAcpi_ThermalZoneTemperature' -ErrorAction Stop) } catch {}
if (-not $items) {
    try { $items = @(Get-WmiObject -Namespace 'root/wmi' -Class MSAcpi_ThermalZoneTemperature -ErrorAction Stop) } catch {}
}
if ($items) {
    foreach ($z in $items) {
        $c = [math]::Round(($z.CurrentTemperature - 2732) / 10.0, 1)
        if ($c -gt 5 -and $c -lt 125) {
            $lbl = Label-Zone ($z.InstanceName)
            $res += [PSCustomObject]@{ name=$lbl; value=[double]$c }
        }
    }
}
@($res) | ConvertTo-Json -Compress -Depth 2
"#;
    let out = ps(script).unwrap_or_default();
    if out.trim().is_empty() || out.trim() == "null" { return vec![]; }
    let arr: Vec<serde_json::Value> = serde_json::from_str(out.trim()).unwrap_or_default();
    arr.iter().filter_map(|v| {
        let value = v["value"].as_f64().unwrap_or(0.0);
        if value == 0.0 { return None; }
        Some(SensorReading {
            hardware:      ACPI_HARDWARE.to_string(),
            hardware_type: "Carte mère".to_string(),
            name:          v["name"].as_str().unwrap_or("Zone thermique").to_string(),
            sensor_type:   "Temperature".to_string(),
            value,
            unit:          "°C".to_string(),
            min: 0.0, max: 0.0,
            source:        "ACPI".to_string(),
        })
    }).collect()
}

/// Température CPU via WMI thermal zones (plus fiable qu'ACPI sur certains systèmes)
fn query_wmi_cpu_thermal() -> Vec<SensorReading> {
    let script = r#"
$res = @()
# Approche 1 : Win32_PerfFormattedData via WMI
$items = $null
try { $items = @(Get-WmiObject -Namespace 'root/cimv2' -Class Win32_PerfFormattedData_Counters_ThermalZoneInformation -ErrorAction Stop) } catch {}
if (-not $items) {
    try { $items = @(Get-CimInstance -Namespace 'root/cimv2' -ClassName 'Win32_PerfFormattedData_Counters_ThermalZoneInformation' -ErrorAction Stop) } catch {}
}
if ($items) {
    foreach ($z in $items) {
        $hpt = if ($z.HighPrecisionTemperature) { [double]$z.HighPrecisionTemperature } else { 0.0 }
        if ($hpt -gt 0) {
            $temp = [math]::Round($hpt / 10.0 - 273.15, 1)
            if ($temp -gt 5 -and $temp -lt 120) {
                $name = switch -Wildcard ($z.Name) {
                    '*CPU*'    { 'Zone processeur (compteur)' }
                    '*Proc*'   { 'Zone processeur (compteur)' }
                    default    { "Zone $(($z.Name -replace '.*\\','' -replace '_',' ').Trim()) (compteur)" }
                }
                $res += [PSCustomObject]@{ name=$name; value=[double]$temp }
            }
        }
    }
}
@($res) | ConvertTo-Json -Compress -Depth 2
"#;
    let out = ps(script).unwrap_or_default();
    if out.trim().is_empty() || out.trim() == "null" { return vec![]; }
    let arr: Vec<serde_json::Value> = serde_json::from_str(out.trim()).unwrap_or_default();
    arr.iter().filter_map(|v| {
        let value = v["value"].as_f64().unwrap_or(0.0);
        if value == 0.0 { return None; }
        Some(SensorReading {
            hardware:      ACPI_HARDWARE.to_string(),
            hardware_type: "Carte mère".to_string(),
            name:          v["name"].as_str().unwrap_or("Zone thermique").to_string(),
            sensor_type:   "Temperature".to_string(),
            value,
            unit:          "°C".to_string(),
            min: 0.0, max: 0.0,
            source:        "WMI-Thermal".to_string(),
        })
    }).collect()
}

/// GPU intégré — utilisation + mémoire via Compteurs de performance Windows
/// Couvre Intel UHD/Iris, AMD Radeon intégré, sans LHM
fn query_igpu_perf() -> Vec<SensorReading> {
    let script = format!("{}{}", crate::utils::ps::LOC_COUNTER_PRELUDE, r#"
$res = @()
try {
    # Utilisation GPU par adaptateur (chemins de compteurs traduits pour Windows FR)
    $pMem = "\{0}(*)\{1}" -f (Loc-Counter 'GPU Adapter Memory'), (Loc-Counter 'Dedicated Usage')
    $gpuCounters = Get-Counter $pMem -ErrorAction SilentlyContinue
    if ($gpuCounters) {
        foreach ($sample in $gpuCounters.CounterSamples) {
            $adapter = ($sample.InstanceName -replace '.*luid_.*?_.*?_phys_.*?_eng_.*?_engt.*','GPU').Trim()
            if ($adapter -eq '' ) { $adapter = 'GPU' }
            $mb = [math]::Round($sample.CookedValue / 1MB, 0)
            if ($mb -gt 0) {
                $res += [PSCustomObject]@{ hardware=$adapter; hardware_type='GPU Intel'; name='VRAM utilisée'; sensor_type='Data'; value=[double]$mb; unit='MB' }
            }
        }
    }
    # Utilisation GPU engine 3D
    $pUtil = "\{0}(*engtype_3D*)\{1}" -f (Loc-Counter 'GPU Engine'), (Loc-Counter 'Utilization Percentage')
    $util = Get-Counter $pUtil -ErrorAction SilentlyContinue
    if ($util) {
        $total = ($util.CounterSamples | Measure-Object CookedValue -Sum).Sum
        $pct = [math]::Round([math]::Min($total, 100), 1)
        $res += [PSCustomObject]@{ hardware='GPU intégré'; hardware_type='GPU Intel'; name='Utilisation 3D'; sensor_type='Load'; value=[double]$pct; unit='%' }
    }
} catch {}
@($res) | ConvertTo-Json -Compress -Depth 2
"#);
    let out = ps(&script).unwrap_or_default();
    if out.trim().is_empty() || out.trim() == "null" { return vec![]; }
    let arr: Vec<serde_json::Value> = serde_json::from_str(out.trim()).unwrap_or_default();
    arr.iter().filter_map(|v| {
        let value = v["value"].as_f64().unwrap_or(0.0);
        if value == 0.0 { return None; }
        let st = v["sensor_type"].as_str().unwrap_or("Load").to_string();
        Some(SensorReading {
            hardware:      v["hardware"].as_str().unwrap_or("GPU intégré").to_string(),
            hardware_type: v["hardware_type"].as_str().unwrap_or("GPU Intel").to_string(),
            name:          v["name"].as_str().unwrap_or("Utilisation").to_string(),
            sensor_type:   st.clone(),
            value,
            unit:          v["unit"].as_str().unwrap_or(unit_for(&st)).to_string(),
            min: 0.0, max: 0.0,
            source:        "PerfCounters".to_string(),
        })
    }).collect()
}

fn get_all_sensors_sync() -> Vec<SensorReading> {
    // 1. LHM — source la plus complète (CPU par cœur, iGPU temp, fans, voltages)
    let lhm = query_lhm();
    if !lhm.is_empty() { return lhm; }

    // 2. OHM — alternative LHM
    let ohm = query_ohm();
    if !ohm.is_empty() { return ohm; }

    // 3. Sources natives combinées — TOUJOURS toutes les sources
    let mut result = Vec::new();
    result.extend(query_nvidia_smi());       // GPU NVIDIA dédié
    result.extend(query_igpu_perf());        // iGPU Intel/AMD (performance counters)
    result.extend(query_storage_temps());    // Températures disques (SMART)
    result.extend(query_wmi_cpu_thermal());  // Temp CPU via WMI thermal
    result.extend(query_acpi_temps());       // Zones thermiques ACPI
    // Dédoublonnage par (hardware+name): on garde la valeur la plus précise
    result.sort_by(|a, b| a.hardware.cmp(&b.hardware).then(a.name.cmp(&b.name)));
    result.dedup_by(|a, b| a.hardware == b.hardware && a.name == b.name);
    result
}

#[tauri::command]
pub async fn get_all_sensors() -> Result<Vec<SensorReading>, String> {
    tokio::task::spawn_blocking(get_all_sensors_sync)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_for_known_types() {
        assert_eq!(unit_for("Temperature"), "°C");
        assert_eq!(unit_for("Fan"), "RPM");
        assert_eq!(unit_for("Voltage"), "V");
        assert_eq!(unit_for("Load"), "%");
        assert_eq!(unit_for("Clock"), "MHz");
        assert_eq!(unit_for("Power"), "W");
        assert_eq!(unit_for("Data"), "GB");
        assert_eq!(unit_for("SmallData"), "MB");
    }

    #[test]
    fn unit_for_unknown_returns_empty() {
        assert_eq!(unit_for("Unknown"), "");
        assert_eq!(unit_for(""), "");
    }

    #[test]
    fn hw_type_label_cpu_and_gpu() {
        assert_eq!(hw_type_label("CPU"), "CPU");
        assert_eq!(hw_type_label("GpuNvidia"), "GPU NVIDIA");
        assert_eq!(hw_type_label("GpuAmd"), "GPU AMD");
        assert_eq!(hw_type_label("GpuIntel"), "GPU Intel");
    }

    #[test]
    fn hw_type_label_storage() {
        assert_eq!(hw_type_label("HDD"), "Stockage");
        assert_eq!(hw_type_label("SSD"), "Stockage");
        assert_eq!(hw_type_label("NVMe"), "Stockage NVMe");
    }

    #[test]
    fn tjmax_distance_is_not_a_temperature() {
        let (name, st) = reclassify_sensor("CPU Core #1 Distance to TjMax", "Temperature");
        assert_eq!(st, "TjMaxMargin");
        assert_eq!(name, "CPU Core #1 : marge avant TjMax");
        assert_eq!(unit_for(&st), "°C");
        let (name, st) = reclassify_sensor("Distance to TjMax", "Temperature");
        assert_eq!((name.as_str(), st.as_str()), ("Marge avant TjMax", "TjMaxMargin"));
    }

    #[test]
    fn real_temperatures_untouched() {
        for n in ["CPU Package", "Core Max", "CPU Core #3", "GPU Hot Spot"] {
            assert_eq!(reclassify_sensor(n, "Temperature"), (n.to_string(), "Temperature".to_string()));
        }
        assert_eq!(reclassify_sensor("TjMax", "Load").1, "Load");
    }

    #[test]
    fn lhm_row_tjmax_reclassified_and_zero_filtered() {
        let row = serde_json::json!({"hardware":"Intel Core i7","hardware_type":"Cpu","name":"CPU Core #2 Distance to TjMax","sensor_type":"Temperature","value":42.0,"min":30.0,"max":60.0});
        let r = parse_hw_monitor_row(&row, "LibreHardwareMonitor").expect("lecture valide");
        assert_eq!(r.sensor_type, "TjMaxMargin");
        assert_eq!(r.hardware_type, "CPU");
        let zero = serde_json::json!({"name":"Fan #1","sensor_type":"Fan","value":0.0});
        assert!(parse_hw_monitor_row(&zero, "LibreHardwareMonitor").is_none());
        let volt = serde_json::json!({"name":"Vcore","sensor_type":"Voltage","value":0.0});
        assert!(parse_hw_monitor_row(&volt, "LibreHardwareMonitor").is_some());
    }

    #[test]
    fn hw_type_label_unknown_returns_autre() {
        assert_eq!(hw_type_label("SomeFutureSensor"), "Autre");
        assert_eq!(hw_type_label(""), "Autre");
    }
}
