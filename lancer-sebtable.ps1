# Sebtable - Lancement 1-clic (PowerShell)
# Clic droit > Executer avec PowerShell  OU  double-clic si associe
Set-Location $PSScriptRoot
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Sebtable - Lancement" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Verifs
try { $pnpmVer = pnpm --version; Write-Host "pnpm $pnpmVer" -ForegroundColor Green } catch { Write-Host "pnpm non trouve - installe Node.js + pnpm" -ForegroundColor Red; pause; exit 1 }
try { $nodeVer = node --version; Write-Host "node $nodeVer" -ForegroundColor Green } catch {}

$cargoOk = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargoOk) {
    Write-Host "[Mode complet] Tauri detecte (cargo $($cargoOk.Version))" -ForegroundColor Green
    Write-Host "Lancement: pnpm tauri dev ..." -ForegroundColor Yellow
    Write-Host ""
    pnpm tauri dev
} else {
    Write-Host "[Mode web seul] Rust non detecte" -ForegroundColor Yellow
    Write-Host "Lancement: pnpm dev -> http://localhost:1420" -ForegroundColor Yellow
    Write-Host ""
    pnpm dev
}

Write-Host ""
Write-Host "App fermee." -ForegroundColor Cyan
pause
