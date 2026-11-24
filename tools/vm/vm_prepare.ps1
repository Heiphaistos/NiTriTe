# Prépare la VM Hyper-V de test et y lance NiTriTe (build de test avec port CDP 9333).
# Usage (PowerShell admin hôte) : vm_prepare.ps1 [-Exe <nitrite.exe>] [-Vm 'W11 dev'] [-Snapshot 'avant-NiTriTe-test']
# Checkpoint hors ligne -> copie de l'exe + launch.ps1 dans le VHD -> service SYSTEM à usage unique -> démarrage.
# Restauration : vm_restore.ps1 (remet la VM dans l'état du checkpoint puis le supprime).
param([string]$Exe = "$PSScriptRoot\..\..\src-tauri\target\debug\nitrite.exe",
      [string]$Vm = 'W11 dev', [string]$Snapshot = 'avant-NiTriTe-test', [int]$MemoryGo = 4)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path $Exe)) { throw "Exe introuvable : $Exe" }
if ((Get-PSDrive D).Free -lt 20GB) { throw 'Moins de 20 Go libres sur D: (la VM passerait en PausedCritical).' }
if ((Get-VM $Vm).State -ne 'Off') { Stop-VM $Vm -TurnOff -Force }
if (-not (Get-VMSnapshot -VMName $Vm -Name $Snapshot -ErrorAction SilentlyContinue)) { Checkpoint-VM -Name $Vm -SnapshotName $Snapshot }
Set-VMMemory $Vm -DynamicMemoryEnabled $true -StartupBytes ($MemoryGo * 1GB) -MinimumBytes 1GB -MaximumBytes (($MemoryGo + 1) * 1GB)
Get-VMDvdDrive -VMName $Vm | Set-VMDvdDrive -Path $null
$p = (Get-VM $Vm).HardDrives[0].Path
$d = Mount-VHD -Path $p -Passthru | Get-Disk
try {
    $L = (Get-Partition -DiskNumber $d.Number | Get-Volume | Where-Object { $_.DriveLetter -and $_.Size -gt 50GB }).DriveLetter
    $dst = "${L}:\NiTriTeTest"
    Remove-Item $dst -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $dst | Out-Null
    Copy-Item $Exe "$dst\nitrite.exe" -Force
    Copy-Item "$PSScriptRoot\vm_launch.ps1" "$dst\launch.ps1" -Force
    reg load HKLM\VMSYS "${L}:\Windows\System32\config\SYSTEM" | Out-Null
    $s = 'HKLM:\VMSYS\ControlSet001\Services'
    foreach ($n in 'wuauserv', 'UsoSvc', 'WaaSMedicSvc') { if (Test-Path "$s\$n") { Set-ItemProperty "$s\$n" Start 4 -Type DWord } }
    $k = "$s\NiTriTeTest"; New-Item $k -Force | Out-Null
    Set-ItemProperty $k Type 16 -Type DWord; Set-ItemProperty $k Start 2 -Type DWord; Set-ItemProperty $k ErrorControl 0 -Type DWord
    Set-ItemProperty $k ImagePath 'cmd.exe /c start "" /b powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\NiTriTeTest\launch.ps1' -Type ExpandString
    Set-ItemProperty $k ObjectName LocalSystem
    [gc]::Collect(); Start-Sleep 1
    reg unload HKLM\VMSYS | Out-Null
}
finally { Dismount-VHD -Path $p }
Start-VM $Vm
"VM demarree : $((Get-VM $Vm).State) ; D: libre $([math]::Round((Get-PSDrive D).Free/1GB,1)) Go"
