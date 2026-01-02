# Cargo build with optimized error file output
# Usage: .\scripts\build.ps1 [--release]

param(
    [switch]$Release,
    [switch]$Clean,
    [int]$KeepDays = 7,
    [string]$LogDir = "logs"
)

# Create logs directory if it doesn't exist
if (-not (Test-Path $LogDir)) {
    New-Item -ItemType Directory -Path $LogDir | Out-Null
    Write-Host "Created logs directory: $LogDir" -ForegroundColor Green
}

# Clean old log files if requested
if ($Clean) {
    $cutoffDate = (Get-Date).AddDays(-$KeepDays)
    $oldFiles = Get-ChildItem -Path $LogDir -Filter "*.log","*.txt" | Where-Object { $_.LastWriteTime -lt $cutoffDate }
    
    if ($oldFiles.Count -gt 0) {
        $oldFiles | Remove-Item -Force
        Write-Host "Cleaned up $($oldFiles.Count) old log file(s) (older than $KeepDays days)" -ForegroundColor Yellow
    }
}

# Generate timestamped log file name
$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$buildType = if ($Release) { "release" } else { "debug" }
$errorLogFile = Join-Path $LogDir "build_error_${buildType}_$timestamp.log"
$combinedLogFile = Join-Path $LogDir "build_combined_${buildType}_$timestamp.log"

$buildArgs = if ($Release) { @("build", "--release") } else { @("build") }

Write-Host "Running cargo build ($buildType)..." -ForegroundColor Cyan
Write-Host "Error log: $errorLogFile" -ForegroundColor Gray

# Run cargo build and capture both stdout and stderr
$process = Start-Process -FilePath "cargo" -ArgumentList $buildArgs -NoNewWindow -Wait -PassThru -RedirectStandardOutput $combinedLogFile -RedirectStandardError $errorLogFile

# Read and display errors if they exist
if (Test-Path $errorLogFile) {
    $errorContent = Get-Content $errorLogFile -Raw -Encoding UTF8
    if ($errorContent -and $errorContent.Trim().Length -gt 0) {
        Write-Host "`n=== Build Errors ===" -ForegroundColor Red
        $errorContent | Write-Host -ForegroundColor Red
        
        # Also check combined log for any additional info
        if (Test-Path $combinedLogFile) {
            $combinedContent = Get-Content $combinedLogFile -Raw -Encoding UTF8
            if ($combinedContent) {
                Add-Content -Path $errorLogFile -Value "`n=== Combined Output ===" -Encoding UTF8
                Add-Content -Path $errorLogFile -Value $combinedContent -Encoding UTF8
            }
        }
        
        Write-Host "`nFull error log saved to: $errorLogFile" -ForegroundColor Yellow
        exit $process.ExitCode
    }
}

# If no errors, show success message
if (Test-Path $combinedLogFile) {
    $output = Get-Content $combinedLogFile -Raw -Encoding UTF8
    Write-Host $output -ForegroundColor Green
}

Write-Host "`nBuild completed successfully!" -ForegroundColor Green
exit 0

