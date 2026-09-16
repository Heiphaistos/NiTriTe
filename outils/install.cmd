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
::
:: Ne PAS decouper cette ligne en jetons separes par des espaces : le nom de la
:: valeur par defaut est TRADUIT (« (par defaut) » en francais, deux jetons ;
:: « (Default) » en anglais, un seul), donc « tokens=2,* » ne tombe pas au meme
:: endroit selon la langue de Windows. Le premier jet rendait
:: « REG_SZ    C:\Program Files\Nitrite », un chemin invalide, et robocopy
:: refusait tout avec le code 16.
:: `REG_SZ`, lui, n'est jamais traduit : on coupe la ligne dessus.
set "DEST="
set "LIGNE="
for /f "delims=" %%A in ('reg query "HKLM\Software\nitrite\Nitrite" /ve 2^>nul ^| findstr /C:"REG_SZ"') do set "LIGNE=%%A"
if defined LIGNE (
    set "DEST=!LIGNE:*REG_SZ=!"
    rem Retire les espaces de tete laisses par reg query.
    for /f "tokens=* delims= " %%B in ("!DEST!") do set "DEST=%%B"
)
if not defined DEST set "DEST=%ProgramFiles%\Nitrite"
if not exist "!DEST!\nitrite.exe" (
    echo [ERREUR] Dossier d'installation introuvable : !DEST!
    pause & exit /b 1
)
echo --- Dossier d'installation : !DEST!

:: ── Contenu depose a cote de l'application ──────────────────────────────────
:: robocopy /MOVE : instantane quand la source et la destination sont sur le
:: meme volume (cas courant : %TEMP% et Program Files sont tous deux sur C:),
:: et bascule sur une vraie copie quand ce n'est pas le cas.
set "RATE=0"
for %%D in ("logiciel" "Drivers" "Script Windows") do (
    if exist "%~dp0%%~D" (
        echo --- Mise en place de %%~D ...
        robocopy "%~dp0%%~D" "!DEST!\%%~D" /E /MOVE /NFL /NDL /NJH /NJS /NP >nul
        rem robocopy rend 0 a 7 quand tout va bien, 8 et plus en cas d'echec.
        if !ERRORLEVEL! geq 8 (
            echo [ERREUR] Copie de %%~D echouee ^(robocopy !ERRORLEVEL!^).
            set "RATE=1"
        )
    )
)

:: Une installation amputee ne doit pas se declarer reussie : Nitrite tourne
:: sans ces dossiers, mais il ne fait plus la moitie de ce qu'on attend de lui.
if "!RATE!"=="1" (
    echo.
    echo [ERREUR] L'application est installee mais son contenu est incomplet.
    pause & exit /b 1
)

echo.
echo ============================================================
echo   Installation terminee.
echo   Nitrite : !DEST!\nitrite.exe
echo ============================================================
echo.
timeout /t 5 >nul
exit /b 0
