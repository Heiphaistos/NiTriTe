use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct PerfPoint {
    pub timestamp: String,
    pub cpu_percent: f64,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub disk_read_mbps: f64,
    pub disk_write_mbps: f64,
    pub net_recv_mbps: f64,
    pub net_send_mbps: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PerfHistory {
    pub points: Vec<PerfPoint>,
    pub sample_interval_secs: u32,
    pub duration_secs: u32,
    pub avg_cpu: f64,
    pub peak_cpu: f64,
    pub avg_ram_mb: u64,
    pub peak_ram_mb: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TopProcess {
    pub name: String,
    pub pid: u32,
    pub cpu_percent: f64,
    pub ram_mb: u64,
    pub disk_mbps: f64,
}

/// Historique de performances — non-bloquant via spawn_blocking
#[tauri::command]
pub async fn get_perf_history(samples: u32, interval_secs: u32) -> PerfHistory {
    let n = samples.clamp(5, 120);
    let interval = interval_secs.clamp(1, 60);

    let ps = format!(
        r#"
$n = {n}
$iv = {interval}
$points = @()

for ($i = 0; $i -lt $n; $i++) {{
    $ts = (Get-Date).ToString('HH:mm:ss')
    $cpu = try {{
        [math]::Round((Get-WmiObject Win32_Processor -EA Stop | Measure-Object LoadPercentage -Average).Average, 1)
    }} catch {{ 0 }}
    $os = Get-WmiObject Win32_OperatingSystem -EA SilentlyContinue
    $ramUsed  = if($os) {{ [math]::Round(($os.TotalVisibleMemorySize - $os.FreePhysicalMemory)/1024, 0) }} else {{ 0 }}
    $ramTotal = if($os) {{ [math]::Round($os.TotalVisibleMemorySize/1024, 0) }} else {{ 0 }}

    $diskRead = 0; $diskWrite = 0
    try {{
        # Classes WMI perf : noms de propriétés INVARIANTS. Get-Counter utilise
        # des chemins de compteurs LOCALISÉS (échec silencieux sur Windows FR →
        # I/O toujours à 0), d'où le passage à WMI.
        $d = Get-WmiObject Win32_PerfFormattedData_PerfDisk_PhysicalDisk -EA Stop | Where-Object {{ $_.Name -eq '_Total' }} | Select-Object -First 1
        if ($d) {{
            $diskRead  = [math]::Round([double]$d.DiskReadBytesPersec / 1MB, 2)
            $diskWrite = [math]::Round([double]$d.DiskWriteBytesPersec / 1MB, 2)
        }}
    }} catch {{}}

    $netRecv = 0; $netSend = 0
    try {{
        $nets = Get-WmiObject Win32_PerfFormattedData_Tcpip_NetworkInterface -EA Stop
        $netRecv = [math]::Round(($nets | Measure-Object BytesReceivedPersec -Sum).Sum / 1MB, 3)
        $netSend = [math]::Round(($nets | Measure-Object BytesSentPersec -Sum).Sum / 1MB, 3)
    }} catch {{}}

    # PSCustomObject (pas hashtable) : Measure-Object -Property cpu/ramUsed plus
    # bas ne voit PAS les clés d'une hashtable → avgCpu/peakCpu/avgRam/peakRam
    # sortiraient tous à 0. La sérialisation JSON reste identique.
    $points += [PSCustomObject]@{{
        ts        = $ts
        cpu       = [double]$cpu
        ramUsed   = [long]$ramUsed
        ramTotal  = [long]$ramTotal
        diskRead  = [double]$diskRead
        diskWrite = [double]$diskWrite
        netRecv   = [double]$netRecv
        netSend   = [double]$netSend
    }}
    if ($i -lt $n - 1) {{ Start-Sleep -Seconds $iv }}
}}

$avgCpu  = if($points.Count) {{ [math]::Round(($points | Measure-Object -Property cpu -Average).Average, 1) }} else {{ 0 }}
$peakCpu = if($points.Count) {{ [math]::Round(($points | Measure-Object -Property cpu -Maximum).Maximum, 1) }} else {{ 0 }}
$avgRam  = if($points.Count) {{ [long]($points | Measure-Object -Property ramUsed -Average).Average }} else {{ 0 }}
$peakRam = if($points.Count) {{ [long]($points | Measure-Object -Property ramUsed -Maximum).Maximum }} else {{ 0 }}

@{{
    points   = $points
    interval = $iv
    duration = $n * $iv
    avgCpu   = $avgCpu
    peakCpu  = $peakCpu
    avgRam   = $avgRam
    peakRam  = $peakRam
}} | ConvertTo-Json -Depth 4 -Compress
"#,
        n = n,
        interval = interval
    );

    // Durée prévue + 10 s de marge WMI par échantillon : `.output()` brut
    // (version d'origine) attendait sans fin si WMI se figeait.
    let timeout = (n as u64) * (interval as u64 + 10) + 30;
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            if let Ok(t) = crate::utils::ps::ps_with_timeout(&ps, timeout) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(t.trim()) {
                    let points = v["points"].as_array().map(|arr| {
                        arr.iter().map(|p| PerfPoint {
                            timestamp: p["ts"].as_str().unwrap_or("").to_string(),
                            cpu_percent: p["cpu"].as_f64().unwrap_or(0.0),
                            ram_used_mb: p["ramUsed"].as_u64().unwrap_or(0),
                            ram_total_mb: p["ramTotal"].as_u64().unwrap_or(0),
                            disk_read_mbps: p["diskRead"].as_f64().unwrap_or(0.0),
                            disk_write_mbps: p["diskWrite"].as_f64().unwrap_or(0.0),
                            net_recv_mbps: p["netRecv"].as_f64().unwrap_or(0.0),
                            net_send_mbps: p["netSend"].as_f64().unwrap_or(0.0),
                        }).collect()
                    }).unwrap_or_default();

                    return PerfHistory {
                        points,
                        sample_interval_secs: v["interval"].as_u64().unwrap_or(interval as u64) as u32,
                        duration_secs: v["duration"].as_u64().unwrap_or(0) as u32,
                        avg_cpu: v["avgCpu"].as_f64().unwrap_or(0.0),
                        peak_cpu: v["peakCpu"].as_f64().unwrap_or(0.0),
                        avg_ram_mb: v["avgRam"].as_u64().unwrap_or(0),
                        peak_ram_mb: v["peakRam"].as_u64().unwrap_or(0),
                    };
                }
            }
        }
        PerfHistory::default()
    })
    .await
    .unwrap_or_default()
}

