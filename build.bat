@echo off
setlocal EnableDelayedExpansion
cd /d "%~dp0"

:: ============================================================
:: CHANGELOG
:: ============================================================
:: v6.1.0 (2026-04-07) — Audit sécurité complet
::   - PS injection fixes : debloat.rs, extended_info.rs (escape '' correct)
::   - Validation IP/hostname hosts_editor.rs, lettre A-Z advanced_recovery.rs
::   - delete_dll : whitelist System32/SysWOW64/ProgramFiles uniquement
::   - check_portable_installed : guard path traversal (../ \\ /)
::   - lib_diagnostic_extra_a.rs : validation HH:MM tâche planifiée
::   - script_generator.rs : safe_pkg_id + safe_display_name
::   - DiagTabBoot.vue : try/catch sur doRecovery
::   - invokeRaw -> invoke (timeout 15s) sur 6 composants rapides
::   - useCachedInvoke.ts : pendingRequests Map (dédup concurrence)
::   - tauri.conf.json : CSP sans wildcard https:
::   - Suppression dead code : useTauriCommand.ts, useLocalStorage.ts
::   - vue-tsc 0 erreur, cargo check 0 warning
::
:: v6.0.0 (2026-04-04) — Export Scan complet
::   - Export batterie dans tous les formats (TXT/HTML/MD/JSON)
::     avec capacité originale + actuelle en mWh, santé, cycles, chimie
::   - Correction champs manquants : bios_ok, activation_type,
::     office_activation_type, scan_errors
::   - 4 gestionnaires de paquets toujours affichés (WinGet, Chocolatey,
::     Scoop, Windows Update) avec détail des mises à jour disponibles
::     ou "Aucune mise à jour" explicite
::   - TXT : UTF-8 BOM pour affichage correct dans Notepad
::   - DiagTabScan.vue : Chocolatey ajouté, Scoop toujours visible
::   - Deux systèmes d'export synchronisés (useScanExport + useDiagnosticExport)
:: ============================================================

:: ── Version dynamique depuis package.json ────────────────────────────────────
for /f "delims=" %%V in ('powershell -NoProfile -Command "(Get-Content package.json -Raw | ConvertFrom-Json).version"') do set "VERSION=%%V"
if not defined VERSION set "VERSION=unknown"

title Nitrite Build v!VERSION!

:: ── Auto-elevation UAC ───────────────────────────────────────────────────────
net session >nul 2>&1
if !ERRORLEVEL! neq 0 (
    echo --- Elevation UAC necessaire. Relancement en administrateur...
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -WorkingDirectory '%~dp0' -Verb RunAs"
    exit /b
)

echo ============================================================
echo   Nitrite 2.0 - Build Production v!VERSION!  [Administrateur]
echo ============================================================
echo.

set "EXE_SRC=src-tauri\target\release\nitrite.exe"
set "OUT_DIR=release"

:: ── 7-Zip : tools\7za.exe en priorite, installation systeme en fallback ───────
set "SEVENZIP="
if exist "%~dp0tools\x64\7za.exe" set "SEVENZIP=%~dp0tools\x64\7za.exe"
if not defined SEVENZIP (
    if exist "%~dp0tools\7za.exe" set "SEVENZIP=%~dp0tools\7za.exe"
)
if not defined SEVENZIP (
    if exist "C:\Program Files\7-Zip\7z.exe" set "SEVENZIP=C:\Program Files\7-Zip\7z.exe"
)
if not defined SEVENZIP (
    set "_7Z86=C:\Program Files (x86)\7-Zip\7z.exe"
    if exist "!_7Z86!" set "SEVENZIP=!_7Z86!"
)

:: ── Log (timestamp ISO via PowerShell - independant de la locale) ─────────────
if not exist ".logs" mkdir ".logs"
for /f "delims=" %%D in ('powershell -NoProfile -Command "Get-Date -Format yyyyMMdd_HHmmss"') do set "TIMESTAMP=%%D"
set "LOGFILE=.logs\build_v!VERSION!_!TIMESTAMP!.log"
echo [!TIMESTAMP!] [INFO] Build Nitrite v!VERSION! > "!LOGFILE!"

