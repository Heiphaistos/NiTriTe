use serde::Serialize;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize)]
pub struct InstalledApp {
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub uninstall_string: String,
    pub source: String,
    pub registry_path: String,
    pub install_size_kb: u64,
    pub install_date: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UninstallResult {
    pub app_name: String,
    pub success: bool,
    pub message: String,
    pub residuals_found: Vec<String>, // trouvés mais PAS encore supprimés
}

#[derive(Debug, Clone, Serialize)]
pub struct ResidualCleanResult {
    pub success: bool,
    pub deleted_count: usize,
    pub failed_count: usize,
    pub message: String,
}

/// Liste les applications installées via le registre Windows
pub fn list_installed_apps() -> Vec<InstalledApp> {
    let ps = r#"
$paths = @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
)
$apps = Get-ItemProperty $paths -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -and $_.UninstallString -and $_.DisplayName -notmatch '^KB\d+' } |
    ForEach-Object {
        $rp = ($_.PSPath -replace 'Microsoft\.PowerShell\.Core\\Registry::', '') -replace '^HKEY_LOCAL_MACHINE\\', 'HKLM:\' -replace '^HKEY_CURRENT_USER\\', 'HKCU:\'
        [PSCustomObject]@{
            DisplayName = $_.DisplayName
            DisplayVersion = $_.DisplayVersion
            Publisher = $_.Publisher
            UninstallString = $_.UninstallString
            EstimatedSize = $_.EstimatedSize
            InstallDate = $_.InstallDate
            RegistryPath = $rp
        }
    } |
    Sort-Object DisplayName
$apps | ConvertTo-Json -Compress -Depth 2
"#;

    // run_ps decode via decode_output : DisplayName/Publisher viennent du
    // registre et sont frequemment accentues (editeurs FR, composants Microsoft
    // localises), sortis en OEM cote PowerShell → mojibake avec from_utf8_lossy.
    let Some(text) = run_ps(ps, 60) else { return vec![] };
    let text = text.trim();
    if text.is_empty() || text == "null" { return vec![]; }
    let json_text = if text.starts_with('{') { format!("[{}]", text) } else { text.to_string() };
    let values: Vec<serde_json::Value> = match serde_json::from_str(&json_text) {
        Ok(v) => v, Err(_) => return vec![],
    };
    values.into_iter().filter_map(|v| {
        let name = v["DisplayName"].as_str()?.to_string();
        let version = v["DisplayVersion"].as_str().unwrap_or("").to_string();
        let publisher = v["Publisher"].as_str().unwrap_or("").to_string();
        let uninstall_string = v["UninstallString"].as_str()?.to_string();
        let raw_date = v["InstallDate"].as_str().unwrap_or("");
        let install_date = if raw_date.len() == 8 {
            format!("{}-{}-{}", &raw_date[..4], &raw_date[4..6], &raw_date[6..8])
        } else { raw_date.to_string() };
        Some(InstalledApp {
            name, version, publisher, uninstall_string,
            source: "registry".to_string(),
            registry_path: v["RegistryPath"].as_str().unwrap_or("").to_string(),
            install_size_kb: v["EstimatedSize"].as_u64().unwrap_or(0),
            install_date,
        })
    }).collect()
}

/// Désinstalle silencieusement + trouve les résidus (sans les supprimer)
pub fn uninstall_app_clean(
    app_name: String,
    uninstall_string: String,
    publisher: String,
    window: &tauri::Window,
) -> UninstallResult {
    emit(window, "start", &app_name, "Désinstallation silencieuse en cours...");

    let uninstall_ok = run_uninstall_silent(&app_name, &uninstall_string);

    emit(window, "scan", &app_name, "Recherche des résidus...");
    let residuals = find_residuals(&app_name, &publisher);

    emit(window, "done", &app_name, if uninstall_ok { "Désinstallé" } else { "Échec désinstallation" });

    UninstallResult {
        app_name: app_name.clone(),
        success: uninstall_ok,
        message: if uninstall_ok {
            format!("Désinstallé avec succès. {} résidu(s) détecté(s).", residuals.len())
        } else {
            format!("Échec : application toujours présente dans le registre. {} résidu(s) détecté(s).", residuals.len())
        },
        residuals_found: residuals,
    }
}

fn emit(window: &tauri::Window, step: &str, app: &str, msg: &str) {
    let _ = window.emit("uninstall-progress", serde_json::json!({
        "app": app, "step": step, "message": msg
    }));
}

// ── Désinstallation silencieuse — 100% natif, vérifié par registre ───────────
//
// On n'utilise PAS winget : trop imprévisible (peut sortir avant la fin,
// laisser la GUI ouverte, faux-positifs sur les codes de retour).
//
// Stratégie :
//  1. MSI  → msiexec /X{GUID} /qn /norestart
//  2. Inno → exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
//  3. NSIS / inconnu → exe /S
//     Avec /S, NSIS est VRAIMENT silencieux : pas de GUI, pas de processus enfant.
//  4. Start-Process -Wait : attend la VRAIE fin du processus
//  5. Vérification registre pour confirmer (pas de faux positif)
//
/// Desinstallation silencieuse par la commande du registre, verifiee (l'entree
/// doit avoir disparu). Utilisee aussi par la cascade de `smart_install`.
pub fn uninstall_silent(app_name: &str, uninstall_string: &str) -> bool {
    run_uninstall_silent(app_name, uninstall_string)
}

