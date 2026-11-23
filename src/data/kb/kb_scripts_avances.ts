import type { KBCategory } from "../knowledgeBase";

export const kbScriptsAvances: KBCategory[] = [
  {
    id: "scripts-systeme-utiles",
    label: "Scripts PowerShell — Maintenance Système",
    icon: "Terminal",
    items: [
      {
        title: "Script de nettoyage système complet",
        solution: [
          "Ce script nettoie les fichiers temporaires, le cache, et génère un rapport d'espace libéré",
          "Exécuter en admin pour accéder à tous les dossiers système",
          "Paramètre -DryRun disponible pour simuler sans supprimer",
          "Nettoyage : temp utilisateur, temp Windows, prefetch, miniatures, corbeille",
        ],
        code: "# Cleanup-System.ps1 — Nettoyage complet du système\nparam([switch]$DryRun)\n\n$ErrorActionPreference = 'SilentlyContinue'\n$sizeBefore = (Get-PSDrive C).Free / 1GB\n\nWrite-Host '=== Nettoyage Système ===' -ForegroundColor Cyan\nWrite-Host \"Espace libre avant: $([math]::Round($sizeBefore, 2)) GB\"\n\n$paths = @(\n    \"$env:TEMP\\*\",\n    'C:\\Windows\\Temp\\*',\n    'C:\\Windows\\Prefetch\\*',\n    \"$env:LOCALAPPDATA\\Temp\\*\",\n    \"$env:LOCALAPPDATA\\Microsoft\\Windows\\Explorer\\thumbcache_*.db\",\n    'C:\\Windows\\SoftwareDistribution\\Download\\*',\n    'C:\\Windows\\Logs\\CBS\\*',\n    \"$env:APPDATA\\Microsoft\\Windows\\Recent\\*\"\n)\n\n$totalFreed = 0\nforeach ($path in $paths) {\n    try {\n        $items = Get-ChildItem -Path $path -Recurse -Force -ErrorAction SilentlyContinue\n        $size = ($items | Measure-Object -Property Length -Sum).Sum\n        $sizeMB = [math]::Round($size / 1MB, 2)\n        \n        if ($DryRun) {\n            Write-Host \"[SIM] $path -> $sizeMB MB\" -ForegroundColor Yellow\n        } else {\n            Remove-Item -Path $path -Recurse -Force -ErrorAction SilentlyContinue\n            Write-Host \"[OK] $path ($sizeMB MB)\" -ForegroundColor Green\n        }\n        $totalFreed += $size\n    } catch {\n        Write-Host \"[SKIP] $path\" -ForegroundColor DarkGray\n    }\n}\n\n# Vider la corbeille\nif (-not $DryRun) {\n    Clear-RecycleBin -Force -ErrorAction SilentlyContinue\n    Write-Host '[OK] Corbeille vidée' -ForegroundColor Green\n}\n\n$sizeAfter = (Get-PSDrive C).Free / 1GB\n$freed = $sizeAfter - $sizeBefore\nWrite-Host \"`n=== Résultat ===\"\nWrite-Host \"Espace libre après: $([math]::Round($sizeAfter, 2)) GB\" -ForegroundColor Cyan\nWrite-Host \"Espace libéré: $([math]::Round($freed, 2)) GB\" -ForegroundColor Green",
        note: "Exécuter en PowerShell admin : .\\Cleanup-System.ps1 ou .\\Cleanup-System.ps1 -DryRun pour la simulation.",
      },
      {
        title: "Script d'inventaire système complet",
        solution: [
          "Génère un rapport HTML détaillé de la configuration système",
          "Infos : OS, CPU, RAM, GPU, disques, réseau, processus, services",
          "Utile pour les interventions technicien — rapport à laisser au client",
        ],
        code: "# SystemInventory.ps1 — Rapport système complet en HTML\nparam([string]$OutputPath = \"C:\\Users\\Public\\Rapport_$(Get-Date -Format 'yyyyMMdd_HHmmss').html\")\n\n$css = @'\n<style>\nbody { font-family: Segoe UI, sans-serif; background: #1a1a2e; color: #e0e0e0; margin: 20px; }\nh1 { color: #f97316; } h2 { color: #fb923c; border-bottom: 1px solid #374151; }\ntable { width: 100%; border-collapse: collapse; margin-bottom: 20px; }\nth { background: #374151; padding: 8px; text-align: left; }\ntd { padding: 6px 8px; border-bottom: 1px solid #2d2d44; }\n.ok { color: #22c55e; } .warn { color: #f59e0b; } .err { color: #ef4444; }\n</style>\n'@\n\n$os = Get-CimInstance Win32_OperatingSystem\n$cs = Get-CimInstance Win32_ComputerSystem\n$cpu = Get-CimInstance Win32_Processor\n$gpu = Get-CimInstance Win32_VideoController | Select-Object -First 1\n$disks = Get-CimInstance Win32_LogicalDisk | Where-Object {$_.DriveType -eq 3}\n$ram = Get-CimInstance Win32_PhysicalMemory\n$net = Get-NetAdapter | Where-Object {$_.Status -eq 'Up'}\n\n$html = @\"\n<!DOCTYPE html><html><head><meta charset='UTF-8'><title>Rapport Système</title>$css</head><body>\n<h1>Rapport Système — $(Get-Date -Format 'dd/MM/yyyy HH:mm')</h1>\n\n<h2>Système</h2>\n<table>\n<tr><th>Propriété</th><th>Valeur</th></tr>\n<tr><td>Ordinateur</td><td>$($cs.Name)</td></tr>\n<tr><td>Fabricant/Modèle</td><td>$($cs.Manufacturer) $($cs.Model)</td></tr>\n<tr><td>OS</td><td>$($os.Caption) Build $($os.BuildNumber)</td></tr>\n<tr><td>Utilisateur</td><td>$env:USERNAME</td></tr>\n<tr><td>Uptime</td><td>$([math]::Round(((Get-Date) - $os.LastBootUpTime).TotalHours, 1)) heures</td></tr>\n</table>\n\n<h2>CPU</h2>\n<table>\n<tr><th>Propriété</th><th>Valeur</th></tr>\n<tr><td>Processeur</td><td>$($cpu.Name.Trim())</td></tr>\n<tr><td>Cœurs physiques</td><td>$($cpu.NumberOfCores)</td></tr>\n<tr><td>Threads logiques</td><td>$($cpu.NumberOfLogicalProcessors)</td></tr>\n<tr><td>Fréquence Max</td><td>$($cpu.MaxClockSpeed) MHz</td></tr>\n</table>\n\"@\n\n# RAM\n$ramTotal = [math]::Round(($ram | Measure-Object -Property Capacity -Sum).Sum / 1GB, 1)\n$html += \"<h2>Mémoire RAM ($ramTotal GB total)</h2><table><tr><th>Slot</th><th>Capacité</th><th>Vitesse</th><th>Fabricant</th></tr>\"\nforeach ($stick in $ram) {\n    $cap = [math]::Round($stick.Capacity / 1GB, 0)\n    $html += \"<tr><td>$($stick.BankLabel)</td><td>$cap GB</td><td>$($stick.Speed) MHz</td><td>$($stick.Manufacturer)</td></tr>\"\n}\n$html += '</table>'\n\n# Disques\n$html += '<h2>Stockage</h2><table><tr><th>Lecteur</th><th>Total</th><th>Libre</th><th>Utilisation</th></tr>'\nforeach ($disk in $disks) {\n    $total = [math]::Round($disk.Size / 1GB, 0)\n    $free = [math]::Round($disk.FreeSpace / 1GB, 1)\n    $pct = [math]::Round(($disk.Size - $disk.FreeSpace) / $disk.Size * 100, 0)\n    $class = if ($pct -gt 90) { 'err' } elseif ($pct -gt 75) { 'warn' } else { 'ok' }\n    $html += \"<tr><td>$($disk.DeviceID)</td><td>$total GB</td><td class='$class'>$free GB</td><td class='$class'>$pct%</td></tr>\"\n}\n$html += '</table></body></html>'\n\n$html | Out-File -FilePath $OutputPath -Encoding UTF8\nStart-Process $OutputPath\nWrite-Host \"Rapport généré: $OutputPath\" -ForegroundColor Green",
        note: "Le rapport s'ouvre automatiquement dans le navigateur. Utile pour les clients ou pour garder un historique de configuration.",
      },
      {
        title: "Script de surveillance de santé système",
        solution: [
          "Vérifie les seuils critiques et envoie des alertes dans le journal d'événements",
          "Vérifications : espace disque, RAM, CPU, services critiques",
          "Peut être planifié toutes les 5-15 minutes via le Planificateur de tâches",
        ],
        code: "# HealthCheck.ps1 — Surveillance santé système\nparam(\n    [int]$DiskThresholdPct = 90,   # Alerte si disque > 90% plein\n    [int]$RamThresholdPct = 90,    # Alerte si RAM > 90% utilisée\n    [string[]]$CriticalServices = @('wuauserv','bits','Schedule','EventLog')\n)\n\n$alerts = @()\n\n# Vérifier les disques\nGet-CimInstance Win32_LogicalDisk | Where-Object {$_.DriveType -eq 3} | ForEach-Object {\n    $pct = [math]::Round(($_.Size - $_.FreeSpace) / $_.Size * 100)\n    if ($pct -ge $DiskThresholdPct) {\n        $freeMB = [math]::Round($_.FreeSpace / 1MB)\n        $alerts += \"DISQUE $($_.DeviceID): $pct% utilisé, $freeMB MB restants\"\n        Write-Warning \"Disque $($_.DeviceID) presque plein: $pct%\"\n    }\n}\n\n# Vérifier la RAM\n$os = Get-CimInstance Win32_OperatingSystem\n$ramUsedPct = [math]::Round(($os.TotalVisibleMemorySize - $os.FreePhysicalMemory) / $os.TotalVisibleMemorySize * 100)\nif ($ramUsedPct -ge $RamThresholdPct) {\n    $alerts += \"RAM: $ramUsedPct% utilisée\"\n    Write-Warning \"RAM critique: $ramUsedPct% utilisée\"\n}\n\n# Vérifier les services critiques\nforeach ($svcName in $CriticalServices) {\n    $svc = Get-Service -Name $svcName -ErrorAction SilentlyContinue\n    if ($svc -and $svc.Status -ne 'Running') {\n        $alerts += \"SERVICE $svcName: $($svc.Status)\"\n        Write-Warning \"Service $svcName non démarré: $($svc.Status)\"\n    }\n}\n\n# Résumé\nif ($alerts.Count -eq 0) {\n    Write-Host '[OK] Tous les contrôles sont au vert' -ForegroundColor Green\n} else {\n    Write-Host \"[ALERTE] $($alerts.Count) problème(s) détecté(s):\" -ForegroundColor Red\n    $alerts | ForEach-Object { Write-Host \"  - $_\" -ForegroundColor Yellow }\n    \n    # Logger dans l'Event Viewer\n    $alertMsg = $alerts -join \"`n\"\n    Write-EventLog -LogName Application -Source 'HealthCheck' -EventId 9999 -EntryType Warning -Message $alertMsg -ErrorAction SilentlyContinue\n}",
        note: "Planifier via : Register-ScheduledTask ou Task Scheduler GUI. Exécuter sous SYSTEM pour accéder à toutes les données.",
      },
      {
        title: "Sauvegarde automatique avec rotation (script complet)",
        solution: [
          "Sauvegarde compressée avec date/heure dans le nom",
          "Rotation automatique : suppression des sauvegardes au-delà de N jours",
          "Log des opérations avec horodatage",
        ],
        code: "# AutoBackup.ps1 — Sauvegarde avec rotation\nparam(\n    [string]$SourcePath = \"$env:USERPROFILE\\Documents\",\n    [string]$BackupPath = 'D:\\Backups',\n    [int]$RetentionDays = 30,\n    [string]$LogFile = 'D:\\Backups\\backup.log'\n)\n\nfunction Write-Log($message) {\n    $entry = \"[$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')] $message\"\n    Add-Content -Path $LogFile -Value $entry\n    Write-Host $entry\n}\n\n# Créer le dossier de backup si nécessaire\nif (-not (Test-Path $BackupPath)) {\n    New-Item -ItemType Directory -Path $BackupPath | Out-Null\n}\n\nWrite-Log '=== Début de la sauvegarde ==='\n\ntry {\n    $timestamp = Get-Date -Format 'yyyyMMdd_HHmmss'\n    $backupFile = Join-Path $BackupPath \"Backup_$timestamp.zip\"\n    \n    # Vérifier que la source existe\n    if (-not (Test-Path $SourcePath)) {\n        throw \"Source introuvable: $SourcePath\"\n    }\n    \n    # Créer la sauvegarde\n    Compress-Archive -Path $SourcePath -DestinationPath $backupFile -CompressionLevel Optimal -ErrorAction Stop\n    \n    $size = [math]::Round((Get-Item $backupFile).Length / 1MB, 2)\n    Write-Log \"Sauvegarde créée: $backupFile ($size MB)\"\n    \n    # Rotation : supprimer les vieilles sauvegardes\n    $cutoff = (Get-Date).AddDays(-$RetentionDays)\n    $oldBackups = Get-ChildItem -Path $BackupPath -Filter 'Backup_*.zip' | Where-Object {$_.LastWriteTime -lt $cutoff}\n    \n    foreach ($old in $oldBackups) {\n        Remove-Item -Path $old.FullName -Force\n        Write-Log \"Ancienne sauvegarde supprimée: $($old.Name)\"\n    }\n    \n    Write-Log \"=== Sauvegarde terminée. $($oldBackups.Count) ancien(s) fichier(s) supprimé(s) ===\"\n\n} catch {\n    Write-Log \"ERREUR: $($_.Exception.Message)\"\n    exit 1\n}",
        note: "Modifier $SourcePath pour cibler les dossiers importants. Prévoir un disque externe ou réseau comme $BackupPath.",
      },
    ],
  },
  {
    id: "scripts-administration",
    label: "Scripts PowerShell — Administration",
    icon: "Terminal",
    items: [
      {
        title: "Gestion des utilisateurs locaux",
        code: "# === Gestion des utilisateurs locaux ===\n\n# Lister tous les utilisateurs\nGet-LocalUser | Select-Object Name, Enabled, LastLogon, PasswordLastSet, Description\n\n# Créer un utilisateur\n$password = ConvertTo-SecureString 'MotDePasseForт!' -AsPlainText -Force\nNew-LocalUser -Name 'TechnicienLocal' -Password $password -Description 'Compte technicien' -FullName 'Technicien'\n\n# Ajouter à un groupe\nAdd-LocalGroupMember -Group 'Administrateurs' -Member 'TechnicienLocal'\nAdd-LocalGroupMember -Group 'Utilisateurs du Bureau à distance' -Member 'TechnicienLocal'\n\n# Modifier le mot de passe\n$newPwd = ConvertTo-SecureString 'NouveauMDP2024!' -AsPlainText -Force\nSet-LocalUser -Name 'TechnicienLocal' -Password $newPwd\n\n# Désactiver un compte\nDisable-LocalUser -Name 'TechnicienLocal'\n\n# Activer un compte\nEnable-LocalUser -Name 'TechnicienLocal'\n\n# Supprimer un utilisateur\nRemove-LocalUser -Name 'TechnicienLocal' -Confirm:$false\n\n# Lister les membres d'un groupe\nGet-LocalGroupMember -Group 'Administrateurs'\n\n# Voir les dernières connexions\nGet-WinEvent -LogName Security -MaxEvents 50 | Where-Object {$_.Id -eq 4624} | Select-Object -First 10 TimeCreated, Message",
        solution: [
          "Ces commandes nécessitent des droits administrateur",
          "Toujours utiliser des mots de passe forts (majuscule, minuscule, chiffre, symbole, 12+ caractères)",
          "Les comptes désactivés ne peuvent pas se connecter mais leurs données sont conservées",
        ],
        note: "Sur un domaine Active Directory, utiliser les cmdlets du module ActiveDirectory (Get-ADUser, New-ADUser etc.) à la place.",
      },
      {
        title: "Audit des droits et permissions de fichiers",
        code: "# === Audit des permissions de fichiers/dossiers ===\n\n# Voir les permissions NTFS d'un dossier\n$path = 'C:\\Données'\n(Get-Acl $path).Access | Select-Object IdentityReference, FileSystemRights, AccessControlType\n\n# Trouver tous les dossiers avec droits 'Everyone'\nGet-ChildItem 'C:\\' -Recurse -ErrorAction SilentlyContinue | ForEach-Object {\n    $acl = Get-Acl $_.FullName -ErrorAction SilentlyContinue\n    if ($acl) {\n        $everyoneRules = $acl.Access | Where-Object {$_.IdentityReference -match 'Everyone|Tout le monde'}\n        if ($everyoneRules) {\n            [PSCustomObject]@{\n                Path = $_.FullName\n                Rights = ($everyoneRules | Select-Object -ExpandProperty FileSystemRights) -join ', '\n            }\n        }\n    }\n} | Export-Csv 'C:\\audit_permissions.csv' -NoTypeInformation\n\n# Modifier les permissions d'un dossier\n$acl = Get-Acl 'C:\\Données'\n$rule = New-Object System.Security.AccessControl.FileSystemAccessRule('Jean', 'ReadAndExecute', 'ContainerInherit,ObjectInherit', 'None', 'Allow')\n$acl.SetAccessRule($rule)\nSet-Acl -Path 'C:\\Données' -AclObject $acl\n\n# Prendre possession d'un fichier verrouillé\ntakeown /f 'C:\\Fichier\\Verrouillé.txt' /r /d y\nicacls 'C:\\Fichier\\Verrouillé.txt' /grant Administrateurs:F",
        solution: [
          "GetAcl retourne l'objet ACL complet avec toutes les règles d'accès",
          "takeown + icacls sont les commandes CMD pour récupérer l'accès aux fichiers système verrouillés",
          "L'audit des permissions every = risque de sécurité, souvent laissé par des logiciels mal configurés",
        ],
        note: "icacls /grant et takeown nécessitent des droits admin. Utiles pour récupérer l'accès aux profils corrompus ou fichiers système.",
      },
      {
        title: "Script de déploiement — configurer un nouveau PC",
        solution: [
          "Script automatisant la configuration initiale d'un nouveau PC Windows",
          "Installe les logiciels de base via Winget, configure les paramètres courants",
          "Adapter la liste des apps selon vos besoins standards",
        ],
        code: "# NewPC-Setup.ps1 — Configuration initiale d'un nouveau PC\n#Requires -RunAsAdministrator\n\nWrite-Host '=== Configuration Nouveau PC ===' -ForegroundColor Cyan\n\n# 1. Mise à jour Windows\nWrite-Host '[1/5] Vérification mises à jour...' -ForegroundColor Yellow\nwuauclt /detectnow /updatenow\n\n# 2. Installer les apps via Winget\nWrite-Host '[2/5] Installation des applications...' -ForegroundColor Yellow\n$apps = @(\n    '7zip.7zip',\n    'VideoLAN.VLC',\n    'Google.Chrome',\n    'Mozilla.Firefox',\n    'Notepad++.Notepad++',\n    'Microsoft.VisualStudioCode',\n    'Git.Git',\n    'Malwarebytes.Malwarebytes',\n    'REALiX.HWiNFO',\n    'CrystalDewWorld.CrystalDiskInfo'\n)\n\nforeach ($app in $apps) {\n    Write-Host \"  Installation: $app\"\n    winget install -e --id $app --accept-package-agreements --accept-source-agreements --silent 2>$null\n}\n\n# 3. Paramètres Windows\nWrite-Host '[3/5] Configuration Windows...' -ForegroundColor Yellow\n\n# Extensions de fichiers visibles\nSet-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced' HideFileExt 0\n\n# Fichiers cachés visibles\nSet-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced' Hidden 1\n\n# Plan d'alimentation : Haute performance\npowercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c\n\n# Désactiver les sons système agaçants\nSet-ItemProperty 'HKCU:\\AppEvents\\Schemes' '(Default)' '.None'\n\n# 4. Sécurité\nWrite-Host '[4/5] Configuration sécurité...' -ForegroundColor Yellow\n# Activer Defender\nSet-MpPreference -DisableRealtimeMonitoring $false\nUpdate-MpSignature\n\n# 5. Rapport final\nWrite-Host '[5/5] Génération du rapport...' -ForegroundColor Yellow\n$report = @{\n    ComputerName = $env:COMPUTERNAME\n    OS = (Get-CimInstance Win32_OperatingSystem).Caption\n    ConfigDate = Get-Date -Format 'dd/MM/yyyy HH:mm'\n    AppsInstalled = $apps.Count\n}\n\nWrite-Host '`n=== Configuration terminée ===' -ForegroundColor Green\n$report | Format-List",
        note: "Modifier la liste $apps avec les applications spécifiques à votre environnement. Ajouter les logiciels métier en fin de liste.",
      },
      {
        title: "Monitoring réseau — alertes de disponibilité",
        code: "# NetworkMonitor.ps1 — Surveillance de la disponibilité réseau\nparam(\n    [string[]]$Targets = @('8.8.8.8', '1.1.1.1', 'google.com'),\n    [int]$IntervalSeconds = 30,\n    [int]$AlertThreshold = 3  # Alerter après 3 échecs consécutifs\n)\n\n$failures = @{}\n$Targets | ForEach-Object { $failures[$_] = 0 }\n\nWrite-Host \"Monitoring démarré. Cibles: $($Targets -join ', ')\" -ForegroundColor Cyan\nWrite-Host \"Vérification toutes les $IntervalSeconds secondes. Ctrl+C pour arrêter.\"\n\nwhile ($true) {\n    $timestamp = Get-Date -Format 'HH:mm:ss'\n    \n    foreach ($target in $Targets) {\n        $result = Test-Connection -ComputerName $target -Count 1 -ErrorAction SilentlyContinue\n        \n        if ($result) {\n            if ($failures[$target] -ge $AlertThreshold) {\n                Write-Host \"[$timestamp] RÉCUPÉRÉ: $target (latence: $($result.ResponseTime)ms)\" -ForegroundColor Green\n            }\n            $failures[$target] = 0\n        } else {\n            $failures[$target]++\n            \n            if ($failures[$target] -ge $AlertThreshold) {\n                Write-Host \"[$timestamp] ALERTE: $target inaccessible ($($failures[$target]) échecs)\" -ForegroundColor Red\n                \n                # Log dans Event Viewer\n                try {\n                    Write-EventLog -LogName Application -Source 'NetworkMonitor' -EventId 9001 -EntryType Error -Message \"$target inaccessible\"\n                } catch { }\n            } else {\n                Write-Host \"[$timestamp] ERREUR $($failures[$target])/$AlertThreshold: $target\" -ForegroundColor Yellow\n            }\n        }\n    }\n    \n    Start-Sleep -Seconds $IntervalSeconds\n}",
        solution: [
          "Ce script tourne en continu et alerte quand une cible devient inaccessible",
          "Laisser tourner en arrière-plan dans une fenêtre PowerShell minimisée",
          "Peut être combiné avec un script email pour des alertes par mail",
        ],
        note: "Pour des alertes email : utiliser Send-MailMessage (SMTP interne) ou la librairie MailKit pour Gmail/SMTP avec authentification moderne.",
      },
    ],
  },
  {
    id: "cmd-avance",
    label: "CMD & Scripts Batch — Guide Avancé",
    icon: "Terminal",
    items: [
      {
        title: "Commandes CMD indispensables pour technicien",
        code: ":: === Commandes CMD indispensables ===\n\n:: Informations système\nsysteminfo                    :: Infos complètes OS/matériel\nmsinfo32                     :: GUI informations système\nwinver                       :: Version Windows\nwhoami /all                  :: Infos utilisateur complet et groupes\n\n:: Réseau\nnetstat -ano                 :: Connexions actives + PID\nipconfig /all                :: Config réseau complète\nnslookup google.com 8.8.8.8  :: Test DNS avec serveur spécifique\ntracert google.com           :: Tracé de route\npathping google.com          :: Tracé + latence par saut\nnet use                      :: Lecteurs réseau mappés\nnet share                    :: Partages réseau locaux\n\n:: Disques et fichiers\ndiskpart                     :: Gestion avancée des disques (interactif)\nchkdsk C: /f /r              :: Vérification et réparation disque\ndism /online /cleanup-image /restorehealth  :: Réparation image Windows\nsfc /scannow                 :: Vérification fichiers système\n\n:: Processus et services\ntasklist /v                  :: Liste processus verbeux\ntaskkill /f /im notepad.exe  :: Tuer un processus par nom\ntaskkill /f /pid 1234        :: Tuer un processus par PID\nsc query                     :: Services et leur état\nsc start NomService          :: Démarrer un service\nsc stop NomService           :: Arrêter un service\n\n:: Registre\nreg query HKLM\\\\SOFTWARE\\\\..  :: Lire une clé de registre\nreg add HKCU\\\\... /v nom /d valeur  :: Ajouter une valeur\nreg export HKCU C:\\\\backup.reg       :: Exporter une branche",
        solution: [
          "systeminfo est lent mais exhaustif — rediriger vers fichier : systeminfo > C:\\sysinfo.txt",
          "netstat -ano | findstr :3389 : trouver les connexions sur le port RDP",
          "diskpart nécessite un admin et est interactif : utiliser avec précaution",
        ],
        note: "CMD est toujours utile pour certaines commandes legacy et les scripts batch. PowerShell est plus puissant mais CMD reste universel.",
      },
      {
        title: "Script Batch — Template de base",
        code: "@echo off\n:: MonScript.bat — Template script batch\n:: Description: Exemple de script batch avec bonnes pratiques\n\n:: Vérifier droits administrateur\nnet session >nul 2>&1\nif %errorLevel% neq 0 (\n    echo Ce script requiert des droits administrateur.\n    echo Clic droit -^> Executer en tant qu'administrateur\n    pause\n    exit /b 1\n)\n\n:: Variables\nset \"VERSION=1.0\"\nset \"LOG_FILE=C:\\Logs\\script_%date:~-4,4%%date:~-7,2%%date:~-10,2%.log\"\n\n:: Créer dossier logs si nécessaire\nif not exist \"C:\\Logs\" mkdir \"C:\\Logs\"\n\n:: Fonction de log\n:log\necho [%date% %time%] %~1 >> \"%LOG_FILE%\"\necho %~1\ngoto :eof\n\n:: === Début du script ===\ncall :log \"Debut du script v%VERSION%\"\n\n:: Exemple : vider le cache DNS\ncall :log \"Vider le cache DNS...\"\nipconfig /flushdns\nif %errorlevel% equ 0 (\n    call :log \"Cache DNS vide avec succes\"\n) else (\n    call :log \"ERREUR: Impossible de vider le cache DNS\"\n)\n\n:: Exemple : démarrer un service\ncall :log \"Demarrage service Spooler...\"\nnet start Spooler >nul 2>&1\n\n:: Résultat final\ncall :log \"Script termine\"\necho.\necho Script termine. Voir le log: %LOG_FILE%\npause",
        solution: [
          "Toujours vérifier les droits admin si le script les requiert",
          "Utiliser des guillemets autour des chemins avec espaces : set \"PATH=C:\\Mon Dossier\"",
          "if %errorlevel% equ 0 pour vérifier le succès de chaque commande critique",
          "La redirection >nul 2>&1 masque la sortie tout en capturant le code de retour",
        ],
        note: "Les scripts batch sont hérités de MS-DOS. Pour de nouveaux projets, préférer PowerShell. Le batch est utile pour les environnements avec restrictions PS.",
      },
      {
        title: "Get-CimInstance — Interrogation système avancée (remplace WMIC)",
        code: "# === Équivalents PowerShell des anciennes commandes WMIC ===\n# WMIC est RETIRÉ par défaut de Windows 11 24H2/25H2 : utiliser Get-CimInstance\n\n# CPU\nGet-CimInstance Win32_Processor | Format-List Name, MaxClockSpeed, NumberOfCores, NumberOfLogicalProcessors\n\n# RAM\nGet-CimInstance Win32_PhysicalMemory | Format-List Capacity, Speed, Manufacturer, BankLabel\n\n# Disques\nGet-PhysicalDisk | Format-List FriendlyName, Size, HealthStatus, MediaType\nGet-CimInstance Win32_LogicalDisk | Format-Table DeviceID, Size, FreeSpace, VolumeName\n\n# Cartes réseau\nGet-CimInstance Win32_NetworkAdapter -Filter 'NetEnabled=True' | Format-List Name, MACAddress, Speed\n\n# Produit Windows (clé OEM du BIOS)\n(Get-CimInstance SoftwareLicensingService).OA3xOriginalProductKey\n\n# Logiciels installés (rapide, sans Win32_Product)\nGet-ItemProperty HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\* | Select DisplayName, DisplayVersion, Publisher\n\n# Processus en cours\nGet-CimInstance Win32_Process | Format-Table Name, ProcessId, WorkingSetSize\n\n# Services démarrés\nGet-CimInstance Win32_Service -Filter \"State='Running'\" | Format-Table Name, DisplayName, StartMode\n\n# Exporter en CSV\nGet-CimInstance Win32_Processor | Export-Csv cpu_info.csv -NoTypeInformation\nGet-CimInstance Win32_PhysicalMemory | Export-Csv ram_info.csv -NoTypeInformation",
        solution: [
          "WMIC est déprécié depuis Windows 10 21H1 et n'est plus installé par défaut sur Windows 11 24H2/25H2",
          "Il peut être réinstallé comme fonctionnalité facultative (Paramètres > Système > Fonctionnalités facultatives > WMIC), mais Microsoft prévoit sa suppression complète",
          "Win32_Product (ancien « wmic product ») est TRÈS lent et déclenche une vérification de tous les MSI : lire le registre Uninstall à la place",
        ],
        note: "Tous les anciens scripts WMIC se traduisent en Get-CimInstance <classe> : mêmes classes WMI (Win32_Processor, Win32_DiskDrive…), même résultat.",
      },
    ],
  },
];
