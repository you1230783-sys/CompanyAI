# 固定的 Python 離線環境；只在開發／建置時執行，使用者端不執行 pip。
[CmdletBinding()]
param([string]$Destination, [switch]$RefreshManifest)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (-not $Destination) { $Destination = Join-Path $root '.build\python-runtime' }
$Destination = [IO.Path]::GetFullPath($Destination)
$archive = Join-Path $root 'offline\python-inputs.zip'
$lock = Get-Content -LiteralPath (Join-Path $root 'offline\python-lock.json') -Raw | ConvertFrom-Json
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $lock.archive_sha256) { throw 'Python offline archive checksum mismatch.' }
$inputs = Join-Path $root '.build\python-inputs'
New-Item -ItemType Directory -Path $inputs -Force | Out-Null
Expand-Archive -LiteralPath $archive -DestinationPath $inputs -Force
foreach ($entry in $lock.files) {
    if ([IO.Path]::GetFileName($entry.name) -ne $entry.name) { throw 'Invalid Python input filename.' }
    $path = Join-Path $inputs $entry.name
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.sha256) { throw "Python input checksum mismatch: $($entry.name)" }
}
# 新目錄重建，避免把舊版模組、開發機 site-packages 或快取混入交付。
if (Test-Path -LiteralPath $Destination) { throw "Python destination must be new: $Destination" }
New-Item -ItemType Directory -Path $Destination -Force | Out-Null
Expand-Archive -LiteralPath (Join-Path $inputs $lock.python_archive) -DestinationPath $Destination
$packages = Join-Path $Destination 'Lib\site-packages'
New-Item -ItemType Directory -Path $packages -Force | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem
foreach ($entry in $lock.files | Where-Object { $_.name -like '*.whl' }) {
    [IO.Compression.ZipFile]::ExtractToDirectory((Join-Path $inputs $entry.name), $packages)
}
# 明確隔離使用者 PATH／登錄／site-packages；不啟用 site 或自動執行 .pth。
@('python313.zip', '.', 'Lib\site-packages') | Set-Content -LiteralPath (Join-Path $Destination 'python313._pth') -Encoding ASCII
Copy-Item -LiteralPath (Join-Path $root 'src\projects\python_worker.py') -Destination (Join-Path $Destination 'lm_worker.py')
& (Join-Path $Destination 'python.exe') -I -B -c 'import sys,pandas,numpy,openpyxl; print(sys.version); print(pandas.__version__,numpy.__version__,openpyxl.__version__)'
if ($LASTEXITCODE -ne 0) { throw 'Offline Python import verification failed.' }
# 保存每個檔案雜湊，原生啟動端核對後才允許執行模型程式。
$manifest = @()
Get-ChildItem -LiteralPath $Destination -Recurse -File | Sort-Object FullName | ForEach-Object {
    $manifest += [ordered]@{ path = $_.FullName.Substring($Destination.Length + 1).Replace('\','/'); sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(); size = $_.Length }
}
$json = ConvertTo-Json -InputObject $manifest -Depth 4 -Compress
$pinnedManifest = Join-Path $root 'assets\python-runtime-manifest.json'
$pinnedHash = Join-Path $root 'assets\python-runtime-sha256.txt'
if ($RefreshManifest) {
    # 僅在維護者刻意更新 worker／依賴時改版；一般離線建置不能改信任基準。
    [IO.File]::WriteAllText($pinnedManifest, $json, (New-Object Text.UTF8Encoding($false)))
    [IO.File]::WriteAllText($pinnedHash, (Get-FileHash -LiteralPath $pinnedManifest).Hash.ToLowerInvariant() + "`n", (New-Object Text.UTF8Encoding($false)))
}
if ((Get-FileHash -LiteralPath $pinnedManifest).Hash.ToLowerInvariant() -ne (Get-Content -LiteralPath $pinnedHash -Raw).Trim()) { throw 'Pinned Python manifest/hash disagree.' }
# PowerShell 5.1／7 的文化排序不同。比較檔案集合與 bytes，而非重新序列化後的排列；
# 驗證全部通過才複製同一份固定清單，確保不同建置環境產生完全相同的 runtime。
$actual = @{}
foreach ($entry in $manifest) { $actual[$entry.path] = $entry }
$expected = Get-Content -LiteralPath $pinnedManifest -Raw | ConvertFrom-Json
if ($actual.Count -ne $expected.Count) { throw 'Rebuilt Python file count differs from the pinned manifest.' }
foreach ($entry in $expected) {
    $rebuilt = $actual[$entry.path]
    if (-not $rebuilt -or $rebuilt.sha256 -ne $entry.sha256 -or $rebuilt.size -ne $entry.size) { throw "Rebuilt Python differs: $($entry.path). Review the changes and use -RefreshManifest only for an intentional runtime update." }
}
Copy-Item -LiteralPath $pinnedManifest -Destination (Join-Path $Destination 'runtime-manifest.json')
Write-Host "Offline Python ready: $Destination"