fn run_uninstall_silent(app_name: &str, uninstall_string: &str) -> bool {
    // ── Validation Rust de l'uninstall_string ────────────────────────────────
    // Refuse les strings qui ne correspondent pas à un désinstalleur légitime.
    // Bloque les commandes shell connues pour prévenir l'injection.
    {
        let us_lower = uninstall_string.trim().to_lowercase();
        // Préfixes autorisés : msiexec, ou chemin absolu Windows (C:\...), ou UNC (\\)
        let allowed = us_lower.starts_with("msiexec")
            || us_lower.starts_with("msiexec.exe")
            || (us_lower.len() >= 3 && us_lower.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && us_lower[1..].starts_with(":\\"))
            || us_lower.starts_with("\"")  // chemin entre guillemets, ex : "C:\Program Files\..."
            || us_lower.starts_with("\\\\"); // UNC path
        // Préfixes interdits explicitement (commandes shell)
        let denied_prefixes = ["cmd", "powershell", "wscript", "cscript", "rundll32", "regsvr32"];
        let is_denied = denied_prefixes.iter().any(|&d| {
            us_lower == d
                || us_lower.starts_with(&format!("{} ", d))
                || us_lower.starts_with(&format!("{}.exe", d))
        });
        if !allowed || is_denied {
            return false;
        }
    }

    // 15 min : au-dela, le desinstalleur attend une fenetre invisible.
    match run_ps(&uninstall_script(app_name, uninstall_string), 900) {
        Some(t) => t.trim().starts_with("OK:"),
        None    => false,
    }
}

/// Chaine PowerShell litterale entre apostrophes : rien n'y est interprete
/// (ni `$`, ni backtick, ni `"`). PowerShell traite aussi les apostrophes
/// typographiques (U+2018-U+201B) comme delimiteurs : elles sont doublees
/// comme `'`, sinon un nom comme « Assistant d’installation » fermait la chaine.
fn ps_lit(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for c in value.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            out.push(c);
        }
        out.push(c);
    }
    out.push('\'');
    out
}

/// Script de desinstallation silencieuse. Avant : les valeurs etaient
/// injectees en JSON brut (`"…\"C:\\…\"…" | ConvertFrom-Json`) — PowerShell
/// n'interprete pas `\"` : tout UninstallString entre guillemets (la majorite)
/// rendait le script syntaxiquement invalide, et `"Nom" | ConvertFrom-Json`
/// donnait `$null` pour le nom, donc `-like '**'` : la desinstallation propre
/// echouait a coup sur (verifie 2026-11-23 avec le parseur PowerShell).
fn uninstall_script(app_name: &str, uninstall_string: &str) -> String {
    format!(
        r#"
function Is-Installed($name) {{
    $paths = @(
        'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
        'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
    )
    # Egalite exacte : le nom vient du registre lui-meme. Un -like *nom*
    # declarait « toujours installee » une app desinstallee des qu'une autre
    # la contenait (« Git » / « GitHub Desktop ») — faux echec.
    $found = Get-ItemProperty $paths -ErrorAction SilentlyContinue |
        Where-Object {{ $_.DisplayName -and $_.DisplayName -eq $name }}
    return ($null -ne $found)
}}

# Valeurs en chaines litterales (ps_lit) — aucune interpretation possible
$appName = {app_lit}
$s = ({us_lit}).Trim()

$proc = $null
try {{
    if ($s -match 'msiexec' -or $s -match '\.msi"?$') {{
        # ── MSI ──────────────────────────────────────────────────────────────
        $re = '/[Ii](\{{[^}}]+\}})'
        $msiArgs = ($s -replace $re, '/X$1') -replace 'msiexec(\.exe)?\s*', ''
        # Sanitisation : retire les caractères pouvant injecter des commandes PS
        $msiArgs = $msiArgs -replace '[&|;`]|\$\(|\$\{{', ''
        if ($msiArgs -notmatch '/q') {{ $msiArgs = $msiArgs.Trim() + ' /qn /norestart' }}
        $proc = Start-Process -FilePath 'msiexec.exe' `
            -ArgumentList $msiArgs.Trim() -Wait -PassThru -ErrorAction Stop

    }} else {{
        # ── Non-MSI : extraire le chemin de l'exe ────────────────────────────
        if ($s -match '^"([^"]+)"') {{ $exe = $matches[1] }}
        else {{ $exe = ($s -split '\s+')[0].Trim('"') }}

        if (-not $exe -or -not (Test-Path -LiteralPath $exe)) {{
            Write-Output "FAIL:exe_not_found:$exe"; exit
        }}

        # Détecter le type via les métadonnées PE
        $vi   = (Get-Item -LiteralPath $exe -ErrorAction SilentlyContinue).VersionInfo
        $meta = "$($vi.FileDescription) $($vi.ProductName) $($vi.CompanyName)"

        $silentArgs = if ($meta -match 'Inno|Jordan Russell') {{
            @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-')
        }} else {{
            # NSIS (Nullsoft) ou inconnu → /S
            # /S = mode silencieux complet, PAS de GUI, PAS de processus enfant
            @('/S')
        }}

        # -Wait : bloque jusqu'à la VRAIE fin (fonctionne car /S empêche le spawn enfant)
        $proc = Start-Process -FilePath $exe `
            -ArgumentList $silentArgs -Wait -PassThru -ErrorAction Stop
    }}
}} catch {{
    Write-Output "FAIL:exception:$($_.Exception.Message)"; exit
}}

# Attendre que le registre soit mis à jour (flush)
Start-Sleep -Milliseconds 2500

$code = if ($proc) {{ $proc.ExitCode }} else {{ -1 }}
if (Is-Installed $appName) {{
    Write-Output "FAIL:still_installed:$code"
}} else {{
    Write-Output "OK:$code"
}}
"#,
        app_lit = ps_lit(app_name),
        us_lit  = ps_lit(uninstall_string)
    )
}