/// Top processus par CPU — non-bloquant
#[tauri::command]
pub async fn get_top_processes_by_cpu(limit: u32) -> Vec<TopProcess> {
    let n = limit.clamp(5, 50);
    let ps = format!(
        r#"
# $_.CPU = temps processeur CUMULÉ (secondes) depuis le démarrage, pas un %.
# Pour un vrai % instantané : deux relevés de CPU espacés de 0,5 s, puis
# delta / intervalle / nb_cœurs * 100.
$ncores = [math]::Max(1, [int](Get-CimInstance Win32_ComputerSystem -EA SilentlyContinue).NumberOfLogicalProcessors)
$snap = @{{}}
foreach ($p in (Get-Process -EA SilentlyContinue)) {{ $snap[$p.Id] = [double]$p.CPU }}
Start-Sleep -Milliseconds 500
$results = foreach ($p in (Get-Process -EA SilentlyContinue)) {{
    $prev = $snap[$p.Id]
    $delta = if ($null -ne $prev -and $null -ne $p.CPU) {{ [double]$p.CPU - $prev }} else {{ 0 }}
    if ($delta -lt 0) {{ $delta = 0 }}
    $pct = [math]::Round(($delta / 0.5 / $ncores) * 100, 1)
    [PSCustomObject]@{{
        name = $p.ProcessName
        pid  = [int]$p.Id
        cpu  = $pct
        ram  = [long][math]::Round($p.WorkingSet64/1MB, 1)
        disk = 0.0
    }}
}}
@($results | Sort-Object cpu -Descending | Select-Object -First {n}) | ConvertTo-Json -Compress
"#,
        n = n
    );

    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            if let Ok(t) = crate::utils::ps::ps_with_timeout(&ps, 30) {
                let t = t.trim();
                let arr_t = if t.starts_with('{') { format!("[{}]", t) } else { t.to_string() };
                if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&arr_t) {
                    return arr.iter().map(|p| TopProcess {
                        name: p["name"].as_str().unwrap_or("").to_string(),
                        pid: p["pid"].as_u64().unwrap_or(0) as u32,
                        cpu_percent: p["cpu"].as_f64().unwrap_or(0.0),
                        ram_mb: p["ram"].as_u64().unwrap_or(0),
                        disk_mbps: p["disk"].as_f64().unwrap_or(0.0),
                    }).collect();
                }
            }
        }
        vec![]
    })
    .await
    .unwrap_or_default()
}
