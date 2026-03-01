# Analyze target directory to understand what's taking up space
# Usage: .\scripts\analyze-target.ps1

param(
    [string]$TargetDir = ""
)

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

if ([string]::IsNullOrWhiteSpace($TargetDir)) {
    $TargetDir = Resolve-CargoTargetDir
}

if (-not (Test-Path $TargetDir)) {
    Write-Host "Target directory does not exist: $TargetDir" -ForegroundColor Yellow
    exit 0
}

Write-Host "=== Target Directory Analysis ===" -ForegroundColor Cyan
Write-Host "Target directory: $TargetDir" -ForegroundColor Gray
Write-Host ""

# Analyze top-level directories in target
Write-Host "Top-level directories:" -ForegroundColor Yellow
Get-ChildItem -Path $TargetDir -Directory | ForEach-Object {
    $size = (Get-ChildItem $_.FullName -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    $percentage = ($size / (Get-ChildItem -Path $TargetDir -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum) * 100
    [PSCustomObject]@{
        Name = $_.Name
        'Size(GB)' = [math]::Round($size/1GB, 3)
        'Size(MB)' = [math]::Round($size/1MB, 2)
        'Percentage' = [math]::Round($percentage, 1)
    }
} | Sort-Object 'Size(MB)' -Descending | Format-Table -AutoSize

# Analyze debug directory contents
$debugPath = Join-Path $TargetDir "debug"
if (Test-Path $debugPath) {
    Write-Host "`ndebug/ directory contents:" -ForegroundColor Yellow
    Get-ChildItem -Path $debugPath -Directory -ErrorAction SilentlyContinue | ForEach-Object {
        $size = (Get-ChildItem $_.FullName -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
        $item = if ($_.Name -eq "incremental") { "⚡ INCREMENTAL (重要: ビルド速度に直結)" }
               elseif ($_.Name -eq "build") { "📦 BUILD ARTIFACTS (削除可能、再生成される)" }
               elseif ($_.Name -eq "deps") { "🔗 DEPENDENCIES (重要: ビルド速度に直結)" }
               else { $_.Name }
        [PSCustomObject]@{
            Item = $item
            'Size(GB)' = [math]::Round($size/1GB, 3)
            'Size(MB)' = [math]::Round($size/1MB, 2)
        }
    } | Sort-Object 'Size(MB)' -Descending | Format-Table -AutoSize
}

# Check x86_64-pc-windows-msvc directory
$x86Path = Join-Path $TargetDir "x86_64-pc-windows-msvc"
if (Test-Path $x86Path) {
    Write-Host "`nx86_64-pc-windows-msvc/ directory (クロスコンパイル用):" -ForegroundColor Yellow
    $x86Size = (Get-ChildItem -Path $x86Path -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    Write-Host "  Size: $([math]::Round($x86Size/1GB, 3)) GB ($([math]::Round($x86Size/1MB, 2)) MB)" -ForegroundColor Cyan
    
    $x86DebugPath = Join-Path $x86Path "debug"
    if (Test-Path $x86DebugPath) {
        $x86IncrementalPath = Join-Path $x86DebugPath "incremental"
        if (Test-Path $x86IncrementalPath) {
        $x86IncrementalSize = (Get-ChildItem -Path $x86IncrementalPath -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
        Write-Host "  Incremental cache: $([math]::Round($x86IncrementalSize/1GB, 3)) GB ($([math]::Round($x86IncrementalSize/1MB, 2)) MB)" -ForegroundColor Gray
            Write-Host "  Note: クロスコンパイル用なので、通常開発では不要な場合が多い" -ForegroundColor Yellow
        }
    }
}

Write-Host "`n=== Recommendations ===" -ForegroundColor Cyan
Write-Host "1. incremental/ と deps/ はビルド速度に重要 → 保持推奨" -ForegroundColor Green
Write-Host "2. build/ は再生成可能だが時間がかかる → 容量に余裕があるなら保持" -ForegroundColor Yellow
Write-Host "3. x86_64-pc-windows-msvc/ はクロスコンパイル用 → 不要なら削除可能" -ForegroundColor Yellow
