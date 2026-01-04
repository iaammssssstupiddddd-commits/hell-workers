# Clean old log files
# Usage: .\scripts\clean-logs.ps1 [-KeepDays 7] [-LogDir "logs"]

param(
    [int]$KeepDays = 7,
    [string]$LogDir = "logs"
)

if (-not (Test-Path $LogDir)) {
    Write-Host "Logs directory does not exist: $LogDir" -ForegroundColor Yellow
    exit 0
}

$cutoffDate = (Get-Date).AddDays(-$KeepDays)
$oldFiles = Get-ChildItem -Path $LogDir -Filter "*.log","*.txt" | Where-Object { $_.LastWriteTime -lt $cutoffDate }

if ($oldFiles.Count -eq 0) {
    Write-Host "No old log files found (older than $KeepDays days)" -ForegroundColor Green
    exit 0
}

Write-Host "Found $($oldFiles.Count) old log file(s) to delete:" -ForegroundColor Yellow
$oldFiles | ForEach-Object {
    Write-Host "  - $($_.Name) (last modified: $($_.LastWriteTime))" -ForegroundColor Gray
}

$response = Read-Host "Delete these files? (y/N)"
if ($response -eq 'y' -or $response -eq 'Y') {
    $oldFiles | Remove-Item -Force
    Write-Host "Deleted $($oldFiles.Count) log file(s)" -ForegroundColor Green
} else {
    Write-Host "Cancelled." -ForegroundColor Yellow
}




