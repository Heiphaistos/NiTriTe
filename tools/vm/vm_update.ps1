# Remplace l'exe de test dans la VM hors ligne (VM arrêtée, VHD monté) puis redémarre :
# le service injecté par vm_prepare.ps1 relance NiTriTe au démarrage de session.
param([string]$Exe = "$PSScriptRoot\..\..\src-tauri\target\debug\nitrite.exe", [string]$Vm = 'W11 dev')
$ErrorActionPreference = 'Stop'
Stop-VM $Vm -Force
$p = (Get-VM $Vm).HardDrives[0].Path
$d = Mount-VHD -Path $p -Passthru | Get-Disk
try {
    $L = (Get-Partition -DiskNumber $d.Number | Get-Volume | Where-Object { $_.DriveLetter -and $_.Size -gt 50GB }).DriveLetter
    Copy-Item $Exe "${L}:\NiTriTeTest\nitrite.exe" -Force
    Copy-Item "$PSScriptRoot\vm_launch.ps1" "${L}:\NiTriTeTest\launch.ps1" -Force
    Remove-Item "${L}:\NiTriTeTest\nitrite-new.exe", "${L}:\NiTriTeTest\started" -Force -ErrorAction SilentlyContinue
    # Le service à usage unique ne se relance pas seul : on le remet en démarrage auto.
    reg load HKLM\VMSYS "${L}:\Windows\System32\config\SYSTEM" | Out-Null
    Set-ItemProperty 'HKLM:\VMSYS\ControlSet001\Services\NiTriTeTest' Start 2 -Type DWord
    [gc]::Collect(); Start-Sleep 1
    reg unload HKLM\VMSYS | Out-Null
}
finally { Dismount-VHD -Path $p }
Start-VM $Vm
"VM redemarree avec le nouvel exe : $((Get-VM $Vm).State)"
