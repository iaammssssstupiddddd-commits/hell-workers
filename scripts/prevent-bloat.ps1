# Prevent target directory from growing too large
# Usage: .\scripts\prevent-bloat.ps1 [options]
# This script can be run periodically or added to git hooks

param(
    [int]$MaxSizeGB = 3,
    [int]$CleanupDays = 7,
    [switch]$AutoClean,
    [string]$TargetDir = "target"
)

$maxSizeBytes = $MaxSizeGB * 1GB

function Get-DirectorySize {
    param([string]$Path)
    if (-not (Test-Path $Path)) { return 0 }
    return (Get-ChildItem -Path $Path -Recurse -ErrorAction SilentlyContinue | 
        Measure-Object -Property Length -Sum).Sum
}

Write-Host "=== Target Directory Bloat Prevention ===" -ForegroundColor Cyan

# Check current size
$currentSize = Get-DirectorySize -Path $TargetDir
$currentSizeGB = [math]::Round($currentSize / 1GB, 2)

Write-Host "Current size: $currentSizeGB GB" -ForegroundColor Yellow
Write-Host "Maximum size: $MaxSizeGB GB" -ForegroundColor Gray
Write-Host ""

# If size exceeds threshold, suggest cleanup
if ($currentSize -gt $maxSizeBytes) {
    $excessGB = [math]::Round(($currentSize - $maxSizeBytes) / 1GB, 2)
    Write-Host "⚠️  WARNING: Target directory exceeds $MaxSizeGB GB limit by $excessGB GB" -ForegroundColor Red
    Write-Host ""
    
    if ($AutoClean) {
        Write-Host "Auto-cleanup enabled. Cleaning up..." -ForegroundColor Yellow
    } else {
        Write-Host "Run cleanup? Options:" -ForegroundColor Yellow
        Write-Host "  1. Automatic cleanup (recommended)" -ForegroundColor Cyan
        Write-Host "  2. Manual cleanup with prompts" -ForegroundColor Cyan
        Write-Host "  3. Skip" -ForegroundColor Cyan
        $choice = Read-Host "Choice (1/2/3)"
    }
    
    if ($AutoClean -or $choice -eq "1" -or $choice -eq "2") {
        # Clean up cross-compilation artifacts first (safest)
        $x86Path = Join-Path $TargetDir "x86_64-pc-windows-msvc"
        if (Test-Path $x86Path) {
            $x86Size = Get-DirectorySize -Path $x86Path
            if ($x86Size -gt 0) {
                Write-Host "`nRemoving cross-compilation artifacts..." -ForegroundColor Yellow
                Remove-Item -Path $x86Path -Recurse -Force
                Write-Host "  ✓ Removed $([math]::Round($x86Size / 1GB, 2)) GB" -ForegroundColor Green
                $currentSize -= $x86Size
            }
        }
        
        # Clean old incremental caches
        $cutoffDate = (Get-Date).AddDays(-$CleanupDays)
        foreach ($incPath in @(
            (Join-Path $TargetDir "debug" "incremental"),
            (Join-Path $TargetDir "x86_64-pc-windows-msvc" "debug" "incremental")
        )) {
            if (Test-Path $incPath) {
                $oldCaches = Get-ChildItem -Path $incPath -Directory -ErrorAction SilentlyContinue | 
                    Where-Object { $_.LastWriteTime -lt $cutoffDate }
                
                if ($oldCaches.Count -gt 0) {
                    $oldSize = ($oldCaches | Get-ChildItem -Recurse -ErrorAction SilentlyContinue | 
                        Measure-Object -Property Length -Sum).Sum
                    Write-Host "`nRemoving old incremental caches (older than $CleanupDays days)..." -ForegroundColor Yellow
                    $oldCaches | Remove-Item -Recurse -Force
                    Write-Host "  ✓ Removed $([math]::Round($oldSize / 1MB, 2)) MB" -ForegroundColor Green
                    $currentSize -= $oldSize
                }
            }
        }
        
        # Only clean build/ if still over limit
        if ($currentSize -gt $maxSizeBytes -and ($AutoClean -or $choice -eq "1")) {
            $buildPath = Join-Path $TargetDir "debug" "build"
            if (Test-Path $buildPath) {
                $buildSize = Get-DirectorySize -Path $buildPath
                if ($buildSize -gt 0 -and $currentSize - $buildSize -gt ($maxSizeBytes * 0.5)) {
                    Write-Host "`nRemoving build artifacts (keeping deps/)..." -ForegroundColor Yellow
                    Remove-Item -Path $buildPath -Recurse -Force
                    Write-Host "  ✓ Removed $([math]::Round($buildSize / 1MB, 2)) MB" -ForegroundColor Green
                    Write-Host "  ⚠️  Next build may take longer" -ForegroundColor Yellow
                }
            }
        }
        
        $finalSize = Get-DirectorySize -Path $TargetDir
        $finalSizeGB = [math]::Round($finalSize / 1GB, 2)
        Write-Host "`n✓ Cleanup completed!" -ForegroundColor Green
        Write-Host "  Size after cleanup: $finalSizeGB GB" -ForegroundColor Cyan
    } else {
        Write-Host "Skipped cleanup." -ForegroundColor Yellow
    }
} else {
    Write-Host "✓ Target directory size is within acceptable limits" -ForegroundColor Green
}

Write-Host ""

