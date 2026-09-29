# Test de fumee de NiTriTe Agent sur un vrai Windows (CI et poste local).
# Lance l'exe, verifie la securite, les commandes reelles (WMI, registre,
# sysinfo), le WebSocket d'evenements et l'arret propre.
#
#   pwsh -File scripts\smoke-test.ps1 -Exe agent\target\release\nitrite-agent.exe
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [int]$Port = 7979
)
$ErrorActionPreference = "Stop"
$failures = 0
function Check($name, [bool]$ok, $detail = "") {
  if ($ok) { Write-Host "  OK   $name" -ForegroundColor Green }
  else { Write-Host "  ECHEC $name $detail" -ForegroundColor Red; $script:failures++ }
}

$sessionFile = Join-Path $env:LOCALAPPDATA "NiTriTe-WebPanel\session.json"
Remove-Item $sessionFile -ErrorAction SilentlyContinue

Write-Host "==> Demarrage de $Exe" -ForegroundColor Cyan
$proc = Start-Process -FilePath $Exe -ArgumentList "--no-browser", "--stay", "--no-update-check", "--port", $Port -PassThru
try {
  $deadline = (Get-Date).AddSeconds(60)
  while (-not (Test-Path $sessionFile) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
  Check "session.json cree" (Test-Path $sessionFile)
  $s = Get-Content $sessionFile -Raw | ConvertFrom-Json
  $base = "http://127.0.0.1:$($s.port)"
  $auth = @{ "X-Nitrite-Token" = $s.token }

  function Req($method, $path, $headers = @{}, $body = $null) {
    $p = @{ Method = $method; Uri = "$base$path"; Headers = $headers; SkipHttpErrorCheck = $true; TimeoutSec = 120 }
    if ($null -ne $body) { $p.Body = ($body | ConvertTo-Json -Compress -Depth 5); $p.ContentType = "application/json" }
    Invoke-WebRequest @p
  }

  Write-Host "==> Securite" -ForegroundColor Cyan
  Check "sans jeton -> 401" ((Req GET "/api/health").StatusCode -eq 401)
  Check "mauvais Host -> 403" ((Req GET "/api/health" (@{ Host = "evil.example:$($s.port)" } + $auth)).StatusCode -eq 403)
  Check "mauvaise Origin -> 403" ((Req POST "/api/invoke/get_apps" (@{ Origin = "http://evil.example" } + $auth) @{}).StatusCode -eq 403)
  $index = Req GET "/"
  Check "CSP presente" ($index.Headers["Content-Security-Policy"] -match "frame-ancestors 'none'")
  Check "X-Frame-Options DENY" ($index.Headers["X-Frame-Options"] -eq "DENY")
  $sess = Req POST "/api/session" @{} @{ token = $s.token }
  Check "cookie de session HttpOnly/Strict" ($sess.StatusCode -eq 204 -and "$($sess.Headers['Set-Cookie'])" -match "HttpOnly" -and "$($sess.Headers['Set-Cookie'])" -match "SameSite=Strict")

  Write-Host "==> Commandes NiTriTe reelles" -ForegroundColor Cyan
  $health = Req GET "/api/health" $auth
  Check "health 200" ($health.StatusCode -eq 200)
  $cmds = ($health.Content | ConvertFrom-Json).commands
  Check "commandes exposees ($cmds)" ($cmds -ge 300)
  $apps = Req POST "/api/invoke/get_apps" $auth @{}
  Check "get_apps (catalogue)" ($apps.StatusCode -eq 200 -and (($apps.Content | ConvertFrom-Json).Count -gt 700))
  $sys = Req POST "/api/invoke/get_system_info" $auth @{}
  Check "get_system_info (sysinfo/WMI)" ($sys.StatusCode -eq 200 -and $sys.Content.Length -gt 50) "$($sys.StatusCode) $($sys.Content)"
  $mon = Req POST "/api/invoke/start_monitoring" $auth @{}
  Check "start_monitoring" ($mon.StatusCode -eq 200) "$($mon.StatusCode) $($mon.Content)"

  Write-Host "==> Evenements (WebSocket)" -ForegroundColor Cyan
  $ws = [System.Net.WebSockets.ClientWebSocket]::new()
  $ws.Options.SetRequestHeader("Origin", $base)
  $ws.ConnectAsync([Uri]"ws://127.0.0.1:$($s.port)/api/events?token=$($s.token)", [Threading.CancellationToken]::None).Wait(10000) | Out-Null
  Check "WebSocket connecte" ($ws.State -eq "Open")
  # Le monitoring emet `system-monitor` en continu : on attend un message.
  $buf = [byte[]]::new(65536)
  $seg = [ArraySegment[byte]]::new($buf)
  $recv = $ws.ReceiveAsync($seg, [Threading.CancellationToken]::None)
  $got = $recv.Wait(20000)
  $text = if ($got) { [Text.Encoding]::UTF8.GetString($buf, 0, $recv.Result.Count) } else { "" }
  Check "evenement recu" ($got -and $text -match '"event"') $text
  $ws.Dispose()

  Write-Host "==> Arret" -ForegroundColor Cyan
  Check "arret demande" ((Req POST "/api/host/shutdown" $auth @{}).StatusCode -eq 202)
  Check "processus termine" ($proc.WaitForExit(15000))
  Check "session.json supprime" (-not (Test-Path $sessionFile))
}
finally {
  if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
  $log = Join-Path $env:LOCALAPPDATA "NiTriTe-WebPanel\agent.log"
  if (Test-Path $log) { Write-Host "==> agent.log"; Get-Content $log -Tail 20 }
}

if ($failures -gt 0) { Write-Host "$failures verification(s) en echec" -ForegroundColor Red; exit 1 }
Write-Host "Toutes les verifications sont passees." -ForegroundColor Green
