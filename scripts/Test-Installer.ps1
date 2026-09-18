# 在隔離的使用者目錄實際驗證 NSIS 安裝、等待退出、重新啟動、失敗保留及解除安裝。
# 僅開發機執行；不打包到安裝程式，不用於使用者更新。
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$work = Join-Path $root '.build\installer-test'
New-Item -ItemType Directory -Path $work -Force | Out-Null
$installRoot = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Programs\LM_AI_Installer_Test'))
$allowedRoot = [IO.Path]::GetFullPath($env:LOCALAPPDATA).TrimEnd('\') + '\Programs\'
if (-not $installRoot.StartsWith($allowedRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe test installation path.' }
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN.LM_AI_Installer_Test'
$shortcuts = @((Join-Path ([Environment]::GetFolderPath('Programs')) 'LM_AI_Installer_Test.lnk'), (Join-Path ([Environment]::GetFolderPath('Desktop')) 'LM_AI_Installer_Test.lnk'))
foreach ($path in @($installRoot,$key) + $shortcuts) { if (Test-Path -LiteralPath $path) { throw "Previous test artifacts exist; inspect before retrying: $path" } }
function Start-Hidden([string]$File, [string]$Arguments) {
    $process = New-Object Diagnostics.Process
    $process.StartInfo.FileName = $File
    $process.StartInfo.Arguments = $Arguments
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.CreateNoWindow = $true
    $process.StartInfo.WindowStyle = 'Hidden'
    if (-not $process.Start()) { throw "Cannot start $File" }
    return $process
}
function Run-Checked([string]$File, [string]$Arguments, [int]$Expected = 0) {
    $process = Start-Hidden $File $Arguments
    if (-not $process.WaitForExit(60000)) { $process.Kill(); throw "Timeout: $File" }
    if ($process.ExitCode -ne $Expected) { throw "Exit code $($process.ExitCode), expected ${Expected}: $File" }
    $process.Dispose()
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
Run-Checked (Join-Path $work 'one-setup.exe') '/S'
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
if ((Get-ItemProperty $key).Publisher -ne 'LARGAN') { throw 'Missing uninstall registration.' }
if ((Get-ItemProperty $key).UninstallString -ne ('"' + $uninstaller + '"')) { throw 'Uninstaller path is not properly quoted.' }
# 保留未知檔案，驗證解除安裝不會遞迴刪除使用者新增內容。
$keep = Join-Path $installRoot 'user-file.txt'
[IO.File]::WriteAllText($keep, 'keep me')
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
# 模擬防毒／其他程序占用主程式：安裝不得刪掉原版。
$locked = [IO.File]::Open($app, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
try { Run-Checked (Join-Path $work 'one-setup.exe') '/S' 1 } finally { $locked.Dispose() }
if ((Get-FileHash $app).Hash -ne (Get-FileHash (Join-Path $work 'two.exe')).Hash) { throw 'Failed installation changed the working app.' }
Run-Checked $uninstaller '/S'
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path $uninstaller) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
foreach ($path in @($app,$uninstaller,$key) + $shortcuts) { if (Test-Path -LiteralPath $path) { throw "Uninstall did not remove $path" } }
if ([IO.File]::ReadAllText($keep) -ne 'keep me') { throw 'Uninstaller deleted user data.' }
# 只清除本次測試建立的兩個已知檔案與空目錄，不做遞迴刪除。
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
[ordered]@{result='PASS';recorded_at=(Get-Date -Format o);toolset=$env:VCToolsVersion;checks=@('NSIS install','shortcuts and registration','PID handoff wait','update replacement','automatic restart','locked-file failure preserves old app','NSIS uninstall preserves unknown files');scope='isolated native payload, same NSIS source; corporate antivirus requires company testing'} | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $root 'offline\installer-verification.json') -Encoding UTF8
Write-Host 'NSIS installation/update/restart/failure/uninstall tests: PASS'
