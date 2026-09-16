@echo off
setlocal EnableDelayedExpansion
:: Lance par le SFX Nitrite_<version>_installateur_complet.exe.
::
:: L'installeur NSIS pose l'APPLICATION et rien d'autre : le contenu
:: (logiciel\, Drivers\, Script Windows\) est depose A COTE, volontairement hors
:: de son manifeste. Raison verifiee dans le script NSIS genere : a chaque mise
:: a jour, l'installeur execute le DESINSTALLEUR de la version precedente, qui
:: efface tout ce qu'il avait pose -- « resources » comprises. Un contenu livre
:: en resource serait donc detruit a la premiere mise a jour et jamais remis.
:: Depose a cote, il survit : la desinstallation fait RMDir sans /r.

cd /d "%~dp0"

:: ── Elevation : ecrire dans Program Files la demande ────────────────────────
net session >nul 2>&1
if !ERRORLEVEL! neq 0 (
    echo Elevation necessaire. Relancement en administrateur...
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -WorkingDirectory '%~dp0' -Verb RunAs"
    exit /b
)

echo ============================================================
echo   Installation de Nitrite
echo ============================================================
echo.

set "SETUP="
for %%F in ("%~dp0Nitrite_*_x64-setup.exe") do set "SETUP=%%~fF"
if not defined SETUP (
    echo [ERREUR] Installeur introuvable a cote de ce script.
    pause & exit /b 1
)

echo --- Installation de l'application...
"!SETUP!" /P
if !ERRORLEVEL! neq 0 (
    echo [ERREUR] L'installeur a rendu le code !ERRORLEVEL!.
    pause & exit /b 1
)

:: ── Ou l'installeur s'est-il pose ? ─────────────────────────────────────────
:: NSIS ecrit le dossier dans HKLM\Software\nitrite\Nitrite (valeur par defaut).
set "DEST="
for /f "tokens=2,*" %%A in ('reg query "HKLM\Software\nitrite\Nitrite" /ve 2^>nul ^| findstr /R "REG_SZ"') do set "DEST=%%B"
if not defined DEST set "DEST=%ProgramFiles%\Nitrite"
echo --- Dossier d'installation : !DEST!

:: ── Contenu depose a cote de l'application ──────────────────────────────────
:: robocopy /MOVE : instantane quand la source et la destination sont sur le
:: meme volume (cas courant : %TEMP% et Program Files sont tous deux sur C:),
:: et bascule sur une vraie copie quand ce n'est pas le cas.
for %%D in ("logiciel" "Drivers" "Script Windows") do (
    if exist "%~dp0%%~D" (
        echo --- Mise en place de %%~D ...
        robocopy "%~dp0%%~D" "!DEST!\%%~D" /E /MOVE /NFL /NDL /NJH /NJS /NP >nul
        if !ERRORLEVEL! geq 8 echo [ATTENTION] Copie de %%~D incomplete ^(robocopy !ERRORLEVEL!^).
    )
)

echo.
echo ============================================================
echo   Installation terminee.
echo   Nitrite : !DEST!\nitrite.exe
echo ============================================================
echo.
timeout /t 5 >nul
exit /b 0
