# 僅驗證當版實際安裝包；未變動的 PID／回復／修復矩陣仍見最近完整 Test-Installer 證據。
# 此腳本不取代完整驗收。若 installer 或 runtime 有改動，請執行 Test-Installer.ps1。
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$releaseRoot = [IO.Path]::GetFullPath('C:\largan\LM_AI')
if ($releaseRoot -cne 'C:\largan\LM_AI') { throw 'Unexpected installation path.' }
$releaseKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN.LM_AI'
$releaseShortcuts = @((Join-Path ([Environment]::GetFolderPath('Programs')) 'LM_AI.lnk'), (Join-Path ([Environment]::GetFolderPath('Desktop')) 'LM_AI.lnk'))
foreach ($path in @($releaseRoot,$releaseKey) + $releaseShortcuts) { if (Test-Path -LiteralPath $path) { throw "Existing installation or test artifacts; inspect before retrying: $path" } }
if ((Test-Path -LiteralPath 'C:\largan') -and ((Get-Item -LiteralPath 'C:\largan').Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Installation parent must not be a link.' }
if ($env:VCToolsVersion -notlike '14.2*') { throw 'Initialize v142 x64 first.' }
$work = Join-Path $root ('.build\release-installer-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$redirect = Join-Path $work 'must-not-install-here'
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
    if (-not $process.WaitForExit(240000)) { $process.Kill(); throw "Timeout: $File" }
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
Run-Checked $releaseApp '--python-self-check'
Run-Checked $releaseApp '--cad-self-check'
# 同一版的更新仍必須保留使用者檔案及既有 runtime；沒有另建四份測試安裝包。
$runtimeWorker = Join-Path $releaseRoot 'python\lm_worker.py'
$creationTimes = @{}
foreach ($file in Get-ChildItem -LiteralPath (Join-Path $releaseRoot 'python') -File -Recurse) { $creationTimes[$file.FullName] = $file.CreationTimeUtc.Ticks }
$runtimeLock = [IO.File]::Open($runtimeWorker, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
try { Run-Checked $releaseSetup '/S' } finally { $runtimeLock.Dispose() }
foreach ($path in $creationTimes.Keys) { if ((Get-Item -LiteralPath $path).CreationTimeUtc.Ticks -ne $creationTimes[$path]) { throw 'Unchanged runtime was replaced during update.' } }
if ((Get-FileHash $releaseApp).Hash -ne (Get-FileHash (Join-Path $root 'dist\LM_AI.exe')).Hash) { throw 'Updated release EXE differs.' }
if ([IO.File]::ReadAllText($releaseKeep) -ne 'keep release data') { throw 'Update changed existing user data.' }
Run-Checked $releaseApp '--self-check'
Run-Checked $releaseApp '--python-self-check'
Run-Checked $releaseApp '--cad-self-check'
Run-Checked $releaseUninstaller '/S'
$deadline = [DateTime]::UtcNow.AddSeconds(60)
while (((Test-Path $releaseUninstaller) -or (Test-Path $releaseKey)) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
foreach ($path in @($releaseApp,$releaseUninstaller,$releaseKey) + $releaseShortcuts) {
    if (Test-Path -LiteralPath $path) { throw "Release uninstall did not remove $path" }
}
if ([IO.File]::ReadAllText($releaseKeep) -ne 'keep release data') { throw 'Release install/uninstall changed existing user data.' }
# 僅刪除此測試建立的已知檔案及空目錄，保留公司共用的 C:\largan。
Remove-Item -LiteralPath $releaseKeep
[IO.Directory]::Delete($releaseRoot, $false)
[ordered]@{
    result = 'PASS_RELEASE_PACKAGE'
    version = (Get-Item $releaseSetup).VersionInfo.FileVersion
    recorded_at = (Get-Date -Format o)
    toolset = $env:VCToolsVersion
    setup_sha256 = (Get-FileHash $releaseSetup -Algorithm SHA256).Hash.ToLowerInvariant()
    app_sha256 = (Get-FileHash (Join-Path $root 'dist\LM_AI.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    install_path = $releaseRoot
    checks = @('actual candidate Setup install','fixed path ignores /D','company metadata, registry and shortcut targets','missing WebView2 child-only guidance','installed WebView2/self-check including feature receipts','installed Python and CAD self-check','same-package update preserves user data and runtime identity','updated EXE/self-check/Python/CAD','uninstall preserves unknown user file')
    not_rerun = @('PID handoff and automatic restart','locked-file rollback','runtime repair matrix','obsolete CAD cleanup matrix')
    prior_full_matrix = 'offline/installer-verification-0.9.15.json'
    scope = 'actual candidate only; installer and runtime unchanged; company environment remains separate'
} | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $root 'offline\installer-verification.json') -Encoding UTF8
Write-Host 'Actual release installation/update/startup/uninstall: PASS_RELEASE_PACKAGE'
