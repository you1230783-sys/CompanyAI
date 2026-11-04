# 下載 RustSec 官方開發工具，不加入應用依賴或發行包。
# 工具採官方預編譯版本；應用本身仍由 Build.ps1 強制以 v142 建置。
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$auditVersion = '0.22.2'
$archiveHash = '0a7316540862c13d954f648917ceacca593747baed6eec180fafa590be2710ab'
$auditRoot = Join-Path $projectRoot '.build\audit-tools'
$archive = Join-Path $auditRoot "cargo-audit-$auditVersion.zip"
New-Item -ItemType Directory -Path $auditRoot -Force | Out-Null
if (-not (Test-Path -LiteralPath $archive)) {
    Invoke-WebRequest -Uri "https://github.com/rustsec/rustsec/releases/download/cargo-audit/v$auditVersion/cargo-audit-x86_64-pc-windows-msvc-v$auditVersion.zip" -OutFile $archive
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ine $archiveHash) {
    throw 'RustSec archive SHA256 mismatch. Inspect the download; no executable was run.'
}
# 每次從已核對的壓縮檔取出，不能只相信既有 EXE 回報的版本字串。
Expand-Archive -LiteralPath $archive -DestinationPath (Join-Path $auditRoot 'bin') -Force
$auditExe = Join-Path $auditRoot "bin\cargo-audit-x86_64-pc-windows-msvc-v$auditVersion\cargo-audit.exe"
$actual = & $auditExe --version
if ($LASTEXITCODE -ne 0 -or $actual -ne "cargo-audit $auditVersion") {
    throw 'Pinned RustSec executable version mismatch.'
}
Write-Host "Verified official $actual archive SHA256=$archiveHash"
