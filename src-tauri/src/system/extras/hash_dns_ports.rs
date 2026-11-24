use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use super::{parse_json_arr, ps, ps_ok, ps_with_args};

// ─── Hash Fichier ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct HashResult {
    pub path: String,
    pub algorithm: String,
    pub hash: String,
    pub size_bytes: u64,
}

// Anti-freeze : Get-FileHash est bloquant (fichiers volumineux) — jamais
// inline sur le thread de commande.
#[tauri::command]
pub async fn hash_file(path: String, algorithm: String) -> Result<HashResult, String> {
    tokio::task::spawn_blocking(move || hash_file_blocking(path, algorithm))
        .await
        .map_err(|e| e.to_string())?
}

/// Délai maximal d'un hachage : un fichier de 100 Go sur disque USB 2.0 reste
/// sous l'heure ; au-delà le processus PowerShell est tué (pas de gel infini).
const HASH_TIMEOUT_SECS: u64 = 3600;

fn hash_algo(algorithm: &str) -> &'static str {
    match algorithm.to_uppercase().as_str() {
        "MD5" => "MD5",
        "SHA1" => "SHA1",
        _ => "SHA256",
    }
}

fn hash_file_blocking(path: String, algorithm: String) -> Result<HashResult, String> {
    let algo = hash_algo(&algorithm);
    let meta = std::fs::metadata(&path).map_err(|e| format!("Fichier inaccessible : {}", e))?;
    if !meta.is_file() {
        return Err("Le chemin n'est pas un fichier".to_string());
    }
    // Chemin passé via $args[0] (arg processus séparé) — aucune interpolation PS possible
    let script = format!(
        "(Get-FileHash -LiteralPath $args[0] -Algorithm {} -ErrorAction Stop).Hash",
        algo
    );
    let r = ps_with_args(&script, &[&path], HASH_TIMEOUT_SECS)?;
    let hash = r.stdout.trim().to_string();
    // Fichier verrouillé (pagefile, VHDX monté) : Get-FileHash n'écrit rien sur
    // stdout et l'erreur part sur stderr — l'ancien code renvoyait Ok avec un
    // hash vide, affiché comme un résultat.
    if hash.is_empty() || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        let err = r.stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("hachage impossible").trim().to_string();
        return Err(format!("Hachage impossible : {}", err));
    }
    Ok(HashResult { path, algorithm: algo.to_string(), hash, size_bytes: meta.len() })
}

// Anti-freeze : hachage d'un dossier entier (jusqu'à 500 fichiers) est
// bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn hash_folder(path: String, algorithm: String, max_files: u32) -> Result<Vec<HashResult>, String> {
    tokio::task::spawn_blocking(move || hash_folder_blocking(path, algorithm, max_files))
        .await
        .map_err(|e| e.to_string())?
}

fn hash_folder_blocking(path: String, algorithm: String, max_files: u32) -> Result<Vec<HashResult>, String> {
    let algo = hash_algo(&algorithm);
    let n = max_files.min(500);
    // Chemin passé via $args[0] (arg processus séparé) — aucune interpolation PS possible
    let script = format!(r#"Get-ChildItem -LiteralPath $args[0] -File -ErrorAction SilentlyContinue | Select-Object -First {n} | ForEach-Object {{
    $h = (Get-FileHash -LiteralPath $_.FullName -Algorithm {algo} -ErrorAction SilentlyContinue).Hash
    if ($h) {{ [PSCustomObject]@{{ path=$_.FullName; size=$_.Length; hash=$h }} }}
}} | ConvertTo-Json -Compress"#,
        n = n, algo = algo);
    // -LiteralPath : sans lui, un nom contenant [ ] (« photo [1].jpg ») était lu
    // comme un motif générique et ignoré en silence.
    let out = ps_with_args(&script, &[&path], HASH_TIMEOUT_SECS)?.stdout;
    let out = out.trim();
    if out.is_empty() { return Ok(vec![]); }
    let arr: Vec<serde_json::Value> = parse_json_arr(out);
    Ok(arr.iter().filter_map(|v| {
        let hash = v["hash"].as_str().unwrap_or("").to_string();
        if hash.is_empty() { return None; }
        Some(HashResult {
            path:       v["path"].as_str().unwrap_or("").to_string(),
            algorithm:  algo.to_string(),
            hash,
            size_bytes: v["size"].as_u64().unwrap_or(0),
        })
    }).collect())
}

