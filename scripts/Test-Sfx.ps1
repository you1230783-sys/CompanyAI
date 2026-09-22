# 執行真正的自解壓縮 EXE，驗證預設路徑、替換與設定保留；不關閉防毒。
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$destination = [IO.Path]::GetFullPath('C:\largan\LM_AI')
$parent = Split-Path $destination -Parent
if ($destination -ne 'C:\largan\LM_AI') { throw 'Unexpected SFX test destination.' }
if (Test-Path -LiteralPath $destination) { throw 'Default destination is in use; inspect it before testing. No existing installation was changed.' }
if ((Test-Path -LiteralPath $parent) -and ((Get-Item -LiteralPath $parent).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'SFX test parent must not be a link.'
}
$package = Join-Path $root 'dist\LM_AI_SFX_Test.exe'
$source = Join-Path $root 'dist\LM_AI.exe'
$app = Join-Path $destination 'LM_AI.exe'
$preserved = @{
    'machines.json' = '{"測試":[{"name":"機台","ip":"192.0.2.1","password":"synthetic-only"}]}'
    'user_config.json' = '{"vnc_path":"","options":{"fullscreen":false,"viewonly":true,"autoscaling":true}}'
    'user-file.txt' = 'preserve existing user data'
}
# 記錄原有捷徑與解除安裝登錄，確認 SFX 沒有新增或更改這些項目。
$shortcutPaths = @(
    (Join-Path ([Environment]::GetFolderPath('Desktop')) 'LM_AI.lnk'),
    (Join-Path ([Environment]::GetFolderPath('Programs')) 'LM_AI.lnk')
)
$shortcutBefore = @{}
foreach ($path in $shortcutPaths) {
    $shortcutBefore[$path] = if (Test-Path -LiteralPath $path) { (Get-FileHash -LiteralPath $path).Hash } else { '' }
}
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN.LM_AI'
$registrationBefore = if (Test-Path $key) { Get-ItemProperty $key | ConvertTo-Json -Compress } else { '' }
function Invoke-TestProcess([string]$File, [string]$Arguments) {
    $process = Start-Process -FilePath $File -ArgumentList $Arguments -PassThru -WindowStyle Hidden
    try {
        if (-not $process.WaitForExit(60000)) { $process.Kill(); throw "SFX test timed out: $File" }
        if ($process.ExitCode -ne 0) { throw "SFX test failed ($($process.ExitCode)): $File" }
    } finally { $process.Dispose() }
}
function Assert-ExtractedApp {
    if ((Get-FileHash -LiteralPath $app).Hash -ne (Get-FileHash -LiteralPath $source).Hash) {
        throw 'Extracted EXE differs from the release EXE.'
    }
}
# -s 僅讓自動測試不顯示視窗；不指定 -d，以實測包內的預設路徑。
Invoke-TestProcess $package '-s'
Assert-ExtractedApp
foreach ($name in $preserved.Keys) { [IO.File]::WriteAllText((Join-Path $destination $name), $preserved[$name]) }
[IO.File]::WriteAllText($app, 'synthetic old payload')
Invoke-TestProcess $package '-s'
Assert-ExtractedApp
foreach ($name in $preserved.Keys) {
    if ([IO.File]::ReadAllText((Join-Path $destination $name)) -cne $preserved[$name]) { throw "SFX changed user file: $name" }
}
Invoke-TestProcess $app '--self-check'
foreach ($path in $shortcutPaths) {
    $after = if (Test-Path -LiteralPath $path) { (Get-FileHash -LiteralPath $path).Hash } else { '' }
    if ($after -cne $shortcutBefore[$path]) { throw "SFX changed shortcut: $path" }
}
$registrationAfter = if (Test-Path $key) { Get-ItemProperty $key | ConvertTo-Json -Compress } else { '' }
if ($registrationAfter -cne $registrationBefore) { throw 'SFX changed uninstall registration.' }
# 只清除已知測試檔與空目錄；失敗則保留現場，不遞迴刪除。
foreach ($name in $preserved.Keys) { Remove-Item -LiteralPath (Join-Path $destination $name) }
Remove-Item -LiteralPath $app
[IO.Directory]::Delete($destination, $false)
[ordered]@{
    recorded_at = (Get-Date -Format o)
    result = 'PASS'
    app_version = (Get-Item $source).VersionInfo.FileVersion
    package = 'LM_AI_SFX_Test.exe'
    package_bytes = (Get-Item $package).Length
    package_sha256 = (Get-FileHash $package -Algorithm SHA256).Hash.ToLowerInvariant()
    app_sha256 = (Get-FileHash $source -Algorithm SHA256).Hash.ToLowerInvariant()
    default_path_tested = $destination
    packager = 'Official WinRAR 7.23 x64 Traditional Chinese, unmodified Default.SFX'
    checks = @('archive contains only LM_AI.exe','archive integrity','actual SFX default extraction path','replace synthetic old EXE','VNC configuration and unknown file preservation','extracted EXE hash and WebView2 self-check','no shortcut or uninstall registration changes','test files cleaned')
    corporate_antivirus_tested = $false
    interactive_dialog_visually_verified = $false
    automatic_update_supported = $false
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'offline\sfx-verification.json') -Encoding UTF8
Write-Host 'SFX extraction/replacement/configuration preservation/self-check: PASS'
