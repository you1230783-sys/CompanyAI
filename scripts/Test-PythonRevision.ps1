# 固定合成CSV的真正Broker／AppContainer串接；不讀取使用者資料。
param([string]$ExecutablePath)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
Set-Location $projectRoot
. "$PSScriptRoot/Enter-DevShell.ps1"
if(-not $ExecutablePath){$ExecutablePath=Join-Path $projectRoot 'target/x86_64-pc-windows-msvc/release/company-ai.exe'}
$ExecutablePath=(Resolve-Path -LiteralPath $ExecutablePath).Path
& cargo build --frozen --example python_smoke
if($LASTEXITCODE -ne 0){throw 'Python revision fixture build failed.'}
$directory=Join-Path $projectRoot 'target/x86_64-pc-windows-msvc/debug/examples'
& "$PSScriptRoot/Stage-Python.ps1" -ExecutableDirectory $directory
$fixture=Join-Path $projectRoot ('.build/python-revision-'+[guid]::NewGuid().ToString('N'))
& (Join-Path $directory 'python_smoke.exe') $ExecutablePath $fixture --revision-only
if($LASTEXITCODE -ne 0){throw 'Python revision bridge failed; fixture retained.'}
@{result='PASS_NATIVE_FILE_PYTHON_REVISION_BRIDGE';exe_sha256=(Get-FileHash -LiteralPath $ExecutablePath).Hash.ToLowerInvariant();fixture=$fixture;unchanged_file_accepted=$true;changed_file_rejected=$true;recorded_at=(Get-Date).ToString('o')}|ConvertTo-Json|Set-Content (Join-Path $projectRoot '.build/python-revision-verification.json') -Encoding utf8
