# Migrate existing log files to logs/ directory
# Usage: .\scripts\migrate-logs.ps1

param(
    [string]$LogDir = "logs"
)

# Create logs directory if it doesn't exist
if (-not (Test-Path $LogDir)) {
    New-Item -ItemType Directory -Path $LogDir | Out-Null
    Write-Host "Created logs directory: $LogDir" -ForegroundColor Green
}

# Patterns for log/error files in root directory
$logPatterns = @(
    "build_error*.txt",
    "build_error*.log",
    "build_errors.txt",
    "cargo_check.log",
    "final_check*.log",
    "game_log.txt",
    "run_output.log",
    "check_out.txt"
)

$movedCount = 0
$skippedCount = 0

Write-Host "Scanning for log files to migrate..." -ForegroundColor Cyan

foreach ($pattern in $logPatterns) {
    $files = Get-ChildItem -Path "." -Filter $pattern -File -ErrorAction SilentlyContinue
    
    foreach ($file in $files) {
        $destination = Join-Path $LogDir $file.Name
        
        # Handle filename conflicts by adding timestamp
        if (Test-Path $destination) {
            $timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
            $nameWithoutExt = [System.IO.Path]::GetFileNameWithoutExtension($file.Name)
            $ext = $file.Extension
            $destination = Join-Path $LogDir "${nameWithoutExt}_migrated_${timestamp}${ext}"
        }
        
        try {
            Move-Item -Path $file.FullName -Destination $destination -Force
            Write-Host "  Moved: $($file.Name) -> $LogDir\$([System.IO.Path]::GetFileName($destination))" -ForegroundColor Green
            $movedCount++
        } catch {
            Write-Host "  Failed to move: $($file.Name) - $($_.Exception.Message)" -ForegroundColor Red
            $skippedCount++
        }
    }
}

Write-Host "`nMigration completed!" -ForegroundColor Cyan
Write-Host "  Moved: $movedCount file(s)" -ForegroundColor Green
if ($skippedCount -gt 0) {
    Write-Host "  Skipped: $skippedCount file(s)" -ForegroundColor Yellow
}



