# Check if file watchers are monitoring build artifacts
# Usage: .\scripts\check-watchers.ps1

Write-Host "=== File Watcher Configuration Check ===" -ForegroundColor Cyan
Write-Host ""

$issues = @()

# Check .vscode/settings.json
$vscodeSettings = ".vscode\settings.json"
if (Test-Path $vscodeSettings) {
    Write-Host "✓ Found: $vscodeSettings" -ForegroundColor Green
    $content = Get-Content $vscodeSettings -Raw
    
    if ($content -match 'files\.watcherExclude') {
        if ($content -match '"\*\*/target') {
            Write-Host "  ✓ target/ is excluded from file watcher" -ForegroundColor Green
        } else {
            Write-Host "  ⚠️  WARNING: target/ may not be excluded" -ForegroundColor Yellow
            $issues += "target/ not excluded in VS Code settings"
        }
    } else {
        Write-Host "  ⚠️  WARNING: files.watcherExclude not found" -ForegroundColor Yellow
        $issues += "Missing files.watcherExclude configuration"
    }
} else {
    Write-Host "⚠️  Missing: $vscodeSettings" -ForegroundColor Yellow
    Write-Host "  Creating recommended configuration..." -ForegroundColor Gray
    $issues += "Missing .vscode/settings.json"
}

Write-Host ""

# Check rust-analyzer.toml (optional - settings are in .vscode/settings.json)
$rustAnalyzerConfig = "rust-analyzer.toml"
if (Test-Path $rustAnalyzerConfig) {
    Write-Host "ℹ️  Found: $rustAnalyzerConfig (non-standard, settings should be in .vscode/settings.json)" -ForegroundColor Cyan
} else {
    Write-Host "✓ rust-analyzer.toml not found (expected - using .vscode/settings.json instead)" -ForegroundColor Green
}

Write-Host ""

# Check .cursorignore
$cursorIgnore = ".cursorignore"
if (Test-Path $cursorIgnore) {
    Write-Host "✓ Found: $cursorIgnore" -ForegroundColor Green
} else {
    Write-Host "⚠️  Missing: $cursorIgnore" -ForegroundColor Yellow
    Write-Host "  Creating recommended configuration..." -ForegroundColor Gray
    $issues += "Missing .cursorignore"
}

Write-Host ""

# Check if target directory is being watched (estimate by file count)
$targetPath = "target"
if (Test-Path $targetPath) {
    $fileCount = (Get-ChildItem -Path $targetPath -Recurse -File -ErrorAction SilentlyContinue | Measure-Object).Count
    Write-Host "Target directory analysis:" -ForegroundColor Cyan
    Write-Host "  Files in target/: $fileCount" -ForegroundColor Gray
    
    if ($fileCount -gt 10000) {
        Write-Host "  ⚠️  WARNING: Large number of files ($fileCount)" -ForegroundColor Red
        Write-Host "     This may cause performance issues if being watched" -ForegroundColor Red
        $issues += "Very large target directory ($fileCount files)"
    } elseif ($fileCount -gt 1000) {
        Write-Host "  ⚠️  CAUTION: Many files ($fileCount)" -ForegroundColor Yellow
        Write-Host "     Ensure target/ is excluded from watchers" -ForegroundColor Yellow
    } else {
        Write-Host "  ✓ Reasonable file count" -ForegroundColor Green
    }
}

Write-Host ""

# Summary
if ($issues.Count -eq 0) {
    Write-Host "=== Summary ===" -ForegroundColor Green
    Write-Host "✓ All recommended configurations are in place" -ForegroundColor Green
    Write-Host "✓ Build artifacts should be excluded from file watchers" -ForegroundColor Green
} else {
    Write-Host "=== Summary ===" -ForegroundColor Yellow
    Write-Host "Found $($issues.Count) issue(s):" -ForegroundColor Yellow
    foreach ($issue in $issues) {
        Write-Host "  - $issue" -ForegroundColor Yellow
    }
    Write-Host ""
    Write-Host "Recommendation: Run this script again after configurations are created" -ForegroundColor Cyan
}

Write-Host ""