// ─── DNS Switcher ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct DnsPreset {
    pub id: String,
    pub name: String,
    pub primary: String,
    pub secondary: String,
    /// Adresses IPv6 du même fournisseur. Sans elles, les box (Free, Orange,
    /// SFR, Bouygues) continuent de distribuer leurs DNS IPv6 (RA/DHCPv6) et
    /// Windows les interroge aussi : un DNS filtrant (Family, AdGuard, Quad9)
    /// était contourné sur tout réseau IPv6.
    pub primary6: String,
    pub secondary6: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct NetworkAdapterInfo {
    pub name: String,
    pub description: String,
    pub status: String,
    pub current_dns: Vec<String>,
}

fn preset(id: &str, name: &str, ips: [&str; 4], description: &str) -> DnsPreset {
    DnsPreset {
        id: id.into(), name: name.into(),
        primary: ips[0].into(), secondary: ips[1].into(),
        primary6: ips[2].into(), secondary6: ips[3].into(),
        description: description.into(),
    }
}

// Adresses publiques officielles des fournisseurs (revues le 2026-11-23).
#[tauri::command]
pub fn get_dns_presets() -> Vec<DnsPreset> {
    vec![
        preset("google", "Google", ["8.8.8.8", "8.8.4.4", "2001:4860:4860::8888", "2001:4860:4860::8844"], "Rapide, fiable, mondial"),
        preset("cloudflare", "Cloudflare", ["1.1.1.1", "1.0.0.1", "2606:4700:4700::1111", "2606:4700:4700::1001"], "Très rapide, respect de la vie privée"),
        preset("cloudflare-doh", "Cloudflare Family", ["1.1.1.3", "1.0.0.3", "2606:4700:4700::1113", "2606:4700:4700::1003"], "Filtrage malwares + contenus adultes"),
        preset("opendns", "OpenDNS", ["208.67.222.222", "208.67.220.220", "2620:119:35::35", "2620:119:53::53"], "Filtrage contenus, Cisco"),
        preset("nextdns", "NextDNS", ["45.90.28.0", "45.90.30.0", "2a07:a8c0::", "2a07:a8c1::"], "Sans compte : résolveur simple ; filtrage après liaison de l'IP sur nextdns.io"),
        preset("quad9", "Quad9", ["9.9.9.9", "149.112.112.112", "2620:fe::fe", "2620:fe::9"], "Sécurisé, bloque les malwares (Suisse)"),
        preset("adguard", "AdGuard", ["94.140.14.14", "94.140.15.15", "2a10:50c0::ad1:ff", "2a10:50c0::ad2:ff"], "Bloque pubs et traqueurs"),
        preset("auto", "Automatique (DHCP)", ["", "", "", ""], "Remettre le DNS automatique (IPv4 et IPv6)"),
    ]
}

// Anti-freeze : PowerShell est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn get_network_adapters_for_dns() -> Result<Vec<NetworkAdapterInfo>, String> {
    tokio::task::spawn_blocking(get_network_adapters_for_dns_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn get_network_adapters_for_dns_blocking() -> Result<Vec<NetworkAdapterInfo>, String> {
    let script = r#"
Get-NetAdapter | Where-Object {$_.Status -eq 'Up'} | ForEach-Object {
    # IPv4 + IPv6 ; fec0:0:0:ffff::1..3 = valeurs par défaut Windows sans DNS IPv6, pas de vrais serveurs
    $dns = (Get-DnsClientServerAddress -InterfaceAlias $_.Name -ErrorAction SilentlyContinue).ServerAddresses | Where-Object { $_ -notlike 'fec0:0:0:ffff::*' }
    [PSCustomObject]@{
        name = $_.Name
        description = $_.InterfaceDescription
        status = $_.Status
        dns = if($dns){$dns -join ','}else{''}
    }
} | ConvertTo-Json -Compress
"#;
    let out = ps(script)?;
    if out.is_empty() { return Ok(vec![]); }
    let arr: Vec<serde_json::Value> = if out.starts_with('[') {
        serde_json::from_str(&out).unwrap_or_default()
    } else {
        serde_json::from_str(&format!("[{}]", out)).unwrap_or_default()
    };
    Ok(arr.iter().map(|v| NetworkAdapterInfo {
        name: v["name"].as_str().unwrap_or("").to_string(),
        description: v["description"].as_str().unwrap_or("").to_string(),
        status: v["status"].as_str().unwrap_or("").to_string(),
        current_dns: v["dns"].as_str().unwrap_or("").split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
    }).collect())
}

// Anti-freeze : PowerShell est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn switch_dns(adapter: String, primary: String, secondary: String, ipv6: Option<Vec<String>>) -> Result<String, String> {
    tokio::task::spawn_blocking(move || switch_dns_blocking(adapter, primary, secondary, ipv6.unwrap_or_default()))
        .await
        .map_err(|e| e.to_string())?
}

