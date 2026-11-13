# Forward to scripts/package.ps1
$ScriptPath = Join-Path $PSScriptRoot "scripts\package.ps1"
& $ScriptPath @args
exit $LASTEXITCODE