:: ============================================================
:: MENU
:: ============================================================
echo Choisissez le mode de packaging :
echo.
echo   [1] EXE Portable seul (~15 MB)
echo       release\Nitrite_v!VERSION!_portable.exe
echo       (application uniquement, sans logiciel/drivers/scripts)
echo.
echo   [2] Dossier portable complet (~2.5 GB)
echo       release\Nitrite_v!VERSION!\
echo       Contient : Nitrite.exe + logiciel + Drivers + Script Windows
echo.
echo   [3] SFX tout-en-un (~1.5 GB compresse)
echo       release\Nitrite_v!VERSION!_full.exe
echo       Un seul .exe qui extrait tout et lance Nitrite
echo.
echo   [4] ISO WinPE 11 bootable
echo       release\Nitrite_v!VERSION!_WinPE.iso
echo       Cle USB bootable - Necessite Windows ADK + WinPE Add-on
echo.
echo   [0] Quitter
echo.
set /p CHOICE="Votre choix [1/2/3/4/0] : "
echo.

if "!CHOICE!"=="0" exit /b 0
if "!CHOICE!"=="1" goto MODE_EXE_SEUL
if "!CHOICE!"=="2" goto MODE_DOSSIER
if "!CHOICE!"=="3" goto MODE_SFX
if "!CHOICE!"=="4" goto MODE_ISO_WINPE
echo Choix invalide. Relancez le script.
echo [!TIMESTAMP!] [ERROR] Choix invalide : !CHOICE! >> "!LOGFILE!"
pause & exit /b 1

:: ============================================================
:: ETAPES COMMUNES (simulee en subroutine via goto + BUILD_RETURN)
:: ============================================================
:BUILD_COMMON
echo --- Arret des instances en cours...
taskkill /F /IM nitrite.exe >nul 2>&1
timeout /t 1 /nobreak >nul

echo --- Verification des dependances npm...
if not exist "node_modules" (
    call npm install
    if !ERRORLEVEL! neq 0 (
        echo [ERREUR] npm install echoue.
        echo [!TIMESTAMP!] [ERROR] npm install failed >> "!LOGFILE!"
        pause & exit /b 1
    )
) else (
    echo      node_modules OK.
)

echo --- Verification TypeScript (vue-tsc)...
call npx vue-tsc --noEmit >> "!LOGFILE!" 2>&1
if !ERRORLEVEL! neq 0 (
    echo [ERREUR] Erreurs TypeScript detectees. Voir !LOGFILE!
    echo [!TIMESTAMP!] [ERROR] vue-tsc failed >> "!LOGFILE!"
    pause & exit /b 1
)
echo      TypeScript OK.

if not exist "!OUT_DIR!" mkdir "!OUT_DIR!"

echo --- Build Tauri Release (Vue + Rust)...
echo [!TIMESTAMP!] [INFO] npx tauri build --no-bundle >> "!LOGFILE!"
call npx tauri build --no-bundle >> "!LOGFILE!" 2>&1
if !ERRORLEVEL! neq 0 (
    echo [ERREUR] Build Tauri echoue. Voir !LOGFILE!
    echo [!TIMESTAMP!] [ERROR] tauri build failed >> "!LOGFILE!"
    pause & exit /b 1
)
if not exist "!EXE_SRC!" (
    echo [ERREUR] EXE non genere apres build.
    echo [!TIMESTAMP!] [ERROR] EXE manquant apres build >> "!LOGFILE!"
    pause & exit /b 1
)
echo [!TIMESTAMP!] [INFO] Build OK : !EXE_SRC! >> "!LOGFILE!"

goto !BUILD_RETURN!

:: ============================================================
:: MODE 1 - EXE portable seul
:: ============================================================
:MODE_EXE_SEUL
set "BUILD_RETURN=AFTER_EXE_SEUL"
goto BUILD_COMMON
:AFTER_EXE_SEUL
set "DEST=!OUT_DIR!\Nitrite_v!VERSION!_portable.exe"
if exist "!DEST!" del /f /q "!DEST!"
copy /Y "!EXE_SRC!" "!DEST!" >nul
echo [!TIMESTAMP!] [INFO] Mode 1 OK : !DEST! >> "!LOGFILE!"
echo.
echo ============================================================
echo   [1] Build termine !
echo   Fichier : !DEST!
for %%F in ("!DEST!") do echo   Taille  : %%~zF octets
echo ============================================================
echo.
pause & exit /b 0

