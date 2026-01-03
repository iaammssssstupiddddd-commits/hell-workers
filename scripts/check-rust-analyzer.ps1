# Check Rust Analyzer installation and configuration
# Usage: .\scripts\check-rust-analyzer.ps1

Write-Host "=== Rust Analyzer Configuration Check ===" -ForegroundColor Cyan
Write-Host ""

$issues = @()
$warnings = @()

# Check if Rust Analyzer extension is installed
Write-Host "1. Rust Analyzer Extension:" -ForegroundColor Yellow

# Check VS Code/Cursor extensions (if possible)
$extensionsPath = "$env:USERPROFILE\.vscode\extensions"
$cursorExtensionsPath = "$env:USERPROFILE\.cursor\extensions"

$found = $false
foreach ($path in @($extensionsPath, $cursorExtensionsPath)) {
    if (Test-Path $path) {
        $rustAnalyzerExt = Get-ChildItem -Path $path -Directory -Filter "*rust-analyzer*" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($rustAnalyzerExt) {
            Write-Host "   ✓ Found: $($rustAnalyzerExt.Name)" -ForegroundColor Green
            Write-Host "     Path: $($rustAnalyzerExt.FullName)" -ForegroundColor Gray
            $found = $true
            break
        }
    }
}

if (-not $found) {
    Write-Host "   ⚠️  Rust Analyzer extension not found in standard locations" -ForegroundColor Yellow
    Write-Host "     This may be normal if using Cursor or different extension manager" -ForegroundColor Gray
    $warnings += "Rust Analyzer extension location could not be verified"
}

Write-Host ""

# Check .vscode/settings.json
Write-Host "2. VS Code/Cursor Settings:" -ForegroundColor Yellow
$vscodeSettings = ".vscode\settings.json"
if (Test-Path $vscodeSettings) {
    Write-Host "   ✓ Found: $vscodeSettings" -ForegroundColor Green
    $content = Get-Content $vscodeSettings -Raw
    
    # Check for rust-analyzer settings
    if ($content -match 'rust-analyzer') {
        Write-Host "   ✓ Rust Analyzer settings found" -ForegroundColor Green
        
        # Check specific settings
        if ($content -match 'rust-analyzer\.files\.excludeDirs') {
            Write-Host "   ✓ excludeDirs configuration found" -ForegroundColor Green
            if ($content -match '"target"') {
                Write-Host "   ✓ target/ is excluded" -ForegroundColor Green
            } else {
                Write-Host "   ⚠️  target/ may not be excluded" -ForegroundColor Yellow
                $issues += "target/ not found in rust-analyzer.files.excludeDirs"
            }
        } else {
            Write-Host "   ⚠️  excludeDirs configuration not found" -ForegroundColor Yellow
            $issues += "Missing rust-analyzer.files.excludeDirs"
        }
    } else {
        Write-Host "   ⚠️  No Rust Analyzer settings found" -ForegroundColor Yellow
        $issues += "No rust-analyzer settings in .vscode/settings.json"
    }
} else {
    Write-Host "   ⚠️  Missing: $vscodeSettings" -ForegroundColor Yellow
    $issues += "Missing .vscode/settings.json"
}

Write-Host ""

# Check if rust-analyzer.toml exists (should not be used)
$rustAnalyzerToml = "rust-analyzer.toml"
if (Test-Path $rustAnalyzerToml) {
    Write-Host "3. rust-analyzer.toml:" -ForegroundColor Yellow
    Write-Host "   ⚠️  Found: $rustAnalyzerToml" -ForegroundColor Yellow
    Write-Host "      Note: This file format is non-standard" -ForegroundColor Gray
    Write-Host "      Rust Analyzer typically uses .vscode/settings.json instead" -ForegroundColor Gray
    $warnings += "rust-analyzer.toml found (non-standard, may not be used)"
} else {
    Write-Host "3. rust-analyzer.toml:" -ForegroundColor Yellow
    Write-Host "   ✓ Not found (expected - using .vscode/settings.json instead)" -ForegroundColor Green
}

Write-Host ""

# Summary
Write-Host "=== Summary ===" -ForegroundColor Cyan

if ($issues.Count -eq 0 -and $warnings.Count -eq 0) {
    Write-Host "✓ All configurations are correct" -ForegroundColor Green
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
Write-Host "=== Recommendations ===" -ForegroundColor Cyan

if ($issues.Count -gt 0) {
    Write-Host "1. Install Rust Analyzer extension:" -ForegroundColor Yellow
    Write-Host "   - Open VS Code/Cursor" -ForegroundColor Gray
    Write-Host "   - Press Ctrl+Shift+X to open Extensions" -ForegroundColor Gray
    Write-Host "   - Search for 'rust-analyzer'" -ForegroundColor Gray
    Write-Host "   - Install the official Rust Analyzer extension" -ForegroundColor Gray
    Write-Host ""
}

Write-Host "2. Verify Rust Analyzer is running:" -ForegroundColor Yellow
Write-Host "   - Open a .rs file" -ForegroundColor Gray
Write-Host "   - Press Ctrl+Shift+P" -ForegroundColor Gray
Write-Host "   - Type 'Rust Analyzer: Restart server'" -ForegroundColor Gray
Write-Host "   - Check the status bar for Rust Analyzer icon" -ForegroundColor Gray
Write-Host ""

Write-Host "3. Check Rust Analyzer output:" -ForegroundColor Yellow
Write-Host "   - View → Output" -ForegroundColor Gray
Write-Host "   - Select 'Rust Analyzer Language Server' from dropdown" -ForegroundColor Gray
Write-Host "   - Look for any error messages" -ForegroundColor Gray



