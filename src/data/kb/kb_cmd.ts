import type { KBCategory } from "../knowledgeBase";

export const kbCmd: KBCategory[] = [
  {
    id: "cmd",
    label: "Invite de Commandes (CMD)",
    icon: "Terminal",
    items: [
      {
        title: "CMD - Commandes essentielles réseau",
        solution: [
          "ipconfig /all : configuration réseau complète (IP, DNS, MAC, passerelle)",
          "ipconfig /flushdns : vider le cache DNS",
          "ipconfig /release et /renew : renouveler l'adresse IP",
          "ping -t 8.8.8.8 : ping continu (Ctrl+C pour arrêter)",
          "tracert google.com : tracer la route jusqu'à un serveur",
          "netstat -ano : toutes les connexions actives avec PID",
          "nslookup google.com : résolution DNS manuelle",
          "netsh wlan show profiles : voir les profils Wi-Fi enregistrés",
          "netsh wlan show profile name='MonWifi' key=clear : voir mot de passe Wi-Fi",
        ],
        command: "ipconfig /all",
      },
      {
        title: "CMD - Commandes de réparation système",
        solution: [
          "sfc /scannow : scanner et réparer les fichiers système corrompus (ADMIN)",
          "DISM /Online /Cleanup-Image /RestoreHealth : réparer l'image Windows depuis Windows Update",
          "chkdsk C: /f /r : vérifier et réparer le disque (redémarrage requis)",
          "bootrec /fixmbr : réparer le MBR (depuis WinRE)",
          "bootrec /rebuildbcd : reconstruire le BCD (boot configuration)",
          "bcdedit : gérer les entrées de démarrage",
          "Get-PhysicalDisk | Select FriendlyName, HealthStatus (PowerShell) : état des disques (wmic est retiré de Windows 11 24H2/25H2)",
          "systeminfo : informations complètes système",
        ],
        command: "sfc /scannow",
      },
      {
        title: "CMD - Scripts Batch utiles",
        solution: [
          "@echo off en première ligne : masquer les commandes affichées",
          "REM ou :: pour les commentaires",
          "pause : attendre l'appui sur une touche",
          "set VARIABLE=valeur : définir une variable",
          "if '%ERRORLEVEL%'=='0' (...) else (...) : condition",
          "for %%F in (*.txt) do echo %%F : boucle sur fichiers",
          "Créer raccourci de lancement en admin avec runas /user:Administrator",
          "call script.bat : appeler un sous-script depuis un script",
        ],
        code: `@echo off
REM Script de nettoyage simple
title Nettoyage Systeme
echo Nettoyage en cours...

echo Suppression fichiers temporaires...
del /f /s /q %temp%\\* 2>nul

echo Vidage corbeille...
powershell -Command "Clear-RecycleBin -Force"

echo Vider cache DNS...
ipconfig /flushdns

echo Nettoyage termine!
pause`,
      },
    ],
  }
];
