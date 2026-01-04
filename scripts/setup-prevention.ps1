# Setup automatic bloat prevention
# Usage: .\scripts\setup-prevention.ps1

Write-Host "=== Setting up Automatic Bloat Prevention ===" -ForegroundColor Cyan
Write-Host ""

# Create scheduled task information (manual setup required)
Write-Host "To set up automatic cleanup, you can:" -ForegroundColor Yellow
Write-Host ""
Write-Host "1. Run manually after builds:" -ForegroundColor Cyan
Write-Host "   .\scripts\prevent-bloat.ps1" -ForegroundColor Gray
Write-Host ""
Write-Host "2. Use the build scripts (automatic cleanup included):" -ForegroundColor Cyan
Write-Host "   .\scripts\build.ps1" -ForegroundColor Gray
Write-Host "   .\scripts\check.ps1" -ForegroundColor Gray
Write-Host ""
Write-Host "3. Set up Windows Task Scheduler (advanced):" -ForegroundColor Cyan
Write-Host "   - Task: Run 'scripts\prevent-bloat.ps1 -AutoClean' weekly" -ForegroundColor Gray
Write-Host "   - Trigger: Weekly, Sunday 2 AM" -ForegroundColor Gray
Write-Host ""
Write-Host "4. Add to your IDE build commands:" -ForegroundColor Cyan
Write-Host "   Post-build command: powershell -File scripts\post-build-cleanup.ps1" -ForegroundColor Gray
Write-Host ""

# Check current configuration
Write-Host "Current configuration check:" -ForegroundColor Yellow

# Check .cargo/config.toml
$configPath = ".cargo\config.toml"
if (Test-Path $configPath) {
    $configContent = Get-Content $configPath -Raw
    if ($configContent -match '\[build\]\s*target\s*=') {
        Write-Host "  ⚠️  WARNING: .cargo/config.toml has build target specified" -ForegroundColor Red
        Write-Host "     This may cause x86_64-pc-windows-msvc duplication" -ForegroundColor Red
    } else {
        Write-Host "  ✓ .cargo/config.toml is properly configured" -ForegroundColor Green
    }
} else {
    Write-Host "  ✓ .cargo/config.toml does not exist (using defaults)" -ForegroundColor Green
}

# Check target size
$targetPath = "target"
if (Test-Path $targetPath) {
    $targetSize = (Get-ChildItem $targetPath -Recurse -ErrorAction SilentlyContinue | 
        Measure-Object -Property Length -Sum).Sum
    $targetSizeGB = [math]::Round($targetSize / 1GB, 2)
    
    if ($targetSizeGB -gt 3) {
        Write-Host "  ⚠️  WARNING: target/ directory is $targetSizeGB GB (recommended: < 3 GB)" -ForegroundColor Yellow
        Write-Host "     Run '.\scripts\prevent-bloat.ps1' to clean up" -ForegroundColor Yellow
    } else {
        Write-Host "  ✓ target/ directory size is acceptable ($targetSizeGB GB)" -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "Setup complete!" -ForegroundColor Green




