# 保留專案內一致的入口；實際下載與安裝交給外層的共用環境。
[CmdletBinding()]
param([string]$SharedRoot = $env:RUST_SHARED_ROOT)
$ErrorActionPreference = 'Stop'
$companyProjectRoot = Split-Path $PSScriptRoot -Parent
if (-not $SharedRoot) {
    $SharedRoot = Split-Path (Split-Path $companyProjectRoot -Parent) -Parent
}
$sharedInstaller = Join-Path $SharedRoot 'scripts\Install-Rust.ps1'
if (-not (Test-Path -LiteralPath $sharedInstaller)) {
    throw 'Shared Rust installer not found. Set RUST_SHARED_ROOT to the shared Rust folder.'
}
& $sharedInstaller
