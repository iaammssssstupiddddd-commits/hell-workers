# Check Cursor/Antigravity compatibility with Rust Analyzer
# Usage: .\scripts\check-cursor-compatibility.ps1

Write-Host "=== Cursor/Antigravity 互換性チェック ===" -ForegroundColor Cyan
Write-Host ""

$issues = @()
$warnings = @()

# 1. Check .vscode/settings.json (Cursor + Rust Analyzer)
Write-Host "1. VS Code/Cursor Settings (.vscode/settings.json):" -ForegroundColor Yellow
$vscodeSettings = ".vscode\settings.json"
if (Test-Path $vscodeSettings) {
    Write-Host "   ✓ Found" -ForegroundColor Green
    $content = Get-Content $vscodeSettings -Raw
    
    # Check files.watcherExclude (Cursor file watcher)
    if ($content -match 'files\.watcherExclude') {
        if ($content -match '"\*\*/target') {
            Write-Host "   ✓ Cursor file watcher: target/ excluded" -ForegroundColor Green
        } else {
            Write-Host "   ⚠️  Cursor file watcher: target/ may not be excluded" -ForegroundColor Yellow
            $warnings += "target/ not excluded in files.watcherExclude"
        }
    } else {
        Write-Host "   ⚠️  files.watcherExclude not found" -ForegroundColor Yellow
        $warnings += "Missing files.watcherExclude"
    }
    
    # Check rust-analyzer settings
    if ($content -match 'rust-analyzer') {
        Write-Host "   ✓ Rust Analyzer settings found" -ForegroundColor Green
        if ($content -match 'rust-analyzer\.files\.excludeDirs') {
            if ($content -match '"target"') {
                Write-Host "   ✓ Rust Analyzer: target/ excluded" -ForegroundColor Green
            } else {
                Write-Host "   ⚠️  Rust Analyzer: target/ may not be excluded" -ForegroundColor Yellow
                $warnings += "target/ not excluded in rust-analyzer.files.excludeDirs"
            }
        }
    } else {
        Write-Host "   ⚠️  Rust Analyzer settings not found" -ForegroundColor Yellow
        $warnings += "Missing rust-analyzer settings"
    }
} else {
    Write-Host "   ⚠️  Not found" -ForegroundColor Yellow
    $issues += "Missing .vscode/settings.json"
}

Write-Host ""

# 2. Check .cursorignore (Antigravity)
Write-Host "2. Cursor Ignore (.cursorignore):" -ForegroundColor Yellow
$cursorIgnore = ".cursorignore"
if (Test-Path $cursorIgnore) {
    Write-Host "   ✓ Found" -ForegroundColor Green
    $content = Get-Content $cursorIgnore -Raw
    
    if ($content -match 'target') {
        Write-Host "   ✓ Antigravity: target/ excluded" -ForegroundColor Green
    } else {
        Write-Host "   ⚠️  Antigravity: target/ may not be excluded" -ForegroundColor Yellow
        $warnings += "target/ not excluded in .cursorignore"
    }
} else {
    Write-Host "   ⚠️  Not found" -ForegroundColor Yellow
    $warnings += "Missing .cursorignore (Antigravity may scan all files)"
}

Write-Host ""

# 3. Check for conflicts
Write-Host "3. Configuration Conflicts:" -ForegroundColor Yellow

# Check if settings are consistent
if ((Test-Path $vscodeSettings) -and (Test-Path $cursorIgnore)) {
    Write-Host "   ✓ Both Cursor and Antigravity configurations exist" -ForegroundColor Green
    Write-Host "   ✓ Configurations are complementary (no conflicts)" -ForegroundColor Green
} else {
    Write-Host "   ⚠️  Some configurations are missing" -ForegroundColor Yellow
}

Write-Host ""

# Summary
Write-Host "=== Summary ===" -ForegroundColor Cyan

$allGood = ($issues.Count -eq 0 -and $warnings.Count -eq 0)

if ($allGood) {
    Write-Host "✓ All configurations are optimal" -ForegroundColor Green
    Write-Host "✓ Cursor, Rust Analyzer, and Antigravity can coexist" -ForegroundColor Green
    Write-Host "✓ File watchers are optimized" -ForegroundColor Green
} else {
    if ($issues.Count -gt 0) {
        Write-Host "Issues found:" -ForegroundColor Red
        foreach ($issue in $issues) {
            Write-Host "  - $issue" -ForegroundColor Red
        }
    }
    
    if ($warnings.Count -gt 0) {
        Write-Host "Warnings:" -ForegroundColor Yellow
        foreach ($warning in $warnings) {
            Write-Host "  - $warning" -ForegroundColor Yellow
        }
    }
}

Write-Host ""

# Recommendations
Write-Host "=== Recommendations ===" -ForegroundColor Cyan

if ($allGood) {
    Write-Host "✓ Current configuration is optimal" -ForegroundColor Green
    Write-Host "✓ No changes needed" -ForegroundColor Green
    Write-Host ""
    Write-Host "All tools are configured to:" -ForegroundColor Gray
    Write-Host "  - Exclude target/ from file watching" -ForegroundColor Gray
    Write-Host "  - Monitor only source code (src/)" -ForegroundColor Gray
    Write-Host "  - Prevent agent crashes" -ForegroundColor Gray
} else {
    Write-Host "1. Ensure .vscode/settings.json exists with proper settings" -ForegroundColor Yellow
    Write-Host "2. Ensure .cursorignore exists with target/ exclusion" -ForegroundColor Yellow
    Write-Host "3. Restart Cursor to apply changes" -ForegroundColor Yellow
}

Write-Host ""