// ── Recherche des résidus (sans supprimer) ───────────────────────────────────

fn find_residuals(app_name: &str, publisher: &str) -> Vec<String> {
    preview_residuals(app_name.to_string(), publisher.to_string())
}

/// Prévisualise les résidus qu'une désinstallation propre supprimerait (sans rien effacer)
/// Mots-cles de recherche des residus. Exclut les mots generiques ET les noms
/// de « dossiers parapluie » partages par plusieurs produits : avant, une app
/// Microsoft (editeur « Microsoft Corporation ») donnait le mot-cle
/// « microsoft », qui faisait lister %APPDATA%\Microsoft et
/// %LOCALAPPDATA%\Microsoft entiers comme residus — un clic sur « Supprimer »
/// effacait profils Office/Teams/Edge, identifiants, etc. Les jetons sans
/// lettre (« 2015-2022 », « 2024 ») matchaient eux aussi n'importe quoi.
fn residual_keywords(app_name: &str, publisher: &str) -> Vec<String> {
    const GENERIC: &[&str] = &[
        "software", "corp", "corporation", "inc", "ltd", "llc", "gmbh",
        "technologies", "solutions", "systems", "group", "the", "and",
        "microsoft", "windows", "google", "apple", "update", "runtime",
        "redistributable", "edition", "version", "setup", "x64", "x86",
    ];
    // Vec::dedup ne retire que les doublons CONSÉCUTIFS ; app_name et publisher
    // partagent souvent un mot (« Adobe Photoshop » + « Adobe Inc »), non adjacent
    // ici → un HashSet garde l'unicité tout en préservant l'ordre d'apparition.
    let mut seen = std::collections::HashSet::new();
    let mut keywords: Vec<String> = Vec::new();
    for word in app_name.split_whitespace().chain(publisher.split_whitespace()) {
        let w = word.to_lowercase();
        let w = w.trim_matches(|c: char| !c.is_alphanumeric());
        if w.len() >= 4 && w.chars().any(char::is_alphabetic) && !GENERIC.contains(&w) && seen.insert(w.to_string()) {
            keywords.push(w.to_string());
        }
    }
    keywords.truncate(4);
    keywords
}

/// Dossiers partages par plusieurs produits ou par Windows : jamais un residu,
/// meme si leur nom contient un mot-cle (garde reprise par les deux gardes de
/// suppression ci-dessous).
const SHARED_DIRS: &[&str] = &[
    "microsoft", "google", "apple", "apple computer", "packages", "temp", "programs",
    "mozilla", "intel", "nvidia", "nvidia corporation", "amd", "adobe", "package cache",
    "windowsapps", "comms", "connecteddevicesplatform", "d3dscache", "crashdumps",
    "publishers", "oracle", "java", "jetbrains", "npm", "npm-cache", "pip",
];

