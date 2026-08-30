@echo off
REM Sebtable - Lancement 1-clic
REM Double-clic sur ce fichier pour lancer l'app
REM - Mode complet (Tauri + Rust + SQLite) si Rust installe
REM - Sinon fallback web seul (Vite sur http://localhost:1420)

cd /d "%~dp0"
echo ========================================
echo   Sebtable - Lancement
echo ========================================
echo.

REM --- Nettoyage des serveurs de dev orphelins (port 1420) ET de l'app ---
REM Evite qu'un ancien processus Vite (qui sert un frontend obsolete) ou un
REM ancien sebtable.exe (qui affiche l'ancienne interface en memoire) ne reste
REM ouvert. C'est la cause classique du "je modifie mais je ne vois rien".
echo Nettoyage des anciens processus (app + port 1420)...
set "KILLED="
taskkill /F /IM sebtable.exe >nul 2>nul && set "KILLED=1"
for /f "tokens=5" %%a in ('netstat -aon ^| findstr ":1420" ^| findstr "LISTENING"') do (
    echo   - fermeture du processus PID %%a
    taskkill /F /PID %%a >nul 2>nul
    set "KILLED=1"
)
if defined KILLED (
    echo.
    echo Anciens processus fermes. Redemarrage propre...
    timeout /t 1 /nobreak >nul
) else (
    echo   rien a fermer.
)
echo.

where cargo >nul 2>nul
if %errorlevel%==0 (
    echo [Mode complet] Tauri detecte - lancement pnpm tauri dev...
    echo.
    call pnpm tauri dev
) else (
    echo [Mode web seul] Rust non detecte - lancement pnpm dev...
    echo Ouvre http://localhost:1420 dans ton navigateur
    echo.
    call pnpm dev
)

echo.
echo App fermee. Appuie sur une touche pour quitter.
pause >nul
