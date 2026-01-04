# Setup git hooks to prevent target directory bloat
# Usage: .\scripts\setup-git-hooks.ps1

$gitHooksDir = Join-Path ".git" "hooks"
$preCommitHook = Join-Path $gitHooksDir "pre-commit"
$postCheckoutHook = Join-Path $gitHooksDir "post-checkout"

# Create hooks directory if it doesn't exist
if (-not (Test-Path $gitHooksDir)) {
    New-Item -ItemType Directory -Path $gitHooksDir | Out-Null
}

# Create pre-commit hook (check size before commit)
$preCommitContent = @"
#!/bin/sh
# Check target directory size and warn if too large

MAX_SIZE_GB=3
TARGET_SIZE=\$(du -sh target 2>/dev/null | cut -f1)

if [ -d "target" ]; then
    SIZE_BYTES=\$(du -sb target 2>/dev/null | cut -f1)
    SIZE_GB=\$(echo "scale=2; \$SIZE_BYTES / 1073741824" | bc 2>/dev/null || echo "0")
    
    if [ ! -z "\$SIZE_GB" ] && (( \$(echo "\$SIZE_GB > \$MAX_SIZE_GB" | bc -l 2>/dev/null || echo 0) )); then
        echo ""
        echo "⚠️  WARNING: target/ directory is \${SIZE_GB} GB (limit: \${MAX_SIZE_GB} GB)"
        echo "Run: .\scripts\prevent-bloat.ps1 -AutoClean"
        echo ""
    fi
fi

exit 0
"@

# Create PowerShell version for Windows
$preCommitPsContent = @"
# Pre-commit hook: Check target directory size
`$MaxSizeGB = 3
`$TargetDir = "target"

if (Test-Path `$TargetDir) {
    `$size = (Get-ChildItem -Path `$TargetDir -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    `$sizeGB = [math]::Round(`$size / 1GB, 2)
    
    if (`$sizeGB -gt `$MaxSizeGB) {
        Write-Host ""
        Write-Host "⚠️  WARNING: target/ directory is `$sizeGB GB (limit: `$MaxSizeGB GB)" -ForegroundColor Yellow
        Write-Host "Run: .\scripts\prevent-bloat.ps1 -AutoClean" -ForegroundColor Cyan
        Write-Host ""
    }
}
"@

# Create post-checkout hook (cleanup cross-compilation artifacts)
$postCheckoutPsContent = @"
# Post-checkout hook: Clean up unnecessary build artifacts
`$x86Path = Join-Path "target" "x86_64-pc-windows-msvc"

if (Test-Path `$x86Path) {
    # Only remove if on Windows (native build doesn't need this)
    if (`$IsWindows -or `$env:OS -eq "Windows_NT") {
        Remove-Item -Path `$x86Path -Recurse -Force -ErrorAction SilentlyContinue
    }
}
"@

# Write hooks (Windows PowerShell version)
if ($IsWindows -or $env:OS -eq "Windows_NT") {
    Set-Content -Path $preCommitHook -Value $preCommitPsContent -Encoding UTF8
    Set-Content -Path $postCheckoutHook -Value $postCheckoutPsContent -Encoding UTF8
    
    # Make hooks executable (Unix-like systems, if using Git Bash)
    if (Test-Path "C:\Program Files\Git\bin\bash.exe") {
        & "C:\Program Files\Git\bin\bash.exe" -c "chmod +x .git/hooks/pre-commit .git/hooks/post-checkout" 2>$null
    }
} else {
    Set-Content -Path $preCommitHook -Value $preCommitContent -Encoding UTF8
    Set-Content -Path $postCheckoutHook -Value $postCheckoutPsContent -Encoding UTF8
    & chmod +x $preCommitHook $postCheckoutHook 2>$null
}

Write-Host "✓ Git hooks installed successfully!" -ForegroundColor Green
Write-Host "  - pre-commit: Warns if target/ exceeds 3 GB" -ForegroundColor Gray
Write-Host "  - post-checkout: Cleans up cross-compilation artifacts" -ForegroundColor Gray