pub fn preview_residuals(app_name: String, publisher: String) -> Vec<String> {
    let keywords = residual_keywords(&app_name, &publisher);
    if keywords.is_empty() { return vec![]; }
    let shared_json = serde_json::to_string(SHARED_DIRS).unwrap_or_else(|_| "[]".into());

    let kw_json = ps_lit(&serde_json::to_string(&keywords).unwrap_or_default());
    let ps = format!(r#"
$keywords = {} | ConvertFrom-Json
$shared = {} | ConvertFrom-Json
$found = @()

function Match-Kw($name) {{
    $n = $name.ToLower()
    # Echapper les métacaractères wildcard du mot-clé : un nom d'app à crochets
    # ferait matcher -like sur une classe de caractères au lieu du littéral.
    foreach ($kw in $keywords) {{
        $pat = '*' + [Management.Automation.WildcardPattern]::Escape($kw) + '*'
        if ($n -like $pat) {{ return $true }}
    }}
    return $false
}}

$fsPaths = @($env:APPDATA, $env:LOCALAPPDATA, "$env:LOCALAPPDATA\Programs", $env:ProgramData, "$env:LOCALAPPDATA\Temp")
foreach ($base in $fsPaths) {{
    if (-not $base -or -not (Test-Path $base)) {{ continue }}
    Get-ChildItem $base -ErrorAction SilentlyContinue | ForEach-Object {{
        if ((Match-Kw $_.Name) -and ($shared -notcontains $_.Name.ToLower())) {{
            $fp = $_.FullName.ToLower()
            # Protection : exclut chemins système critiques
            $isSys = ($fp -like "*\windows\*") -or
                     ($fp -like "*\system32\*") -or
                     ($fp -like "*\syswow64\*") -or
                     ($fp -like "*\microsoft\windows\*") -or
                     ($fp -like "*\microsoft\windows defender\*") -or
                     ($fp -like "*\programdata\microsoft\windows\*") -or
                     ($fp.Length -lt 20)
            if (-not $isSys) {{ $found += "FS:$($_.FullName)" }}
        }}
    }}
}}

$regPaths = @('HKCU:\SOFTWARE', 'HKLM:\SOFTWARE', 'HKLM:\SOFTWARE\WOW6432Node')
foreach ($rp in $regPaths) {{
    if (-not (Test-Path $rp)) {{ continue }}
    Get-ChildItem $rp -ErrorAction SilentlyContinue | ForEach-Object {{
        if (Match-Kw $_.PSChildName) {{
            $n = $_.PSChildName.ToLower()
            # Blocklist étendue — ne jamais toucher aux clés système critiques
            $isSys = ($n -eq 'microsoft') -or ($n -eq 'windows') -or
                     ($n -eq 'classes') -or ($n -like 'wow6432*') -or
                     ($n -eq 'policies') -or ($n -eq 'drivers') -or
                     ($n -eq 'system') -or ($n -like 'system*') -or
                     ($n -eq 'currentcontrolset') -or ($n -eq 'currentversion') -or
                     ($n -eq 'hardware') -or ($n -eq 'security') -or
                     ($n -eq 'sam') -or ($n -eq 'bcd00000000') -or
                     ($n -eq 'services') -or ($n -like 'net*') -or
                     ($n -eq 'run') -or ($n -like 'run*') -or
                     ($n -eq 'explorer') -or ($n -eq 'shell') -or
                     ($n -eq 'fonts') -or ($n -eq 'internet settings') -or
                     ($n -like 'directx*') -or ($n -like 'opencl*') -or
                     ($n -like 'vulkan*') -or ($n -like 'opengl*') -or
                     # Protège HKLM pour les entrées Microsoft/Windows natives
                     ($rp -like 'HKLM:*' -and $_.PSPath -like '*\Microsoft\*') -or
                     ($rp -like 'HKLM:*' -and $_.PSPath -like '*\Windows*')
            if (-not $isSys) {{
                $kp = ($_.PSPath -replace '^.*::', '') -replace '^HKEY_LOCAL_MACHINE\\', 'HKLM:\' -replace '^HKEY_CURRENT_USER\\', 'HKCU:\'
                $found += "RK:$kp"
            }}
        }}
    }}
}}

$runPaths = @('HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run','HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run')
foreach ($rp in $runPaths) {{
    if (-not (Test-Path $rp)) {{ continue }}
    (Get-ItemProperty $rp -ErrorAction SilentlyContinue).PSObject.Properties |
        Where-Object {{ $_.Name -notlike 'PS*' }} |
        ForEach-Object {{ if (Match-Kw $_.Name) {{ $found += "EX:$($_.Name)" }} }}
}}

$sc = @("$env:USERPROFILE\Desktop","$env:APPDATA\Microsoft\Windows\Start Menu\Programs")
foreach ($p in $sc) {{
    if (-not (Test-Path $p)) {{ continue }}
    Get-ChildItem $p -Recurse -Include '*.lnk' -ErrorAction SilentlyContinue |
        Where-Object {{ Match-Kw $_.BaseName }} | ForEach-Object {{ $found += "LN:$($_.FullName)" }}
    Get-ChildItem $p -Directory -ErrorAction SilentlyContinue |
        Where-Object {{ Match-Kw $_.Name }} | ForEach-Object {{ $found += "FS:Menu:$($_.FullName)" }}
}}

$found | ConvertTo-Json -Compress
"#, kw_json, ps_lit(&shared_json));

    let Some(text) = run_ps(&ps, 120) else { return vec![] };
    let text = text.trim();
    if text.is_empty() || text == "null" { return vec![]; }
    let json = if text.starts_with('"') { format!("[{}]", text) } else { text.to_string() };
    serde_json::from_str::<Vec<String>>(&json).unwrap_or_default()
}

// ── Suppression définitive des résidus ───────────────────────────────────────

pub fn delete_residuals(paths: Vec<String>) -> ResidualCleanResult {
    if paths.is_empty() {
        return ResidualCleanResult { success: true, deleted_count: 0, failed_count: 0, message: "Rien à supprimer.".into() };
    }
    let paths_json = ps_lit(&serde_json::to_string(&paths).unwrap_or_default());
    let ps = format!(r#"
$items = {} | ConvertFrom-Json
$shared = {} | ConvertFrom-Json
$ok = 0; $fail = 0

# ── Garde de sécurité : refuse de supprimer tout chemin système critique ──
function Is-SafeToDelete($item) {{
    # Chemins fichiers/dossiers interdits
    $sysFS = @(
        'C:\Windows\', 'C:\windows\',
        '\System32\', '\SysWOW64\', '\system32\', '\syswow64\',
        'C:\Program Files\Common Files\',
        'C:\Program Files (x86)\Common Files\',
        'C:\ProgramData\Microsoft\Windows\',
        '\AppData\Roaming\Microsoft\Windows\',
        '\AppData\Local\Microsoft\Windows\',
        'C:\Users\Default\', 'C:\Users\Public\',
        'C:\ProgramData\Microsoft\Windows Defender\'
    )
    # Clés registre interdites
    $sysReg = @(
        'HKLM:\SOFTWARE\Microsoft\',
        'HKLM:\SOFTWARE\Classes\',
        'HKLM:\SOFTWARE\Policies\',
        'HKLM:\SYSTEM\',
        'HKLM:\SAM\',
        'HKLM:\SECURITY\',
        'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\',
        'HKCU:\SOFTWARE\Microsoft\Windows NT\',
        'HKCU:\SOFTWARE\Classes\'
    )
    # Préfixes ASCII : FS: (filesystem), RK: (registry key), EX: (executable/Run), LN: (link)
    # FS:Menu: = dossier dans le menu démarrer
    $parts = $item -split ':', 2
    $prefix = $parts[0]
    if ($prefix -eq 'FS' -or $prefix -eq 'LN') {{
        # FS:Menu:chemin ou FS:chemin ou LN:chemin
        $raw = ($item -split ':', 2)[1]
        if ($raw -like 'Menu:*') {{ $raw = ($raw -split ':', 2)[1] }}
        if ($shared -contains (Split-Path -Leaf $raw).ToLower()) {{ return $false }}
        # Longueur minimum : évite les chemins racine trop courts
        if ($raw.Length -lt 12) {{ return $false }}
        # Exception : dossier de groupe Menu Démarrer par-utilisateur — c'est
        # EXACTEMENT le dossier que preview_residuals scanne pour taguer
        # FS:Menu:, mais il vit sous \AppData\Roaming\Microsoft\Windows\ (blocage
        # générique ci-dessous) : sans cette exception ciblée, la suppression de
        # groupes Menu Démarrer était refusée à 100% depuis toujours, alors que
        # c'est précisément la fonctionnalité que ce tag existe pour couvrir.
        if ($raw -like '*\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\*') {{ return $true }}
        foreach ($p in $sysFS) {{ if ($raw -like "*$p*") {{ return $false }} }}
        return $true
    }}
    if ($prefix -eq 'RK') {{
        $path = ($item -split ':', 2)[1]
        # Forme PSPath (« Microsoft.PowerShell.Core\Registry::HKEY_... ») : sans
        # normalisation, aucune des cles interdites ci-dessus ne matchait jamais.
        $path = ($path -replace '^.*::', '') -replace '^HKEY_LOCAL_MACHINE\\', 'HKLM:\' -replace '^HKEY_CURRENT_USER\\', 'HKCU:\'
        if ($path.Length -lt 20) {{ return $false }}
        foreach ($p in $sysReg) {{ if ($path -like "$p*") {{ return $false }} }}
        return $true
    }}
    if ($prefix -eq 'EX') {{ return $true }}  # Entrées Run : toujours safe
    return $false
}}

foreach ($item in $items) {{
    # Vérification sécurité avant toute suppression
    if (-not (Is-SafeToDelete $item)) {{
        Write-Warning "SKIP (protégé): $item"
        $fail++
        continue
    }}
    try {{
        $parts = $item -split ':', 2
        $prefix = $parts[0]
        $payload = $parts[1]
        if ($prefix -eq 'FS' -and $payload -like 'Menu:*') {{
            $path = ($payload -split ':', 2)[1]
            Remove-Item $path -Recurse -Force -ErrorAction Stop; $ok++
        }} elseif ($prefix -eq 'FS') {{
            Remove-Item $payload -Recurse -Force -ErrorAction Stop; $ok++
        }} elseif ($prefix -eq 'RK') {{
            Remove-Item $payload -Recurse -Force -ErrorAction Stop; $ok++
        }} elseif ($prefix -eq 'EX') {{
            # -EA SilentlyContinue sur les deux (l'entree n'existe que dans UNE
            # des deux ruches) puis $ok++ inconditionnel comptait toujours cette
            # entree comme nettoyee, meme si un vrai echec de suppression (droits,
            # cle verrouillee) laissait la valeur en place dans la ruche ou elle
            # existait reellement. Verifie maintenant la vraie disparition dans
            # les deux ruches avant de compter un succes, meme pattern que FS/RK/LN.
            Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' -Name $payload -ErrorAction SilentlyContinue
            Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' -Name $payload -ErrorAction SilentlyContinue
            $stillHkcu = $null -ne (Get-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' -Name $payload -ErrorAction SilentlyContinue)
            $stillHklm = $null -ne (Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' -Name $payload -ErrorAction SilentlyContinue)
            if ($stillHkcu -or $stillHklm) {{ $fail++ }} else {{ $ok++ }}
        }} elseif ($prefix -eq 'LN') {{
            Remove-Item $payload -Force -ErrorAction Stop; $ok++
        }} else {{ $fail++ }}
    }} catch {{ $fail++ }}
}}
@{{ok=$ok;fail=$fail}} | ConvertTo-Json -Compress
"#, paths_json, ps_lit(&serde_json::to_string(SHARED_DIRS).unwrap_or_else(|_| "[]".into())));

    match run_ps(&ps, 300) {
        Some(t) => {
            let t = t.trim();
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
                let ok   = v["ok"].as_u64().unwrap_or(0) as usize;
                let fail = v["fail"].as_u64().unwrap_or(0) as usize;
                ResidualCleanResult {
                    success: fail == 0,
                    deleted_count: ok, failed_count: fail,
                    message: format!("{} résidu(s) supprimé(s), {} échec(s)", ok, fail),
                }
            } else {
                ResidualCleanResult { success: false, deleted_count: 0, failed_count: paths.len(), message: "Erreur parse".into() }
            }
        }
        None => ResidualCleanResult { success: false, deleted_count: 0, failed_count: paths.len(), message: "Erreur PowerShell".into() },
    }
}

// ── Extraction puis suppression des résidus ──────────────────────────────────

/// Miroir Rust (testable) de la fonction PowerShell `Is-SafeToDelete` embarquée
/// dans `delete_residuals` — réutilisé ici pour filtrer AVANT la copie dans
/// `extract_residuals`, qui ne faisait auparavant aucune vérification avant de
/// copier (seule la suppression finale, déléguée à `delete_residuals`, était
/// protégée) : un chemin système protégé finissait copié dans le dossier cible
/// choisi par l'utilisateur avant d'être correctement ignoré à la suppression.
fn is_safe_residual(item: &str) -> bool {
    const SYS_FS: [&str; 11] = [
        r"c:\windows\", r"\system32\", r"\syswow64\",
        r"c:\program files\common files\", r"c:\program files (x86)\common files\",
        r"c:\programdata\microsoft\windows\",
        r"\appdata\roaming\microsoft\windows\", r"\appdata\local\microsoft\windows\",
        r"c:\users\default\", r"c:\users\public\",
        r"c:\programdata\microsoft\windows defender\",
    ];
    const SYS_REG: [&str; 9] = [
        r"hklm:\software\microsoft\", r"hklm:\software\classes\", r"hklm:\software\policies\",
        r"hklm:\system\", r"hklm:\sam\", r"hklm:\security\",
        r"hkcu:\software\microsoft\windows\currentversion\explorer\",
        r"hkcu:\software\microsoft\windows nt\", r"hkcu:\software\classes\",
    ];
    let Some((prefix, rest)) = item.split_once(':') else { return false };
    match prefix {
        "FS" | "LN" => {
            let raw = rest.strip_prefix("Menu:").unwrap_or(rest);
            if raw.len() < 12 { return false; }
            let raw_lower = raw.to_lowercase();
            let leaf = raw_lower.trim_end_matches('\\').rsplit('\\').next().unwrap_or("");
            if SHARED_DIRS.contains(&leaf) { return false; }
            // Exception : dossier de groupe Menu Démarrer par-utilisateur — miroir
            // de l'exception PS dans delete_residuals (voir commentaire là-bas).
            if raw_lower.contains(r"\appdata\roaming\microsoft\windows\start menu\programs\") {
                return true;
            }
            !SYS_FS.iter().any(|p| raw_lower.contains(p))
        }
        "RK" => {
            let rest_lower = normalize_reg_path(rest).to_lowercase();
            if rest_lower.len() < 20 { return false; }
            !SYS_REG.iter().any(|p| rest_lower.starts_with(p))
        }
        "EX" => true,
        _ => false,
    }
}

/// « Microsoft.PowerShell.Core\Registry::HKEY_CURRENT_USER\X » -> « HKCU:\X »
/// (forme comparee par les listes de cles interdites).
fn normalize_reg_path(p: &str) -> String {
    let p = p.rsplit_once("::").map_or(p, |(_, r)| r);
    let upper = p.to_uppercase();
    for (long, short) in [("HKEY_LOCAL_MACHINE\\", "HKLM:\\"), ("HKEY_CURRENT_USER\\", "HKCU:\\")] {
        if upper.starts_with(long) {
            return format!("{short}{}", &p[long.len()..]);
        }
    }
    p.to_string()
}

pub fn extract_residuals(paths: Vec<String>, target: String) -> ResidualCleanResult {
    if paths.is_empty() {
        return ResidualCleanResult { success: true, deleted_count: 0, failed_count: 0, message: "Rien à extraire.".into() };
    }
    let skipped = paths.iter().filter(|p| !is_safe_residual(p)).count();
    let paths: Vec<String> = paths.into_iter().filter(|p| is_safe_residual(p)).collect();
    if paths.is_empty() {
        return ResidualCleanResult {
            success: false, deleted_count: 0, failed_count: skipped,
            message: "Aucun résidu sûr à extraire (chemins protégés ignorés).".into(),
        };
    }

    let target_path = std::path::PathBuf::from(&target);

    // Canonicaliser pour éviter le path traversal (../../)
    let canonical_target = match target_path.canonicalize()
        .or_else(|_| { std::fs::create_dir_all(&target_path)?; target_path.canonicalize() }) {
        Ok(p) => p,
        Err(e) => return ResidualCleanResult {
            success: false, deleted_count: 0, failed_count: paths.len(),
            message: format!("Destination inaccessible: {}", e),
        },
    };

    // Exiger un chemin absolu sur un lecteur Windows (ex: C:\...)
    {
        let s = canonical_target.to_string_lossy();
        let valid = s.len() >= 3
            && s.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
            && s.as_bytes().get(1) == Some(&b':')
            && (s.as_bytes().get(2) == Some(&b'\\') || s.as_bytes().get(2) == Some(&b'/'));
        if !valid {
            return ResidualCleanResult {
                success: false, deleted_count: 0, failed_count: paths.len(),
                message: format!("Destination invalide (chemin absolu Windows requis): {}", target),
            };
        }
    }

    if let Err(e) = std::fs::create_dir_all(&canonical_target) {
        return ResidualCleanResult { success: false, deleted_count: 0, failed_count: paths.len(), message: format!("Destination inaccessible: {}", e) };
    }

    let paths_json = serde_json::to_string(&paths).unwrap_or_default();
    let target_str = canonical_target.to_string_lossy().to_string();
    let paths_json_escaped = ps_lit(&paths_json);
    let target_json = ps_lit(&target_str);
    let ps = format!(r#"
$items = {} | ConvertFrom-Json
$target = {}
$ok = 0; $fail = 0

# Export des clés registre en .reg — un fichier PAR clé.
# `reg export` réécrit tout le fichier à chaque appel (pas d'append) : un seul
# .reg partagé ne garderait que la DERNIÈRE clé. On numérote donc les fichiers.
# Préfixes ASCII : RK: (registry key), FS: (filesystem), LN: (link), EX: (Run entry)
$regItems = @($items | Where-Object {{ ($_ -split ':', 2)[0] -eq 'RK' }})
$regIdx = 0
foreach ($r in $regItems) {{
    $path = ($r -split ':', 2)[1]
    try {{
        $path2 = ($path -replace '^.*::', '') -replace '^(HKLM|HKCU):\\', '$1\'
        $regIdx++
        $regFile = Join-Path $target ("residuals_registry_{{0}}.reg" -f $regIdx)
        & reg export "$path2" "$regFile" /y 2>$null
        if ($LASTEXITCODE -eq 0) {{ $ok++ }} else {{ $fail++ }}
    }} catch {{ $fail++ }}
}}

# Copie des fichiers
$fileItems = $items | Where-Object {{ $p = ($_ -split ':', 2)[0]; $p -eq 'FS' -or $p -eq 'LN' }}
foreach ($item in $fileItems) {{
    $payload = ($item -split ':', 2)[1]
    $path = if ($payload -like 'Menu:*') {{ ($payload -split ':', 2)[1] }}
            else                          {{ $payload }}
    try {{
        Copy-Item -LiteralPath $path -Destination $target -Recurse -Force -ErrorAction Stop; $ok++
    }} catch {{ $fail++ }}
}}

@{{ok=$ok;fail=$fail}} | ConvertTo-Json -Compress
"#, paths_json_escaped, target_json);

    let copy_result = run_ps(&ps, 600);
    let (copied, copy_fail) = copy_result
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim()).ok())
        .map(|v| (v["ok"].as_u64().unwrap_or(0) as usize, v["fail"].as_u64().unwrap_or(0) as usize))
        .unwrap_or((0, paths.len()));

    // Suppression après extraction
    let del = delete_residuals(paths);

    ResidualCleanResult {
        success: del.success && skipped == 0,
        deleted_count: del.deleted_count,
        failed_count: del.failed_count + skipped,
        message: if skipped > 0 {
            format!("{} copié(s) dans «{}», {} supprimé(s), {} ignoré(s) (protégé)", copied, target, del.deleted_count, skipped)
        } else if copy_fail > 0 {
            format!("{} copié(s) ({} échec), {} supprimé(s) dans «{}»", copied, copy_fail, del.deleted_count, target)
        } else {
            format!("{} copié(s) dans «{}», {} supprimé(s)", copied, target, del.deleted_count)
        },
    }
}

/// PowerShell avec timeout (avant : `.output()` sans borne, un desinstalleur
/// qui attend un clic dans une fenetre invisible bloquait la file a vie).
fn run_ps(script: &str, timeout_secs: u64) -> Option<String> {
    crate::maintenance::commands::execute_system_command(
        "powershell", &["-NoProfile", "-NonInteractive", "-Command", script], timeout_secs,
    ).ok().map(|r| r.stdout)
}

#[cfg(test)]
mod uninstaller_tests {
    use super::*;

    // Live, non-CI verification (ignorée par défaut) : confirme sur une vraie
    // machine Windows que list_installed_apps() ne produit plus de mojibake
    // sur un DisplayName accentué réel du registre local.
    #[test]
    #[ignore]
    fn list_installed_apps_no_mojibake_on_real_registry() {
        let apps = list_installed_apps();
        assert!(!apps.is_empty(), "aucune app trouvée — le registre local est-il vide ?");
        let has_replacement_char = apps.iter().any(|a| a.name.contains('\u{FFFD}') || a.publisher.contains('\u{FFFD}'));
        assert!(!has_replacement_char, "mojibake détecté dans un DisplayName/Publisher réel : {:?}",
            apps.iter().find(|a| a.name.contains('\u{FFFD}') || a.publisher.contains('\u{FFFD}')));
    }

    #[test]
    fn safe_fs_residual_allowed() {
        assert!(is_safe_residual(r"FS:C:\Users\Momo\AppData\Roaming\SomeApp"));
    }

    #[test]
    fn windows_system32_fs_residual_blocked() {
        assert!(!is_safe_residual(r"FS:C:\Windows\System32\drivers\etc\hosts"));
    }

    #[test]
    fn windows_defender_fs_residual_blocked() {
        assert!(!is_safe_residual(r"FS:C:\ProgramData\Microsoft\Windows Defender\Platform"));
    }

    #[test]
    fn menu_prefixed_start_menu_programs_folder_allowed() {
        // Régression : preview_residuals ne scanne QUE
        // %APPDATA%\Microsoft\Windows\Start Menu\Programs (par-utilisateur, jamais
        // ProgramData tous-users) pour générer le tag FS:Menu: — vérifié en direct
        // sur cette machine (menu_check.ps1) : sans l'exception ciblée, ce chemin
        // était TOUJOURS refusé, rendant le nettoyage de groupes Menu Démarrer
        // non-fonctionnel depuis toujours malgré l'apparence d'une feature qui marche.
        assert!(is_safe_residual(r"FS:Menu:C:\Users\Momo\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\SomeApp"));
    }

    #[test]
    fn menu_prefixed_programdata_all_users_still_blocked() {
        // ProgramData\Microsoft\Windows\ (tous-users) n'est PAS ce que
        // preview_residuals scanne — reste protégé par la blocklist générale,
        // l'exception ne couvre que le cas AppData\Roaming réellement scanné.
        assert!(!is_safe_residual(r"FS:Menu:C:\ProgramData\Microsoft\Windows\Start Menu\Programs\SomeApp"));
    }

    #[test]
    fn short_fs_path_blocked() {
        // < 12 caractères après le préfixe : trop court pour être un vrai résidu.
        assert!(!is_safe_residual("FS:C:\\a"));
    }

    #[test]
    fn safe_registry_key_allowed() {
        assert!(is_safe_residual(r"RK:HKCU:\SOFTWARE\SomeVendor\SomeApp\Settings"));
    }

    #[test]
    fn hklm_software_microsoft_registry_blocked() {
        assert!(!is_safe_residual(r"RK:HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion"));
    }

    #[test]
    fn hklm_system_registry_blocked() {
        assert!(!is_safe_residual(r"RK:HKLM:\SYSTEM\CurrentControlSet\Services"));
    }

    #[test]
    fn run_entry_always_allowed() {
        assert!(is_safe_residual("EX:SomeAppAutostart"));
    }

    #[test]
    fn unknown_prefix_blocked() {
        assert!(!is_safe_residual("XX:whatever"));
        assert!(!is_safe_residual("no-colon-at-all"));
    }

    // Live, lecture seule (rien n'est supprime) : le script PowerShell de
    // preview s'execute et ne propose jamais un dossier parapluie partage.
    #[test]
    #[ignore]
    fn preview_residuals_live_never_lists_shared_dirs() {
        let found = preview_residuals("Microsoft Teams".into(), "Microsoft Corporation".into());
        for f in &found {
            assert!(is_safe_residual(f) || f.starts_with("RK:HKLM"), "residu dangereux propose : {f}");
            let leaf = f.trim_end_matches('\\').rsplit('\\').next().unwrap_or("").to_lowercase();
            assert!(!SHARED_DIRS.contains(&leaf.as_str()), "dossier partage propose : {f}");
        }
        eprintln!("{} residu(s) : {:?}", found.len(), found);
    }

    #[test]
    fn ps_lit_neutralises_quotes() {
        assert_eq!(ps_lit(r#""C:\Program Files\X\unins.exe" /S"#), r#"'"C:\Program Files\X\unins.exe" /S'"#);
        assert_eq!(ps_lit("l'app d\u{2019}installation $x"), "'l''app d\u{2019}\u{2019}installation $x'");
    }

    // Live, lecture seule : PowerShell PARSE les scripts generes sans les
    // executer (Parser::ParseInput). Prouve l'absence d'erreur de syntaxe.
    #[test]
    #[ignore]
    fn generated_scripts_parse_in_powershell() {
        let scripts = [
            uninstall_script("Assistant d\u{2019}installation", r#""C:\Program Files\Obsidian\Uninstall Obsidian.exe" /currentuser"#),
            uninstall_script("7-Zip 24.08 (x64)", "MsiExec.exe /I{23170F69-40C1-2702-2408-000001000000}"),
        ];
        for sc in scripts {
            let check = format!(
                "$e=$null; [void][System.Management.Automation.Language.Parser]::ParseInput({}, [ref]$null, [ref]$e); $e.Count",
                ps_lit(&sc)
            );
            let out = run_ps(&check, 30).unwrap_or_default();
            assert_eq!(out.trim(), "0", "erreurs de syntaxe PowerShell : {out}\n{sc}");
        }
    }

    #[test]
    fn keywords_skip_vendor_umbrellas_and_numbers() {
        let kw = residual_keywords("Microsoft Teams (work or school)", "Microsoft Corporation");
        assert_eq!(kw, vec!["teams".to_string(), "work".into(), "school".into()]);
        let kw = residual_keywords("Microsoft Visual C++ 2015-2022 Redistributable (x64)", "Microsoft Corporation");
        assert_eq!(kw, vec!["visual".to_string()]);
        assert!(residual_keywords("Google Chrome", "Google LLC").iter().all(|k| k != "google"));
    }

    #[test]
    fn shared_vendor_folders_never_deletable() {
        assert!(!is_safe_residual(r"FS:C:\Users\Momo\AppData\Roaming\Microsoft"));
        assert!(!is_safe_residual(r"FS:C:\Users\Momo\AppData\Local\Google\"));
        assert!(!is_safe_residual(r"FS:C:\Users\Momo\AppData\Local\Packages"));
        assert!(is_safe_residual(r"FS:C:\Users\Momo\AppData\Local\Notepad++"));
    }

    #[test]
    fn pspath_registry_form_is_guarded_too() {
        // Forme produite par l'ancien preview_residuals : la garde ne la voyait pas.
        assert!(!is_safe_residual(r"RK:Microsoft.PowerShell.Core\Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Foo"));
        assert!(!is_safe_residual(r"RK:Microsoft.PowerShell.Core\Registry::HKEY_CURRENT_USER\SOFTWARE\Classes\Foo"));
        assert!(is_safe_residual(r"RK:Microsoft.PowerShell.Core\Registry::HKEY_CURRENT_USER\SOFTWARE\SomeVendor"));
        assert_eq!(normalize_reg_path(r"Microsoft.PowerShell.Core\Registry::HKEY_CURRENT_USER\SOFTWARE\X"), r"HKCU:\SOFTWARE\X");
    }

    #[test]
    fn case_insensitive_like_powershell() {
        // PowerShell -like est insensible à la casse — le miroir Rust doit l'être aussi.
        assert!(!is_safe_residual(r"FS:c:\WINDOWS\system32\evil.dll"));
        assert!(!is_safe_residual(r"RK:hklm:\Software\Microsoft\Foo"));
    }
}