/// Valide chaque adresse (IpAddr) et la rend sous forme canonique : ensuite
/// elle ne contient que [0-9a-f.:], donc sûre dans une chaîne PS simple-quote.
fn canonical_ips(list: &[&str], label: &str) -> Result<Vec<String>, String> {
    list.iter().map(|s| s.trim()).filter(|s| !s.is_empty())
        .map(|s| s.parse::<std::net::IpAddr>().map(|ip| ip.to_string()).map_err(|_| format!("IP DNS {} invalide : {}", label, s)))
        .collect()
}

fn ps_list(ips: &[String]) -> String {
    format!("@({})", ips.iter().map(|i| format!("'{}'", i)).collect::<Vec<_>>().join(","))
}

fn switch_dns_script(main: &[String], v6: &[String]) -> String {
    if main.is_empty() {
        return "try { Set-DnsClientServerAddress -InterfaceAlias $args[0] -ResetServerAddresses -ErrorAction Stop; 'OK' } catch { \"ERR:$($_.Exception.Message)\" }".to_string();
    }
    // Liste combinée IPv4+IPv6 d'abord ; si l'adaptateur la refuse (IPv6
    // désactivé), repli IPv4 seul signalé par OK4 plutôt qu'un échec complet.
    format!(
        "$m = {m}; $v6 = {v6}; try {{ Set-DnsClientServerAddress -InterfaceAlias $args[0] -ServerAddresses ($m + $v6) -ErrorAction Stop; 'OK' }} catch {{ $e = $_.Exception.Message; if ($v6.Count) {{ try {{ Set-DnsClientServerAddress -InterfaceAlias $args[0] -ServerAddresses $m -ErrorAction Stop; 'OK4' }} catch {{ \"ERR:$($_.Exception.Message)\" }} }} else {{ \"ERR:$e\" }} }}",
        m = ps_list(main), v6 = ps_list(v6),
    )
}

fn switch_dns_blocking(adapter: String, primary: String, secondary: String, ipv6: Vec<String>) -> Result<String, String> {
    let main = canonical_ips(&[&primary, &secondary], "primaire/secondaire")?;
    let v6 = canonical_ips(&ipv6.iter().map(String::as_str).collect::<Vec<_>>(), "IPv6")?;
    if main.is_empty() && !v6.is_empty() {
        return Err("Serveur DNS primaire requis".to_string());
    }
    // Adaptateur passé via $args[0] (argument de processus séparé) : aucune
    // injection possible par son nom. -ErrorAction Stop + try/catch : sans ça
    // un échec non terminant laissait 'OK' s'imprimer.
    let r = ps_with_args(&switch_dns_script(&main, &v6), &[&adapter], 60)?;
    let t = r.stdout.trim();
    if t.starts_with("OK4") {
        Ok("Serveurs DNS IPv4 mis à jour (IPv6 refusé par l'adaptateur : les DNS IPv6 du réseau restent actifs)".to_string())
    } else if t.starts_with("OK") {
        Ok("Serveurs DNS mis à jour".to_string())
    } else if t.is_empty() {
        Err(r.stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("échec PowerShell sans message").trim().to_string())
    } else {
        Err(t.trim_start_matches("ERR:").trim().to_string())
    }
}

