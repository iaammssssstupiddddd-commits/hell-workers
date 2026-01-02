# Optimize target directory: remove unnecessary files while preserving build speed
# Usage: .\scripts\optimize-target.ps1

param(
    [switch]$KeepCrossCompile,
    [string]$TargetDir = "target"
)

if (-not (Test-Path $TargetDir)) {
    Write-Host "Target directory does not exist: $TargetDir" -ForegroundColor Yellow
    exit 0
}

# Calculate size before cleanup
$sizeBefore = (Get-ChildItem -Path $TargetDir -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
Write-Host "=== Target Directory Optimization ===" -ForegroundColor Cyan
Write-Host "Current size: $([math]::Round($sizeBefore/1GB, 2)) GB ($([math]::Round($sizeBefore/1MB, 2)) MB)" -ForegroundColor Yellow
Write-Host ""

$savedTotal = 0

# 1. Remove x86_64-pc-windows-msvc (cross-compilation artifacts) if not needed
$x86Path = Join-Path $TargetDir "x86_64-pc-windows-msvc"
if ((Test-Path $x86Path) -and (-not $KeepCrossCompile)) {
    $x86Size = (Get-ChildItem -Path $x86Path -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    Write-Host "1. Cross-compilation artifacts (x86_64-pc-windows-msvc/):" -ForegroundColor Cyan
    Write-Host "   Size: $([math]::Round($x86Size/1GB, 2)) GB" -ForegroundColor Gray
    Write-Host "   Impact: ビルド速度への影響なし（通常開発では不要）" -ForegroundColor Green
    
    $response = Read-Host "   Remove? (Y/n)"
    if ($response -ne 'n' -and $response -ne 'N') {
        Remove-Item -Path $x86Path -Recurse -Force
        Write-Host "   ✓ Removed (saved $([math]::Round($x86Size/1GB, 2)) GB)" -ForegroundColor Green
        $savedTotal += $x86Size
    } else {
        Write-Host "   Skipped" -ForegroundColor Yellow
    }
    Write-Host ""
}

# 2. Clean old incremental build caches (keep recent ones)
$debugPath = Join-Path $TargetDir "debug" "incremental"
$x86IncrementalPath = Join-Path $TargetDir "x86_64-pc-windows-msvc" "debug" "incremental"

foreach ($incPath in @($debugPath, $x86IncrementalPath)) {
    if (Test-Path $incPath) {
        # Keep only the most recent incremental cache (last 7 days)
        $cutoffDate = (Get-Date).AddDays(-7)
        $oldCaches = Get-ChildItem -Path $incPath -Directory -ErrorAction SilentlyContinue | 
            Where-Object { $_.LastWriteTime -lt $cutoffDate }
        
        if ($oldCaches.Count -gt 0) {
            $oldSize = ($oldCaches | Get-ChildItem -Recurse -ErrorAction SilentlyContinue | 
                Measure-Object -Property Length -Sum).Sum
            Write-Host "2. Old incremental caches (older than 7 days):" -ForegroundColor Cyan
            Write-Host "   Found: $($oldCaches.Count) old cache(s)" -ForegroundColor Gray
            Write-Host "   Size: $([math]::Round($oldSize/1MB, 2)) MB" -ForegroundColor Gray
            Write-Host "   Impact: 最新のキャッシュは保持されるため、ビルド速度への影響は最小限" -ForegroundColor Green
            
            $response = Read-Host "   Remove old caches? (Y/n)"
            if ($response -ne 'n' -and $response -ne 'N') {
                $oldCaches | Remove-Item -Recurse -Force
                Write-Host "   ✓ Removed old caches" -ForegroundColor Green
                $savedTotal += $oldSize
            }
            Write-Host ""
        }
    }
}

# 3. Option to clean build artifacts (but warn about rebuild time)
$buildPath = Join-Path $TargetDir "debug" "build"
if (Test-Path $buildPath) {
    $buildSize = (Get-ChildItem -Path $buildPath -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    Write-Host "3. Build artifacts (debug/build/):" -ForegroundColor Cyan
    Write-Host "   Size: $([math]::Round($buildSize/1MB, 2)) MB" -ForegroundColor Gray
    Write-Host "   Impact: ⚠️ 削除すると次回ビルド時に再生成が必要（時間がかかる）" -ForegroundColor Yellow
    Write-Host "   Recommendation: 容量に余裕がある場合は保持推奨" -ForegroundColor Yellow
    
    $response = Read-Host "   Remove? (y/N)"
    if ($response -eq 'y' -or $response -eq 'Y') {
        Remove-Item -Path $buildPath -Recurse -Force
        Write-Host "   ✓ Removed (saved $([math]::Round($buildSize/1MB, 2)) MB)" -ForegroundColor Green
        $savedTotal += $buildSize
    } else {
        Write-Host "   Skipped (recommended)" -ForegroundColor Green
    }
    Write-Host ""
}

# Calculate final size
$sizeAfter = (Get-ChildItem -Path $TargetDir -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum

Write-Host "=== Optimization Summary ===" -ForegroundColor Cyan
Write-Host "Size before: $([math]::Round($sizeBefore/1GB, 2)) GB ($([math]::Round($sizeBefore/1MB, 2)) MB)" -ForegroundColor Gray
Write-Host "Size after:  $([math]::Round($sizeAfter/1GB, 2)) GB ($([math]::Round($sizeAfter/1MB, 2)) MB)" -ForegroundColor Gray
Write-Host "Space freed: $([math]::Round($savedTotal/1GB, 2)) GB ($([math]::Round($savedTotal/1MB, 2)) MB)" -ForegroundColor Green

if ($savedTotal -gt 0) {
    $percentage = ($savedTotal / $sizeBefore) * 100
    Write-Host "Reduction:   $([math]::Round($percentage, 1))%" -ForegroundColor Green
}

Write-Host ""
Write-Host "✓ 重要なビルドキャッシュ（deps/、最新のincremental/）は保持されています" -ForegroundColor Green
Write-Host "✓ ビルド速度への影響は最小限です" -ForegroundColor Green

