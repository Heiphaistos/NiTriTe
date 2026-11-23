# Remet la VM de test dans son état d'avant les tests, puis supprime le checkpoint.
param([string]$Vm = 'W11 dev', [string]$Snapshot = 'avant-NiTriTe-test')
$ErrorActionPreference = 'Stop'
if ((Get-VM $Vm).State -ne 'Off') { Stop-VM $Vm -TurnOff -Force }
$snap = Get-VMSnapshot -VMName $Vm -Name $Snapshot -ErrorAction SilentlyContinue
if (-not $snap) { throw "Checkpoint $Snapshot introuvable" }
Restore-VMSnapshot -VMSnapshot $snap -Confirm:$false
Remove-VMSnapshot -VMSnapshot $snap
Set-VMMemory $Vm -DynamicMemoryEnabled $true -StartupBytes 8GB -MinimumBytes 512MB -MaximumBytes 1TB
"VM restauree ($((Get-VM $Vm).State)), checkpoints restants : $((Get-VMSnapshot -VMName $Vm | Measure-Object).Count)"
