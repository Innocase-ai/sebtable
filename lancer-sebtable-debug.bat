@echo off
REM Sebtable - Lancement DEBUG (console visible + log fichier)
cd /d "%~dp0"
echo ========================================
echo   Sebtable - DEBUG
echo   Console restera ouverte. Log -> sebtable-debug.log
echo ========================================
echo.
echo Tape F12 dans l'app pour ouvrir les DevTools (si active)
echo.
pnpm tauri dev 2>&1 | tee sebtable-debug.log
echo.
echo --- App fermee. Log sauvegarde dans sebtable-debug.log ---
pause