// Anti-freeze : PowerShell est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn flush_dns_cache() -> Result<String, String> {
    tokio::task::spawn_blocking(flush_dns_cache_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn flush_dns_cache_blocking() -> Result<String, String> {
    // ps_ok() + -ErrorAction Stop (pas ps() seule) : les erreurs de cmdlet
    // PowerShell sont non-terminantes par defaut — verifie en direct qu'un
    // Get-Item sur un chemin absent laisse la suite du script s'executer
    // normalement (exit code 0 malgre l'erreur). Sans -ErrorAction Stop, un
    // Clear-DnsClientCache qui echoue (droits insuffisants, service DNS
    // Client arrete) laissait quand meme s'imprimer 'DNS cache vide avec
    // succes' — meme piege que update_defender_signatures_blocking (cycle 69).
    ps_ok("Clear-DnsClientCache -ErrorAction Stop; 'DNS cache vidé avec succès'")
}

// Anti-freeze : Test-Connection est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn ping_dns(ip: String) -> Result<i32, String> {
    tokio::task::spawn_blocking(move || ping_dns_blocking(ip))
        .await
        .map_err(|e| e.to_string())?
}

fn ping_dns_blocking(ip: String) -> Result<i32, String> {
    // Valider comme IpAddr — après validation, ne contient que [0-9.:/] donc sûr à embarquer
    ip.parse::<std::net::IpAddr>().map_err(|_| format!("Adresse IP invalide : {}", ip))?;
    let script = format!(
        "try {{ $r = Test-Connection -ComputerName '{}' -Count 2 -ErrorAction Stop; [int]($r | Measure-Object -Property ResponseTime -Average).Average }} catch {{ -1 }}",
        ip
    );
    let out = ps(&script)?;
    Ok(out.trim().parse::<i32>().unwrap_or(-1))
}

// ─── Port Scanner Local ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct OpenPort {
    pub protocol: String,
    pub local_address: String,
    pub local_port: u16,
    pub remote_address: String,
    pub state: String,
    pub pid: u32,
    pub process_name: String,
}

