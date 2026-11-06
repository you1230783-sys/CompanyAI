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
# 套件裁剪或增加 bytecode 時不能直接覆蓋舊樹，否則會留下清單外檔案。
# 建置目錄內的舊版本只改名留存；不刪除檔案，也不處理正式安裝目錄。
$destination = [IO.Path]::GetFullPath($destination)
if (-not $destination.StartsWith(([IO.Path]::GetFullPath($root) + [IO.Path]::DirectorySeparatorChar), [StringComparison]::OrdinalIgnoreCase)) { throw 'Python staging destination must stay inside this project.' }
if (Test-Path -LiteralPath $destination) {
    if ((Get-Item -LiteralPath $destination).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Python staging destination cannot be a link.' }
    $oldManifest = Join-Path $destination 'runtime-manifest.json'
    if (-not (Test-Path -LiteralPath $oldManifest) -or (Get-FileHash -LiteralPath $oldManifest -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) {
        # 留存到開發產物目錄，避免舊 runtime 混入 dist 或被誤收入交付／Git。
        $archiveRoot = [IO.Path]::GetFullPath((Join-Path $root '.build'))
        $preserved = [IO.Path]::GetFullPath((Join-Path $archiveRoot ('python-stage-previous-' + [guid]::NewGuid().ToString('N'))))
        if ([IO.Path]::GetDirectoryName($preserved) -cne $archiveRoot) { throw 'Unsafe previous Python staging path.' }
        New-Item -ItemType Directory -Path $archiveRoot -Force | Out-Null
        Move-Item -LiteralPath $destination -Destination $preserved
        Write-Host "Previous staged runtime preserved: $preserved"
    }
}
New-Item -ItemType Directory -Path $destination -Force | Out-Null
# 不遞迴刪除既有目錄；完整性檢查會拒絕遺漏或錯誤檔案。
Get-ChildItem -LiteralPath $source | Copy-Item -Destination $destination -Recurse -Force
Write-Host "Staged Python: $destination"
