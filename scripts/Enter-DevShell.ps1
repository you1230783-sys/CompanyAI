# Dot-source this file: . .\scripts\Enter-DevShell.ps1
[CmdletBinding()]
param(
    [string]$SharedRoot = $env:RUST_SHARED_ROOT,
    [string]$VisualStudioPath = $env:RUST_PROJECT_VS_PATH,
    [string]$WindowsSdkVersion = '10.0.19041.0'
)
$ErrorActionPreference = 'Stop'
# 所有路徑都從專案位置推導，避免把家裡的磁碟代號寫死在環境腳本中。
$projectRoot = Split-Path $PSScriptRoot -Parent
$toolchainName = '1.98.1-x86_64-pc-windows-msvc'
# 解壓後優先使用包內工具鏈；一般開發也可使用外層共享工具。
$toolchainBin = Join-Path $projectRoot 'toolchain\bin'
if (-not (Test-Path -LiteralPath (Join-Path $toolchainBin 'rustc.exe'))) {
    if (-not $SharedRoot) { $SharedRoot = Split-Path (Split-Path $projectRoot -Parent) -Parent }
    $toolchainBin = Join-Path $SharedRoot ".tools\rustup\toolchains\$toolchainName\bin"
}
if (-not (Test-Path (Join-Path $toolchainBin 'rustc.exe'))) {
    throw 'Rust toolchain is missing. Extract offline\CompanyAI-offline.zip into a new folder, or set RUST_SHARED_ROOT.'
}
# 尋找實際裝有 v142 的 VS；不依賴使用者目前 PATH 中的預設編譯器。
if (-not $VisualStudioPath) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        $candidates = & $vswhere -all -products '*' -property installationPath
        foreach ($candidate in $candidates) {
            if (Test-Path (Join-Path $candidate 'VC\Tools\MSVC\14.29.*\bin\Hostx64\x64\link.exe')) {
                $VisualStudioPath = $candidate
                break
            }
        }
    }
}
if (-not $VisualStudioPath) {
    throw 'MSVC v142 (14.29) not found. Install v142 and Windows SDK 10.0.19041.0 using Visual Studio Installer.'
}
$envScript = Join-Path $PSScriptRoot 'vs-env.cmd'
# vcvars64 是批次檔，先在 cmd 載入，再把設定匯入目前 PowerShell 程序。
$commandLine = 'call "{0}" "{1}" "{2}"' -f $envScript, $VisualStudioPath, $WindowsSdkVersion
$vsEnvironment = & $env:ComSpec /d /c $commandLine
if ($LASTEXITCODE -ne 0) { throw 'Could not initialize MSVC v142 / Windows SDK.' }
foreach ($entry in $vsEnvironment) {
    if ($entry -match '^([^=]+)=(.*)$') {
        [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process')
    }
}
if ($env:VCToolsVersion -notlike '14.29.*') { throw "Unexpected MSVC: $env:VCToolsVersion" }
if ($env:WindowsSDKVersion.TrimEnd('\') -ne $WindowsSdkVersion) { throw 'Unexpected Windows SDK version.' }
if (Test-Path -LiteralPath (Join-Path $projectRoot 'toolchain\bin\rustc.exe')) {
    # 包內模式不使用共享 Cargo 快取；驗證時這個目錄從空目錄開始。
    $env:CARGO_HOME = Join-Path $projectRoot '.tools\cargo'
} else {
    $env:CARGO_HOME = Join-Path $SharedRoot '.tools\cargo'
}
$env:RUSTUP_HOME = Join-Path $projectRoot '.tools\rustup'
# 直接使用固定版本工具，避免 rustup 自動連網或套用其他專案的預設版本。
$env:PATH = "$toolchainBin;$env:CARGO_HOME\bin;$env:PATH"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = Join-Path $env:VCToolsInstallDir 'bin\Hostx64\x64\link.exe'
$env:CARGO_TARGET_DIR = Join-Path $projectRoot 'target'
$env:CARGO_NET_OFFLINE = 'true'
Write-Host "Rust 1.98.1 | MSVC $($env:VCToolsVersion) | SDK $($env:WindowsSDKVersion)"

$env:RUSTUP_AUTO_INSTALL = '0'
$env:RUSTC = Join-Path $toolchainBin 'rustc.exe'
$env:RUSTDOC = Join-Path $toolchainBin 'rustdoc.exe'
Write-Host "Rust binaries: $toolchainBin"
