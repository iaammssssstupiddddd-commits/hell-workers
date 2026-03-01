# Delete x86_64-pc-windows-msvc directory
function Resolve-CargoTargetDir {
    try {
        $metadataJson = cargo metadata --no-deps --format-version 1 2>$null
        if ($LASTEXITCODE -eq 0 -and $metadataJson) {
            $metadata = $metadataJson | ConvertFrom-Json
            if ($metadata.target_directory) {
                return [string]$metadata.target_directory
            }
        }
    } catch {
        # Fallback below
    }

    return "target"
}

$TargetDir = Resolve-CargoTargetDir
$x86Path = Join-Path $TargetDir "x86_64-pc-windows-msvc"

if (Test-Path $x86Path) {
    $size = (Get-ChildItem $x86Path -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    $sizeGB = [math]::Round($size / 1GB, 2)
    Write-Host "Deleting: $sizeGB GB..." -ForegroundColor Yellow
    Remove-Item -Path $x86Path -Recurse -Force
    Write-Host "Deleted successfully!" -ForegroundColor Green
} else {
    Write-Host "Directory not found" -ForegroundColor Yellow
}
