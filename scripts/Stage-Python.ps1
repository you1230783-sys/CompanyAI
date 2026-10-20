# 把已驗證的獨立 Python 目錄複製到 EXE 旁；只在建置機執行。
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$ExecutableDirectory)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$source = Join-Path $root '.build\python-runtime'
if (-not (Test-Path -LiteralPath $source)) { & (Join-Path $PSScriptRoot 'Prepare-Python.ps1') }
$expected = (Get-Content -LiteralPath (Join-Path $root 'assets\python-runtime-sha256.txt') -Raw).Trim()
if ((Get-FileHash -LiteralPath (Join-Path $source 'runtime-manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) { throw 'Python runtime differs from the source-pinned manifest.' }
if ((Get-FileHash -LiteralPath (Join-Path $source 'lm_worker.py')).Hash -ne (Get-FileHash -LiteralPath (Join-Path $root 'src\projects\python_worker.py')).Hash) { throw 'Python worker changed: rebuild in a new directory with Prepare-Python.ps1 -RefreshManifest, review the pinned manifest/hash, then compile.' }
$destination = Join-Path $ExecutableDirectory 'python'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
# 不遞迴刪除既有目錄；完整性檢查會拒絕遺漏或錯誤檔案。
Get-ChildItem -LiteralPath $source | Copy-Item -Destination $destination -Recurse -Force
Write-Host "Staged Python: $destination"