:: ============================================================
:: MODE 2 - Dossier portable complet
:: ============================================================
:MODE_DOSSIER
set "BUILD_RETURN=AFTER_DOSSIER"
goto BUILD_COMMON
:AFTER_DOSSIER
set "DEST_DIR=!OUT_DIR!\Nitrite_v!VERSION!"
if exist "!DEST_DIR!" rmdir /s /q "!DEST_DIR!"
mkdir "!DEST_DIR!"
echo --- Copie de l'executable...
copy /Y "!EXE_SRC!" "!DEST_DIR!\Nitrite.exe" >nul
echo --- Copie de logiciel\ (peut prendre du temps)...
xcopy /E /I /Q /Y "logiciel" "!DEST_DIR!\logiciel" >nul
echo --- Copie de Drivers\ ...
xcopy /E /I /Q /Y "Drivers" "!DEST_DIR!\Drivers" >nul
echo --- Copie de Script Windows\ ...
xcopy /E /I /Q /Y "Script Windows" "!DEST_DIR!\Script Windows" >nul
echo [!TIMESTAMP!] [INFO] Mode 2 OK : !DEST_DIR! >> "!LOGFILE!"
echo.
echo ============================================================
echo   [2] Build termine !
echo   Dossier : !DEST_DIR!\
echo ============================================================
echo.
pause & exit /b 0

:: ============================================================
:: MODE 3 - SFX tout-en-un via 7-Zip SFX
:: ============================================================
:MODE_SFX
if not defined SEVENZIP (
    echo [ERREUR] Aucun compresseur 7-Zip trouve.
    echo Verifiez que tools\7za.exe est present ou installez 7-Zip.
    pause & exit /b 1
)
echo     Compresseur : !SEVENZIP!

:: ── Recherche du stub SFX ────────────────────────────────────────────────────
set "SFX_STUB="
for %%P in (
    "%~dp0tools\7zSD.sfx"
    "%~dp0tools\7zS.sfx"
    "%~dp0tools\7z.sfx"
    "%~dp0tools\x64\7zSD.sfx"
    "%~dp0tools\x64\7zS.sfx"
) do (
    if exist %%P (
        if not defined SFX_STUB set "SFX_STUB=%%~P"
    )
)

:: Chercher aussi dans le dossier de 7-Zip installe si applicable
if not defined SFX_STUB (
    for %%X in ("!SEVENZIP!") do set "_SZDIR=%%~dpX"
    for %%P in (
        "!_SZDIR!7zSD.sfx"
        "!_SZDIR!7zS.sfx"
        "!_SZDIR!7z.sfx"
    ) do (
        if exist %%P (
            if not defined SFX_STUB set "SFX_STUB=%%~P"
        )
    )
)

if defined SFX_STUB goto SFX_STUB_FOUND

:: -- Stub SFX absent : il n y a plus rien a telecharger ---------------------
:: Le telechargement automatique qui vivait ici etait MORT : 7-Zip ne livre
:: plus 7zSD.sfx dans "7-Zip Extra" (verifie le 2026-09-15 : absent de
:: 7z2603-extra.7z, et 7z2408-extra.7z rend 404 sur 7-zip.org). Il ne pouvait
:: donc que faire perdre deux minutes avant d echouer quand meme.
::
:: Le stub se recupere en copiant les 215040 premiers octets d une release
:: Nitrite_vX_full.exe deja publiee -- tout ce qui precede le marqueur
:: ";!@Install@!UTF-8!" -- ou depuis une archive 7-Zip Extra <= 21.07.
if not defined SFX_STUB (
    echo [ERREUR] tools/7zSD.sfx introuvable.
    echo Recuperez-le depuis une release full.exe deja publiee :
    echo   powershell -NoProfile -Command "[IO.File]::WriteAllBytes('tools/7zSD.sfx', [IO.File]::ReadAllBytes('release/Nitrite_vX_full.exe')[0..215039])"
    pause ^& exit /b 1
)

