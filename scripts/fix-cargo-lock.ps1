# Cargoビルドディレクトリのロックを解除するスクリプト
# Usage: .\scripts\fix-cargo-lock.ps1

Write-Host "=== Cargo Build Directory Lock Fix ===" -ForegroundColor Cyan
Write-Host ""

# 1. 実行中のcargoプロセスを終了
Write-Host "1. Terminating cargo processes..." -ForegroundColor Yellow
Get-Process cargo -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1

# 2. rust-analyzerプロセスを終了（オプション）
Write-Host "2. Checking for rust-analyzer processes..." -ForegroundColor Yellow
$rustAnalyzer = Get-Process rust-analyzer -ErrorAction SilentlyContinue
if ($rustAnalyzer) {
    Write-Host "   Found rust-analyzer processes. Consider restarting your IDE." -ForegroundColor Yellow
    Write-Host "   You can stop them with: Get-Process rust-analyzer | Stop-Process -Force" -ForegroundColor Gray
}

# 3. ロックファイルを削除
Write-Host "3. Removing lock files..." -ForegroundColor Yellow
$lockFiles = Get-ChildItem -Path "target" -Recurse -Filter "*.lock" -ErrorAction SilentlyContinue
if ($lockFiles) {
    $lockFiles | Remove-Item -Force -ErrorAction SilentlyContinue
    Write-Host "   Removed $($lockFiles.Count) lock file(s)" -ForegroundColor Green
} else {
    Write-Host "   No lock files found" -ForegroundColor Gray
}

# 4. .cargo-lockを削除
if (Test-Path "target\.cargo-lock") {
    Remove-Item "target\.cargo-lock" -Force -ErrorAction SilentlyContinue
    Write-Host "   Removed .cargo-lock" -ForegroundColor Green
}

Write-Host ""
Write-Host "=== Lock Fix Complete ===" -ForegroundColor Green
Write-Host "You can now try: cargo build" -ForegroundColor Cyan

