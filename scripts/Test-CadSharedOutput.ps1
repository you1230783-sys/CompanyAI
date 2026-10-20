# 真實 CAD／Broker 存檔回歸：只在唯一 .build 子目錄建立固定圖，不修改使用者文件。
# 需本機已安裝佑達 ICAD 2008 及 GstarCAD 2022；與生成模型內容測試分開記錄。
param([string]$ExecutablePath)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location $projectRoot
. "$PSScriptRoot/Enter-DevShell.ps1"
if (-not $ExecutablePath) {
    $ExecutablePath = Join-Path $projectRoot 'target/x86_64-pc-windows-msvc/release/company-ai.exe'
}
$ExecutablePath = (Resolve-Path -LiteralPath $ExecutablePath).Path
& cargo build --frozen --example cad_shared_output
if ($LASTEXITCODE -ne 0) { throw 'CAD Broker fixture build failed.' }
$directory = Join-Path $projectRoot 'target/x86_64-pc-windows-msvc/debug/examples'
& "$PSScriptRoot/Stage-Cad.ps1" -ExecutableDirectory $directory
$helper = Join-Path $directory 'cad_shared_output.exe'
$outputRoot = Join-Path $projectRoot '.build/cad-shared-output'
New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
$lines = @(& $helper $ExecutablePath $outputRoot)
$exitCode = $LASTEXITCODE
$lines | Write-Output
if ($exitCode -ne 0) { throw 'CAD Broker save/reopen regression failed; fixture retained.' }
$fixtureLine = $lines | Where-Object { $_ -like 'Fixture: *' } | Select-Object -Last 1
if (-not $fixtureLine) { throw 'Missing native fixture proof path.' }
$fixture = $fixtureLine.Substring('Fixture: '.Length)
@{
    result = 'PASS_SHARED_CAD_BROKER_SAVE_REOPEN'
    exe_sha256 = (Get-FileHash -LiteralPath $ExecutablePath -Algorithm SHA256).Hash.ToLowerInvariant()
    runtime_sha256 = (Get-Content (Join-Path $projectRoot 'assets/cad-runtime-sha256.txt') -Raw).Trim()
    fixture = $fixture
    versions = @('Icad.Application.1', 'GStarCAD.Application.22')
    requested_version_aliases = @('2008', '2022', 'latest', 'auto', '最新版')
    successful_writes_per_version = 3
    original_unchanged = $true
    recorded_at = (Get-Date).ToString('o')
} | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $projectRoot '.build/cad-shared-output-verification.json') -Encoding utf8