:SFX_STUB_FOUND
echo     SFX stub : !SFX_STUB!
set "BUILD_RETURN=AFTER_SFX"
goto BUILD_COMMON
:AFTER_SFX

set "TEMP_SFX=%TEMP%\_nitrite_sfx_pack"
set "ARCHIVE=!OUT_DIR!\_nitrite_tmp.7z"
set "CFG_FILE=!OUT_DIR!\_sfx_config.txt"
set "SFX_OUT=!OUT_DIR!\Nitrite_v!VERSION!_full.exe"

echo --- Preparation du contenu SFX...
if exist "!TEMP_SFX!" rmdir /s /q "!TEMP_SFX!"
mkdir "!TEMP_SFX!"
copy /Y "!EXE_SRC!" "!TEMP_SFX!\Nitrite.exe" >nul
echo --- Copie logiciel\ ...
xcopy /E /I /Q /Y "logiciel" "!TEMP_SFX!\logiciel" >nul
echo --- Copie Drivers\ ...
xcopy /E /I /Q /Y "Drivers" "!TEMP_SFX!\Drivers" >nul
echo --- Copie Script Windows\ ...
xcopy /E /I /Q /Y "Script Windows" "!TEMP_SFX!\Script Windows" >nul

:: Desactiver temporairement delayed expansion : les ! dans les marqueurs SFX
:: seraient interpretes comme delimiteurs de variable sinon
set "_VER=!VERSION!"
set "_CFG=!CFG_FILE!"
setlocal DisableDelayedExpansion
(
echo ;!@Install@!UTF-8!
echo Title="Nitrite v%_VER%"
echo BeginPrompt="Voulez-vous extraire Nitrite v%_VER% ?"
echo RunProgram="Nitrite.exe"
echo ;!@InstallEnd@!
) > "%_CFG%"
endlocal

echo --- Compression LZMA2 (peut prendre 10-20 min selon le CPU)...
if exist "!SFX_OUT!" del /f /q "!SFX_OUT!"
if exist "!ARCHIVE!" del /f /q "!ARCHIVE!"
"!SEVENZIP!" a -t7z -m0=lzma2 -mx=5 -mmt=on -ms=on "!ARCHIVE!" "!TEMP_SFX!\*" -y
if !ERRORLEVEL! neq 0 (
    echo [ERREUR] Compression 7-Zip echouee.
    rmdir /s /q "!TEMP_SFX!" >nul 2>&1
    del /Q "!CFG_FILE!" "!ARCHIVE!" >nul 2>&1
    pause & exit /b 1
)

echo --- Assemblage SFX...
copy /b "!SFX_STUB!" + "!CFG_FILE!" + "!ARCHIVE!" "!SFX_OUT!" >nul
if !ERRORLEVEL! neq 0 (
    echo [ERREUR] Assemblage SFX echoue.
    rmdir /s /q "!TEMP_SFX!" >nul 2>&1
    del /Q "!ARCHIVE!" "!CFG_FILE!" >nul 2>&1
    pause & exit /b 1
)

rmdir /s /q "!TEMP_SFX!" >nul 2>&1
del /Q "!ARCHIVE!" "!CFG_FILE!" >nul 2>&1

if not exist "!SFX_OUT!" (
    echo [ERREUR] SFX final non genere.
    pause & exit /b 1
)
echo [!TIMESTAMP!] [INFO] Mode 3 OK : !SFX_OUT! >> "!LOGFILE!"
echo.
echo ============================================================
echo   [3] Build termine !
echo   SFX : !SFX_OUT!
for %%F in ("!SFX_OUT!") do echo   Taille : %%~zF octets
echo ============================================================
echo.
pause & exit /b 0

:: ============================================================
:: MODE 4 - ISO WinPE 11 bootable
:: ============================================================
:MODE_ISO_WINPE
echo --- Build de l'exe Nitrite (prerequis pour l'ISO)...
set "BUILD_RETURN=AFTER_ISO_WINPE"
goto BUILD_COMMON
:AFTER_ISO_WINPE
echo [!TIMESTAMP!] [INFO] Mode 4 : lancement build-bootable.bat >> "!LOGFILE!"
echo.
echo --- Lancement du script de creation ISO WinPE...
call "%~dp0boot\build-bootable.bat"
exit /b %ERRORLEVEL%