// Anti-freeze : PowerShell/netstat est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn get_local_ports() -> Result<Vec<OpenPort>, String> {
    tokio::task::spawn_blocking(get_local_ports_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn get_local_ports_blocking() -> Result<Vec<OpenPort>, String> {
    let script = r#"
$procs = @{}
Get-Process | ForEach-Object { $procs[$_.Id] = $_.ProcessName }
$conns = @()
netstat -ano | Select-String -Pattern '^\s+(TCP|UDP)\s+(\S+)\s+(\S+)\s+(\w+|\s+)\s+(\d+)' | ForEach-Object {
    $m = $_.Matches[0]
    if ($m) {
        $parts = $_.Line.Trim() -split '\s+'
        if ($parts.Count -ge 4) {
            $proto = $parts[0]
            $local = $parts[1]
            $remote = $parts[2]
            $stateOrPid = $parts[3]
            $pidNum = if($proto -eq 'TCP' -and $parts.Count -ge 5){[int]$parts[4]}else{[int]$stateOrPid}
            $state = if($proto -eq 'TCP' -and $parts.Count -ge 5){$stateOrPid}else{''}
            $lp = if($local -match ':(\d+)$'){[int]$Matches[1]}else{0}
            $conns += [PSCustomObject]@{
                protocol=$proto; local=$local; remote=$remote
                state=$state; pid=$pidNum; port=$lp
                proc=if($procs.ContainsKey($pidNum)){$procs[$pidNum]}else{''}
            }
        }
    }
}
$conns | ConvertTo-Json -Compress
"#;
    let out = ps(script)?;
    if out.is_empty() { return Ok(vec![]); }
    let arr: Vec<serde_json::Value> = parse_json_arr(&out);
    let mut ports: Vec<OpenPort> = arr.iter().filter_map(|v| {
        let port = v["port"].as_u64().unwrap_or(0) as u16;
        if port == 0 { return None; }
        Some(OpenPort {
            protocol: v["protocol"].as_str().unwrap_or("").to_string(),
            local_address: v["local"].as_str().unwrap_or("").to_string(),
            local_port: port,
            remote_address: v["remote"].as_str().unwrap_or("").to_string(),
            state: v["state"].as_str().unwrap_or("").to_string(),
            pid: v["pid"].as_u64().unwrap_or(0) as u32,
            process_name: v["proc"].as_str().unwrap_or("").to_string(),
        })
    }).collect();
    // Tri (port, protocole) : dedup_by_key ne retire que les doublons CONSÉCUTIFS ;
    // trié par port seul, TCP/UDP alternés sur un même port restaient en double.
    ports.sort_by(|a, b| (a.local_port, &a.protocol).cmp(&(b.local_port, &b.protocol)));
    ports.dedup_by_key(|p| (p.protocol.clone(), p.local_port));
    Ok(ports)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_presets_has_expected_count() {
        let presets = get_dns_presets();
        assert_eq!(presets.len(), 8, "Expected 8 DNS presets, got {}", presets.len());
    }

    #[test]
    fn dns_presets_cloudflare_ips() {
        let presets = get_dns_presets();
        let cf = presets.iter().find(|p| p.id == "cloudflare").expect("cloudflare preset missing");
        assert_eq!(cf.primary, "1.1.1.1");
        assert_eq!(cf.secondary, "1.0.0.1");
    }

    #[test]
    fn dns_presets_google_ips() {
        let presets = get_dns_presets();
        let g = presets.iter().find(|p| p.id == "google").expect("google preset missing");
        assert_eq!(g.primary, "8.8.8.8");
        assert_eq!(g.secondary, "8.8.4.4");
    }

    #[test]
    fn dns_presets_auto_has_empty_ips() {
        let presets = get_dns_presets();
        let auto = presets.iter().find(|p| p.id == "auto").expect("auto preset missing");
        assert_eq!(auto.primary, "");
        assert_eq!(auto.secondary, "");
    }

    #[test]
    fn dns_presets_all_have_ids_and_names() {
        let presets = get_dns_presets();
        for p in &presets {
            assert!(!p.id.is_empty(), "Preset has empty id");
            assert!(!p.name.is_empty(), "Preset '{}' has empty name", p.id);
        }
    }

    #[test]
    fn dns_presets_all_ips_valid_and_ipv6_family() {
        for p in get_dns_presets().iter().filter(|p| p.id != "auto") {
            for ip in [&p.primary, &p.secondary] {
                assert!(ip.parse::<std::net::Ipv4Addr>().is_ok(), "{} : {}", p.id, ip);
            }
            for ip in [&p.primary6, &p.secondary6] {
                assert!(ip.parse::<std::net::Ipv6Addr>().is_ok(), "{} : {}", p.id, ip);
            }
        }
    }

    #[test]
    fn canonical_ips_validates_and_normalizes() {
        assert_eq!(canonical_ips(&["2606:4700:4700:0::1111", " 1.1.1.1 ", ""], "x").unwrap(), vec!["2606:4700:4700::1111", "1.1.1.1"]);
        assert!(canonical_ips(&["1.1.1.1'; Remove-Item C:\\ -Recurse; '"], "x").is_err());
        assert!(canonical_ips(&["8.8.8"], "x").is_err());
    }

    #[test]
    fn switch_dns_script_shapes() {
        let reset = switch_dns_script(&[], &[]);
        assert!(reset.contains("-ResetServerAddresses"));
        let s = switch_dns_script(&["1.1.1.1".into()], &["2606:4700:4700::1111".into()]);
        assert!(s.contains("$m = @('1.1.1.1'); $v6 = @('2606:4700:4700::1111')"));
        assert!(s.contains("'OK4'"), "repli IPv4 seul attendu");
        let s = switch_dns_script(&["1.1.1.1".into()], &[]);
        assert!(s.contains("$v6 = @()"));
    }

    #[test]
    fn dns_presets_no_duplicate_ids() {
        let presets = get_dns_presets();
        let mut ids: Vec<&str> = presets.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), presets.len(), "Duplicate DNS preset IDs found");
    }
}
