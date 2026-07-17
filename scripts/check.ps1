param(
    [string]$Package,
    [switch]$Tests
)

$python = Get-Command python3 -ErrorAction SilentlyContinue
if (-not $python) {
    $python = Get-Command python -ErrorAction Stop
}

$arguments = @((Join-Path $PSScriptRoot "dev.py"), "check")
if ($Package) {
    $arguments += @("--package", $Package)
}
if ($Tests) {
    $arguments += "--tests"
}

& $python.Source @arguments
exit $LASTEXITCODE
