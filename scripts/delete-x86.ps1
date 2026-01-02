# Delete x86_64-pc-windows-msvc directory
$TargetDir = "target"
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
