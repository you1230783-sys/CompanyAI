# 只在 GitHub 的拋棄式 Windows runner 準備工具與 vendor，不修改開發機共用環境。
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Initialize-CI is only for disposable GitHub Actions runners.' }
$projectRoot = Split-Path $PSScriptRoot -Parent
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$visualStudio = & $vswhere -latest -version '[17.0,18.0)' -products '*' -property installationPath
if (-not $visualStudio -or $LASTEXITCODE -ne 0) { throw 'Visual Studio 2022 host is required.' }
$sdk = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\Include\10.0.19041.0\um\Windows.h'
if (-not (Test-Path (Join-Path $visualStudio 'VC\Tools\MSVC\14.29.*\bin\Hostx64\x64\cl.exe')) -or -not (Test-Path -LiteralPath $sdk)) {
    $installer = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\setup.exe'
    # setup.exe 本身沒有 --wait；由 PowerShell 等待程序樹結束，再核對實際元件。
    $arguments = 'modify --installPath "{0}" --add Microsoft.VisualStudio.Component.VC.14.29.16.11.x86.x64 --add Microsoft.VisualStudio.Component.Windows10SDK.19041 --quiet --norestart' -f $visualStudio
    $process = Start-Process -FilePath $installer -ArgumentList $arguments -Wait -PassThru -WindowStyle Hidden
    if ($process.ExitCode -notin @(0, 3010)) { throw "v142 installation failed: $($process.ExitCode)" }
}
if (-not (Test-Path (Join-Path $visualStudio 'VC\Tools\MSVC\14.29.*\bin\Hostx64\x64\link.exe')) -or -not (Test-Path -LiteralPath $sdk)) {
    throw 'Required v142 / SDK 19041 components are still missing.'
}
# 從受版本控制的檔案取得 Rust 版本；不使用 runner 預設 stable。
$toolchainFile = Get-Content -LiteralPath (Join-Path $projectRoot 'rust-toolchain.toml') -Raw
if ($toolchainFile -notmatch '(?m)^channel = "(\d+\.\d+\.\d+)"$') { throw 'Expected an exact Rust version.' }
$toolchain = $matches[1] + '-x86_64-pc-windows-msvc'
& rustup toolchain install $toolchain --profile minimal --component rustfmt,clippy,rust-src
if ($LASTEXITCODE -ne 0) { throw 'Pinned Rust installation failed.' }
$rustc = & rustup which --toolchain $toolchain rustc
if ($LASTEXITCODE -ne 0) { throw 'Pinned rustc not found.' }
$source = Split-Path (Split-Path $rustc -Parent) -Parent
$destination = Join-Path $projectRoot 'toolchain'
if (Test-Path -LiteralPath $destination) { throw 'CI toolchain destination must be absent on a fresh runner.' }
Copy-Item -LiteralPath $source -Destination $destination -Recurse
. (Join-Path $PSScriptRoot 'Enter-DevShell.ps1') -VisualStudioPath $visualStudio
$env:CARGO_HOME = Join-Path $projectRoot '.build\ci-bootstrap-cargo'
$env:CARGO_NET_OFFLINE = 'false'
Push-Location $projectRoot
try {
    $lockHash = (Get-FileHash -LiteralPath Cargo.lock -Algorithm SHA256).Hash
    # vendor 預設忽略 source replacement；僅初次準備連網，不能改寫既有 config。
    & cargo vendor --locked --versioned-dirs vendor
    if ($LASTEXITCODE -ne 0) { throw 'Locked dependency preparation failed.' }
    if ((Get-FileHash -LiteralPath Cargo.lock -Algorithm SHA256).Hash -ne $lockHash) { throw 'Cargo.lock changed during CI preparation.' }
} finally {
    $env:CARGO_NET_OFFLINE = 'true'
    Pop-Location
}
