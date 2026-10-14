# 建置機專用；使用者端不執行 PowerShell 或 pip。
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$ExecutableDirectory)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$source = (Get-Content (Join-Path $root '.build\cad-runtime-stage.txt') -Raw).Trim()
$expected = (Get-Content (Join-Path $root 'assets\cad-runtime-sha256.txt') -Raw).Trim()
if ((Get-FileHash (Join-Path $source 'manifest.json')).Hash.ToLowerInvariant() -ne $expected) { throw 'CAD manifest mismatch.' }
$manifest = Get-Content (Join-Path $source 'manifest.json') -Raw | ConvertFrom-Json
foreach ($entry in $manifest.PSObject.Properties) {
    if ((Get-FileHash (Join-Path $source $entry.Name)).Hash.ToLowerInvariant() -ne $entry.Value) { throw "CAD runtime mismatch: $($entry.Name)" }
}
foreach ($pair in @(@('worker.py','src\cad_worker.py'), @('cad.py','tools\gstarcad-probe\cad.py'), @('registry.py','tools\gstarcad-probe\registry.py'))) {
    if ((Get-FileHash (Join-Path $source $pair[0])).Hash -ne (Get-FileHash (Join-Path $root $pair[1])).Hash) { throw 'CAD source changed: rebuild the pinned runtime first.' }
}
$directoryName = 'cad-' + $expected.Substring(0,16)
$destination = Join-Path $ExecutableDirectory $directoryName
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Get-ChildItem -LiteralPath $source | Copy-Item -Destination $destination -Recurse -Force
Write-Host "Staged CAD runtime: $destination"
