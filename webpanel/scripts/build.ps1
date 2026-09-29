# Construit NiTriTe Agent (un seul .exe, interface web embarquee).
# A lancer depuis PowerShell, a la racine du depot NiTriTe :
#   powershell -ExecutionPolicy Bypass -File webpanel\scripts\build.ps1
$ErrorActionPreference = "Stop"
$web = Split-Path -Parent $PSScriptRoot
$root = Split-Path -Parent $web

Write-Host "==> Dependances de l'interface" -ForegroundColor Cyan
if (Test-Path (Join-Path $root "package.json")) { Push-Location $root; npm ci; Pop-Location }
Push-Location $web
npm ci
Write-Host "==> Tests + build de l'interface web" -ForegroundColor Cyan
npm test
npm run build
Pop-Location

Write-Host "==> Compilation de l'agent (release)" -ForegroundColor Cyan
Push-Location (Join-Path $web "agent")
cargo build --release -p nitrite-agent
Pop-Location

$out = Join-Path $web "release"
New-Item -ItemType Directory -Force -Path $out | Out-Null
Copy-Item (Join-Path $web "agent\target\release\nitrite-agent.exe") (Join-Path $out "NiTriTe-Agent.exe") -Force
Write-Host "==> $out\NiTriTe-Agent.exe" -ForegroundColor Green
