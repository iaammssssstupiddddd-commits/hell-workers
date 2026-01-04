# Clean cargo target directory to free up disk space
# Usage: .\scripts\clean-target.ps1 [options]

param(
    [switch]$Deep,
    [switch]$All,
    [string]$TargetDir = "target"
)

if (-not (Test-Path $TargetDir)) {
    Write-Host "Target directory does not exist: $TargetDir" -ForegroundColor Yellow
    exit 0
}

# Calculate size before cleanup
$sizeBefore = (Get-ChildItem -Path $TargetDir -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
Write-Host "Current target directory size: $([math]::Round($sizeBefore/1GB, 2)) GB ($([math]::Round($sizeBefore/1MB, 2)) MB)" -ForegroundColor Cyan

if ($All) {
    Write-Host "`nRemoving entire target directory..." -ForegroundColor Yellow
    $response = Read-Host "This will delete ALL build artifacts. Continue? (y/N)"
    if ($response -eq 'y' -or $response -eq 'Y') {
        Remove-Item -Path $TargetDir -Recurse -Force
        Write-Host "Target directory removed successfully!" -ForegroundColor Green
    } else {
        Write-Host "Cancelled." -ForegroundColor Yellow
        exit 0
    }
} elseif ($Deep) {
    Write-Host "`nRunning 'cargo clean' (deep cleanup)..." -ForegroundColor Yellow
    cargo clean
    Write-Host "Deep cleanup completed!" -ForegroundColor Green
} else {
    Write-Host "`nCleaning up incremental build cache and old artifacts..." -ForegroundColor Yellow
    
    # Clean incremental build cache (largest contributor to size)
    $incrementalPath = Join-Path $TargetDir "debug" "incremental"
    if (Test-Path $incrementalPath) {
        $incrementalSize = (Get-ChildItem -Path $incrementalPath -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
        Write-Host "  Incremental cache: $([math]::Round($incrementalSize/1MB, 2)) MB" -ForegroundColor Gray
        Remove-Item -Path $incrementalPath -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "  Removed incremental cache" -ForegroundColor Green
    }
    
    # Clean x86_64-pc-windows-msvc incremental cache
    $x86IncrementalPath = Join-Path $TargetDir "x86_64-pc-windows-msvc" "debug" "incremental"
    if (Test-Path $x86IncrementalPath) {
        $x86IncrementalSize = (Get-ChildItem -Path $x86IncrementalPath -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
        Write-Host "  x86_64 incremental cache: $([math]::Round($x86IncrementalSize/1MB, 2)) MB" -ForegroundColor Gray
        Remove-Item -Path $x86IncrementalPath -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "  Removed x86_64 incremental cache" -ForegroundColor Green
    }
    
    # Clean build script artifacts (often very large)
    $buildPath = Join-Path $TargetDir "debug" "build"
    if (Test-Path $buildPath) {
        $buildSize = (Get-ChildItem -Path $buildPath -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
        Write-Host "  Build artifacts: $([math]::Round($buildSize/1MB, 2)) MB" -ForegroundColor Gray
        
        $response = Read-Host "  Remove build artifacts? This may slow down future builds (y/N)"
        if ($response -eq 'y' -or $response -eq 'Y') {
            Remove-Item -Path $buildPath -Recurse -Force -ErrorAction SilentlyContinue
            Write-Host "  Removed build artifacts" -ForegroundColor Green
        }
    }
}

# Calculate size after cleanup
if (Test-Path $TargetDir) {
    $sizeAfter = (Get-ChildItem -Path $TargetDir -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    $saved = $sizeBefore - $sizeAfter
    Write-Host "`nCleanup completed!" -ForegroundColor Cyan
    Write-Host "  Size before: $([math]::Round($sizeBefore/1GB, 2)) GB" -ForegroundColor Gray
    Write-Host "  Size after:  $([math]::Round($sizeAfter/1GB, 2)) GB" -ForegroundColor Gray
    Write-Host "  Space freed: $([math]::Round($saved/1GB, 2)) GB ($([math]::Round($saved/1MB, 2)) MB)" -ForegroundColor Green
} else {
    $saved = $sizeBefore
    Write-Host "`nCleanup completed!" -ForegroundColor Cyan
    Write-Host "  Space freed: $([math]::Round($saved/1GB, 2)) GB ($([math]::Round($saved/1MB, 2)) MB)" -ForegroundColor Green
}




