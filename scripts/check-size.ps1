# Check folder sizes
# Usage: .\scripts\check-size.ps1

Write-Host "=== Directory Sizes ===" -ForegroundColor Cyan
Get-ChildItem -Directory | ForEach-Object {
    $size = (Get-ChildItem $_.FullName -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    [PSCustomObject]@{
        Name = $_.Name
        'Size(MB)' = [math]::Round($size/1MB, 2)
        'Size(GB)' = [math]::Round($size/1GB, 3)
    }
} | Sort-Object 'Size(MB)' -Descending | Format-Table -AutoSize

Write-Host "`n=== Largest Files (Top 20) ===" -ForegroundColor Cyan
Get-ChildItem -File | ForEach-Object {
    [PSCustomObject]@{
        Name = $_.Name
        'Size(MB)' = [math]::Round($_.Length/1MB, 2)
    }
} | Sort-Object 'Size(MB)' -Descending | Select-Object -First 20 | Format-Table -AutoSize

Write-Host "`n=== Total Project Size ===" -ForegroundColor Cyan
$totalSize = (Get-ChildItem -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
Write-Host "Total: $([math]::Round($totalSize/1MB, 2)) MB ($([math]::Round($totalSize/1GB, 3)) GB)" -ForegroundColor Yellow



