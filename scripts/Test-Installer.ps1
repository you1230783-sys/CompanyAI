# 在 C:\largan 的隔離目錄與正式位置驗證 NSIS 安裝、更新、重啟及解除安裝。
# 僅開發機執行；不打包到安裝程式，不用於使用者更新。
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$work = Join-Path $root '.build\installer-test'
New-Item -ItemType Directory -Path $work -Force | Out-Null
$installRoot = [IO.Path]::GetFullPath('C:\largan\LM_AI_Installer_Test')
$allowedRoot = 'C:\largan\'
if (-not $installRoot.StartsWith($allowedRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe test installation path.' }
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN.LM_AI_Installer_Test'
$shortcuts = @((Join-Path ([Environment]::GetFolderPath('Programs')) 'LM_AI_Installer_Test.lnk'), (Join-Path ([Environment]::GetFolderPath('Desktop')) 'LM_AI_Installer_Test.lnk'))
# 正式路徑／登錄／捷徑必須完全未使用，避免驗收覆蓋開發者已安裝的程式。
$releaseRoot = [IO.Path]::GetFullPath('C:\largan\LM_AI')
$releaseKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN.LM_AI'
$releaseShortcuts = @((Join-Path ([Environment]::GetFolderPath('Programs')) 'LM_AI.lnk'), (Join-Path ([Environment]::GetFolderPath('Desktop')) 'LM_AI.lnk'))
foreach ($path in @($installRoot,$key,$releaseRoot,$releaseKey) + $shortcuts + $releaseShortcuts) { if (Test-Path -LiteralPath $path) { throw "Previous test artifacts exist; inspect before retrying: $path" } }
$parentExisted = Test-Path -LiteralPath 'C:\largan'
if ($parentExisted) {
    if ((Get-Item -LiteralPath 'C:\largan').Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Installation parent must not be a link.' }
}
function Start-Hidden([string]$File, [string]$Arguments, [string]$BrowserFolder = '') {
    $process = New-Object Diagnostics.Process
    $process.StartInfo.FileName = $File
    $process.StartInfo.Arguments = $Arguments
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.CreateNoWindow = $true
    $process.StartInfo.WindowStyle = 'Hidden'
    $process.StartInfo.RedirectStandardError = $true
    $process.StartInfo.StandardErrorEncoding = [Text.Encoding]::UTF8
    if ($BrowserFolder) { $process.StartInfo.EnvironmentVariables['WEBVIEW2_BROWSER_EXECUTABLE_FOLDER'] = $BrowserFolder }
    if (-not $process.Start()) { throw "Cannot start $File" }
    return $process
}
function Run-Checked([string]$File, [string]$Arguments, [int]$Expected = 0, [string]$BrowserFolder = '') {
    $process = Start-Hidden $File $Arguments $BrowserFolder
    if (-not $process.WaitForExit(60000)) { $process.Kill(); throw "Timeout: $File" }
    if ($process.ExitCode -ne $Expected) {
        $detail = $process.StandardError.ReadToEnd()
        $code = $process.ExitCode
        $process.Dispose()
        throw "Exit code $code, expected ${Expected}: $File`n$detail"
    }
    $process.Dispose()
}
# 只對測試子程序指定不存在的 Runtime 位置，不移除本機 WebView2 或修改登錄。
# 自檢沿用正式啟動的 WebView2 初始化；失敗文字寫入 stderr，不跳出阻塞對話框。
function Assert-RuntimeUnavailable([string]$File, [string]$BrowserFolder) {
    $process = New-Object Diagnostics.Process
    $process.StartInfo.FileName = $File
    $process.StartInfo.Arguments = '--self-check'
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.CreateNoWindow = $true
    $process.StartInfo.WindowStyle = 'Hidden'
    $process.StartInfo.RedirectStandardError = $true
    $process.StartInfo.StandardErrorEncoding = [Text.Encoding]::UTF8
    $process.StartInfo.EnvironmentVariables['WEBVIEW2_BROWSER_EXECUTABLE_FOLDER'] = $BrowserFolder
    try {
        if (-not $process.Start()) { throw 'Could not start missing-Runtime test.' }
        if (-not $process.WaitForExit(60000)) { $process.Kill(); throw 'Missing-Runtime test timed out.' }
        $message = $process.StandardError.ReadToEnd()
        if ($process.ExitCode -ne 1 -or $message -notmatch '無法啟動 LM_AI 介面' -or $message -notmatch 'MicrosoftEdgeWebView2RuntimeInstallerX64.exe') {
            throw "Missing Runtime did not produce the existing application startup guidance: $message"
        }
    } finally { $process.Dispose() }
}
$compiler = Join-Path $env:VCToolsInstallDir 'bin\Hostx64\x64\cl.exe'
$nsis = Join-Path $root '.build\nsis\nsis-3.12\makensis.exe'
foreach ($generation in @('one','two')) {
    $source = Join-Path $root 'installer\test_payload.cpp'
    $payload = Join-Path $work "$generation.exe"
    & $compiler /nologo /W4 /WX /MT /EHsc /utf-8 "/DTEST_GENERATION=$generation" "/Fo$work\$generation.obj" "/Fe$payload" $source
    if ($LASTEXITCODE -ne 0) { throw 'v142 test payload build failed.' }
    & $nsis /V2 /DAPP_VERSION=98.0.0 /DTEST_PACKAGE /DPRODUCT_DIR=LM_AI_Installer_Test "/DAPP_SOURCE=$payload" "/DSETUP_OUTPUT=$work\$generation-setup.exe" (Join-Path $root 'installer\LM_AI.nsi')
    if ($LASTEXITCODE -ne 0) { throw 'Test installer build failed.' }
}
$app = Join-Path $installRoot 'LM_AI.exe'
$uninstaller = Join-Path $installRoot 'Uninstall.exe'
# /D 即使由呼叫端指定也不可改變固定目錄。
$redirect = Join-Path $work 'must-not-install-here'
if (Test-Path -LiteralPath $redirect) { throw 'Unexpected directory override test artifact.' }
Run-Checked (Join-Path $work 'one-setup.exe') ('/S /D=' + $redirect)
if (Test-Path -LiteralPath $redirect) { throw 'Installer accepted an arbitrary directory override.' }
if ((Get-FileHash $app).Hash -ne (Get-FileHash (Join-Path $work 'one.exe')).Hash) { throw 'First installed payload mismatch.' }
foreach ($path in $shortcuts) { if (-not (Test-Path $path)) { throw 'Missing shortcut.' } }
# 只在開發測試讀取捷徑屬性；使用者安裝／更新仍完全由 NSIS 原生操作完成。
$shortcutReader = New-Object -ComObject WScript.Shell
try {
    foreach ($path in $shortcuts) {
        $shortcut = $shortcutReader.CreateShortcut($path)
        try {
            if ($shortcut.TargetPath -ne $app -or $shortcut.WorkingDirectory -ne $installRoot) { throw 'Shortcut target/working directory does not point to the installed app.' }
        } finally { [Runtime.InteropServices.Marshal]::FinalReleaseComObject($shortcut) | Out-Null }
    }
} finally { [Runtime.InteropServices.Marshal]::FinalReleaseComObject($shortcutReader) | Out-Null }
if ((Get-ItemProperty $key).Publisher -ne 'Largan, Inc.') { throw 'Incorrect uninstall publisher.' }
if ((Get-ItemProperty $key).InstallLocation -ne $installRoot) { throw 'Incorrect fixed install location.' }
if ((Get-Item $uninstaller).VersionInfo.CompanyName -ne 'Largan, Inc.') { throw 'Incorrect uninstaller company name.' }
if ((Get-ItemProperty $key).UninstallString -ne ('"' + $uninstaller + '"')) { throw 'Uninstaller path is not properly quoted.' }
# 保留未知檔案，驗證解除安裝不會遞迴刪除使用者新增內容。
$keep = Join-Path $installRoot 'user-file.txt'
[IO.File]::WriteAllText($keep, 'keep me')
# VNC 設定固定在 EXE 旁；更新與解除安裝都不得改寫使用者的機台及連線選項。
# 僅使用合成資料，測試後依確切檔名清理，不接觸真實機台設定。
$preservedSettings = @{
    'machines.json' = '{"測試分類":[{"name":"保留機台","ip":"192.0.2.1","password":"synthetic-only"}]}'
    'user_config.json' = '{"vnc_path":"","options":{"fullscreen":false,"viewonly":true,"autoscaling":true}}'
}
foreach ($name in $preservedSettings.Keys) {
    [IO.File]::WriteAllText((Join-Path $installRoot $name), $preservedSettings[$name])
}
# 主程式仍持有實例鎖時交棒，NSIS 必須等退出後再更新。
$parent = Start-Hidden $app '--hold'
Start-Sleep -Milliseconds 300
$timer = [Diagnostics.Stopwatch]::StartNew()
Run-Checked (Join-Path $work 'two-setup.exe') "/S /UPDATEPID=$($parent.Id) /RESTART"
if ($timer.ElapsedMilliseconds -lt 1000) { throw 'Installer did not wait for parent exit.' }
$parent.WaitForExit(); $parent.Dispose()
$marker = $app + '.started'
$deadline = [DateTime]::UtcNow.AddSeconds(10)
while (-not (Test-Path $marker) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
if ([IO.File]::ReadAllText($marker) -ne 'two') { throw 'Updated application was not restarted.' }
if ((Get-FileHash $app).Hash -ne (Get-FileHash (Join-Path $work 'two.exe')).Hash) { throw 'Updated payload mismatch.' }
foreach ($name in $preservedSettings.Keys) {
    if ([IO.File]::ReadAllText((Join-Path $installRoot $name)) -cne $preservedSettings[$name]) { throw "Update changed VNC configuration: $name" }
}
# 模擬防毒／其他程序占用主程式：安裝不得刪掉原版。
$locked = [IO.File]::Open($app, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
try { Run-Checked (Join-Path $work 'one-setup.exe') '/S' 1 } finally { $locked.Dispose() }
if ((Get-FileHash $app).Hash -ne (Get-FileHash (Join-Path $work 'two.exe')).Hash) { throw 'Failed installation changed the working app.' }
Run-Checked $uninstaller '/S'
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path $uninstaller) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
foreach ($path in @($app,$uninstaller,$key) + $shortcuts) { if (Test-Path -LiteralPath $path) { throw "Uninstall did not remove $path" } }
if ([IO.File]::ReadAllText($keep) -ne 'keep me') { throw 'Uninstaller deleted user data.' }
foreach ($name in $preservedSettings.Keys) {
    $configPath = Join-Path $installRoot $name
    if ([IO.File]::ReadAllText($configPath) -cne $preservedSettings[$name]) { throw "Uninstall changed VNC configuration: $name" }
    Remove-Item -LiteralPath $configPath
}
# 只清除本次測試建立的已知檔案與空目錄，不做遞迴刪除。
Remove-Item -LiteralPath $keep,$marker
[IO.Directory]::Delete($installRoot, $false)
# 同一 NSIS 來源再安裝真正的 Rust 主程式，驗證解壓後的位元組與 WebView2 介面。
& $nsis /V2 /DAPP_VERSION=98.0.0 /DTEST_PACKAGE /DPRODUCT_DIR=LM_AI_Installer_Test "/DAPP_SOURCE=$root\dist\LM_AI.exe" "/DSETUP_OUTPUT=$work\real-setup.exe" (Join-Path $root 'installer\LM_AI.nsi')
if ($LASTEXITCODE -ne 0) { throw 'Real payload test installer build failed.' }
Run-Checked (Join-Path $work 'real-setup.exe') '/S'
if ((Get-FileHash $app).Hash -ne (Get-FileHash (Join-Path $root 'dist\LM_AI.exe')).Hash) { throw 'Installed Rust app differs from release.' }
Run-Checked $app '--self-check'
Run-Checked $uninstaller '/S'
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path $installRoot) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
if (Test-Path $installRoot) { throw 'Real payload test uninstall incomplete.' }
# 最後安裝真正交付的 Setup，不以測試包取代此驗證。
$releaseSetup = Join-Path $root 'dist\LM_AI_Setup.exe'
$releaseApp = Join-Path $releaseRoot 'LM_AI.exe'
$releaseUninstaller = Join-Path $releaseRoot 'Uninstall.exe'
# 預先存在的產品目錄及其他檔案必須保留；首次測試已涵蓋目錄不存在的情況。
New-Item -ItemType Directory -Path $releaseRoot | Out-Null
$releaseKeep = Join-Path $releaseRoot 'user-file.txt'
[IO.File]::WriteAllText($releaseKeep, 'keep release data')
# 安裝器不依賴 Runtime；主程式啟動時才處理無法載入的情況。
$missingBrowserFolder = Join-Path $work ('missing-runtime-' + [guid]::NewGuid().ToString('N'))
if (Test-Path -LiteralPath $missingBrowserFolder) { throw 'Missing-Runtime test path must not exist.' }
Run-Checked $releaseSetup ('/S /D=' + $redirect) 0 $missingBrowserFolder
if (Test-Path -LiteralPath $redirect) { throw 'Release installer accepted a directory override.' }
if ((Get-FileHash $releaseApp).Hash -ne (Get-FileHash (Join-Path $root 'dist\LM_AI.exe')).Hash) { throw 'Release installation differs from the delivered EXE.' }
foreach ($file in @($releaseApp,$releaseSetup,$releaseUninstaller)) {
    if ((Get-Item $file).VersionInfo.CompanyName -ne 'Largan, Inc.') { throw "Incorrect release company name: $file" }
}
$registration = Get-ItemProperty $releaseKey
if ($registration.Publisher -ne 'Largan, Inc.' -or $registration.InstallLocation -ne $releaseRoot) { throw 'Incorrect release publisher or install location.' }
$shortcutReader = New-Object -ComObject WScript.Shell
try {
    foreach ($path in $releaseShortcuts) {
        if (-not (Test-Path -LiteralPath $path)) { throw 'Missing release shortcut.' }
        $shortcut = $shortcutReader.CreateShortcut($path)
        try {
            if ($shortcut.TargetPath -ne $releaseApp -or $shortcut.WorkingDirectory -ne $releaseRoot) { throw 'Incorrect release shortcut target or working directory.' }
        } finally { [Runtime.InteropServices.Marshal]::FinalReleaseComObject($shortcut) | Out-Null }
    }
} finally { [Runtime.InteropServices.Marshal]::FinalReleaseComObject($shortcutReader) | Out-Null }
Assert-RuntimeUnavailable $releaseApp $missingBrowserFolder
Run-Checked $releaseApp '--self-check'
Run-Checked $releaseUninstaller '/S'
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path $releaseUninstaller) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
foreach ($path in @($releaseApp,$releaseUninstaller,$releaseKey) + $releaseShortcuts) {
    if (Test-Path -LiteralPath $path) { throw "Release uninstall did not remove $path" }
}
if ([IO.File]::ReadAllText($releaseKeep) -ne 'keep release data') { throw 'Release install/uninstall changed existing user data.' }
# 僅刪除此測試建立的已知檔案及空目錄，保留公司共用的 C:\largan。
Remove-Item -LiteralPath $releaseKeep
[IO.Directory]::Delete($releaseRoot, $false)
[ordered]@{
    result = 'PASS'
    recorded_at = (Get-Date -Format o)
    version = (Get-Item $releaseSetup).VersionInfo.FileVersion
    toolset = $env:VCToolsVersion
    publisher = 'Largan, Inc.'
    install_path = $releaseRoot
    parent_existed_before_test = $parentExisted
    setup_sha256 = (Get-FileHash $releaseSetup -Algorithm SHA256).Hash.ToLowerInvariant()
    vnc_configuration_preserved = $true
    webview2_bundled = $false
    runtime_check_stage = 'application startup only; no installer prerequisite check'
    runtime_unavailable_simulation = 'child-only WEBVIEW2_BROWSER_EXECUTABLE_FOLDER points to a nonexistent folder; Setup succeeds, installed app exits with existing guidance'
    checks = @('NSIS install into missing product directory','existing directory preserves user files','fixed path ignores /D','shortcuts and registration','EXE/setup/uninstaller company name','PID handoff wait','update replacement','automatic restart','locked-file failure preserves old app','NSIS uninstall preserves unknown files','actual release Setup installation and WebView2 self-check','actual release uninstall')
    scope = 'isolated native payload and actual release Setup at C:\largan\LM_AI; corporate antivirus and Outlook require company testing'
} | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $root 'offline\installer-verification.json') -Encoding UTF8
Write-Host 'NSIS installation/update/restart/failure/uninstall tests: PASS'
