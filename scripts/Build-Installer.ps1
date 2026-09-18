# 開發／離線重建專用，使用者端不攜帶或執行此腳本。
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$zip = Join-Path $root 'installer\nsis-3.12.zip'
if ((Get-FileHash $zip -Algorithm SHA256).Hash -ne '56581F90DB321581C5381193D796FFFCF2D24B2F8FED2160A6C6A3BAA67F2C4F') { throw 'NSIS tool archive checksum mismatch.' }
$tools = Join-Path $root '.build\nsis'
if (-not (Test-Path (Join-Path $tools 'nsis-3.12\makensis.exe'))) {
    New-Item -ItemType Directory -Path $tools -Force | Out-Null
    Expand-Archive -LiteralPath $zip -DestinationPath $tools -Force
}
$compiler = Join-Path $tools 'nsis-3.12\makensis.exe'
$version = [regex]::Match((Get-Content (Join-Path $root 'Cargo.toml') -Raw), '(?m)^version = "([^"]+)"').Groups[1].Value
$runtime = Join-Path $root 'dist\MicrosoftEdgeWebView2RuntimeInstallerX64.exe'
$signature = Get-AuthenticodeSignature -LiteralPath $runtime
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') { throw 'WebView2 Microsoft signature validation failed.' }
& $compiler /V2 "/DAPP_VERSION=$version" (Join-Path $root 'installer\LM_AI.nsi')
if ($LASTEXITCODE -ne 0) { throw 'NSIS installer build failed.' }
# 私鑰不包含在離線包；重建者可以編譯驗證，只有持有原私鑰的發行者能發布更新。
if (Test-Path (Join-Path $root '.private\update-key.dpapi')) {
    & (Join-Path $PSScriptRoot 'Update-Signing.ps1') -Kind nsis
    $verify = New-Object Diagnostics.Process
    $verify.StartInfo.FileName = Join-Path $root 'dist\LM_AI.exe'
    $verify.StartInfo.Arguments = '--verify-update-package "' + (Join-Path $root 'dist\update-manifest.json') + '" "' + (Join-Path $root 'dist\LM_AI_Setup.exe') + '"'
    $verify.StartInfo.UseShellExecute = $false
    $verify.StartInfo.CreateNoWindow = $true
    $verify.StartInfo.WindowStyle = 'Hidden'
    if (-not $verify.Start() -or -not $verify.WaitForExit(60000) -or $verify.ExitCode -ne 0) { throw 'Native updater rejected the generated release manifest/package.' }
    $verify.Dispose()
} else {
    $oldManifest = Join-Path $root 'dist\update-manifest.json'
    if (Test-Path $oldManifest) { Remove-Item -LiteralPath $oldManifest }
    Write-Host 'No private update key: installer rebuilt, no publishable update manifest generated.'
}
