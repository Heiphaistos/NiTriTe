# Exécuté DANS la VM par le service SYSTEM injecté : ouvre le port CDP vers l'hôte puis
# lance NiTriTe élevé dans la session de l'utilisateur connecté (sans invite UAC).
$log = 'C:\NiTriTeTest\launch.log'
"[$(Get-Date -f HH:mm:ss)] start" | Out-File $log -Append
Start-Sleep 20
for ($i = 0; $i -lt 180; $i++) {
    $u = (Get-CimInstance Win32_ComputerSystem).UserName
    if ($u -and (Get-Process explorer -ErrorAction SilentlyContinue)) { break }
    Start-Sleep 5
}
"user=$u" | Out-File $log -Append
# WebView2 n'écoute que sur 127.0.0.1:9333 : relais 0.0.0.0:9334 pour l'hôte, réseau privé de test.
netsh interface portproxy add v4tov4 listenaddress=0.0.0.0 listenport=9334 connectaddress=127.0.0.1 connectport=9333 2>&1 | Out-File $log -Append
netsh advfirewall firewall add rule name=NiTriTeCDP dir=in action=allow protocol=TCP localport=9334 2>&1 | Out-File $log -Append
# Nouvel exe déposé par Copy-VMFile (vm_update.ps1) : remplacé au démarrage, avant le lancement.
if (Test-Path C:\NiTriTeTest\nitrite-new.exe) { Move-Item C:\NiTriTeTest\nitrite-new.exe C:\NiTriTeTest\nitrite.exe -Force }
if (-not (Get-Process nitrite -ErrorAction SilentlyContinue)) {
    schtasks /Create /TN NiTriTeTest /TR 'C:\NiTriTeTest\nitrite.exe' /SC ONCE /ST 00:00 /RU $u /RL HIGHEST /IT /F 2>&1 | Out-File $log -Append
    schtasks /Run /TN NiTriTeTest 2>&1 | Out-File $log -Append
}
"[$(Get-Date -f HH:mm:ss)] done" | Out-File $log -Append
