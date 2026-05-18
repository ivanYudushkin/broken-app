$ErrorActionPreference = "Stop"
$symDir = Join-Path $env:USERPROFILE "Symbols"
New-Item -ItemType Directory -Force -Path $symDir | Out-Null
$env:_NT_SYMBOL_PATH = "srv*$symDir*https://msdl.microsoft.com/download/symbols"

$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $repoRoot

cargo flamegraph --profile flamegraph --bench baseline
