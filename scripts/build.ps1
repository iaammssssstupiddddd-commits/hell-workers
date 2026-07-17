param(
    [switch]$Release
)

$python = Get-Command python3 -ErrorAction SilentlyContinue
if (-not $python) {
    $python = Get-Command python -ErrorAction Stop
}

$arguments = @((Join-Path $PSScriptRoot "dev.py"), "build")
if ($Release) {
    $arguments += "--release"
}

& $python.Source @arguments
exit $LASTEXITCODE
