# Automatic cleanup after build to prevent bloat
# This script should be run after cargo build/check commands
# Usage: .\scripts\post-build-cleanup.ps1 [--max-size-gb 3]

param(
    [int]$MaxSizeGB = 3,
    [string]$TargetDir = "target"
)

$maxSizeBytes = $MaxSizeGB * 1GB

function Get-DirectorySize {
    param([string]$Path)
    if (-not (Test-Path $Path)) { return 0 }
    return (Get-ChildItem -Path $Path -Recurse -ErrorAction SilentlyContinue | 
        Measure-Object -Property Length -Sum).Sum
}

# Check if x86_64-pc-windows-msvc exists and remove it (shouldn't be needed in normal dev)
$x86Path = Join-Path $TargetDir "x86_64-pc-windows-msvc"
if (Test-Path $x86Path) {
    $x86Size = Get-DirectorySize -Path $x86Path
    if ($x86Size -gt 100MB) {  # Only remove if > 100MB
        Write-Host "Removing unnecessary x86_64-pc-windows-msvc directory ($([math]::Round($x86Size/1GB, 2)) GB)..." -ForegroundColor Yellow
        Remove-Item -Path $x86Path -Recurse -Force
        Write-Host "Removed" -ForegroundColor Green
    }
}

# Check total size
$currentSize = Get-DirectorySize -Path $TargetDir

if ($currentSize -gt $maxSizeBytes) {
    Write-Host "Target directory size ($([math]::Round($currentSize/1GB, 2)) GB) exceeds limit ($MaxSizeGB GB)" -ForegroundColor Yellow
    Write-Host "Run '.\scripts\prevent-bloat.ps1' to clean up" -ForegroundColor Yellow
}


