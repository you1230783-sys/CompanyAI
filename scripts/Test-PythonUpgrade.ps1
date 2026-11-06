# 真正的前版 Setup → 本版 Setup，補足合成交易測試不能證明的 runtime 升級與快取鎖交接。
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$PreviousSetup,
      [Parameter(Mandatory=$true)][string]$PreviousRuntime,
      [string]$Setup, [string]$CacheProbe)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (-not $Setup) { $Setup = Join-Path $root 'dist\LM_AI_Setup.exe' }
if (-not $CacheProbe) { $CacheProbe = Join-Path $root 'target\x86_64-pc-windows-msvc\debug\examples\python_measure.exe' }
$install = [IO.Path]::GetFullPath('C:\largan\LM_AI')
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN.LM_AI'
$shortcuts = @((Join-Path ([Environment]::GetFolderPath('Programs')) 'LM_AI.lnk'), (Join-Path ([Environment]::GetFolderPath('Desktop')) 'LM_AI.lnk'))
foreach ($path in @($install, $key) + $shortcuts) { if (Test-Path -LiteralPath $path) { throw "Existing installation must not be overwritten: $path" } }
if ($install -cne 'C:\largan\LM_AI' -or ((Test-Path C:\largan) -and ((Get-Item C:\largan).Attributes -band [IO.FileAttributes]::ReparsePoint))) { throw 'Unsafe installation parent.' }
$work = Join-Path $root ('.build\python-upgrade-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$sequence = 0
function Start-TestProcess([string]$file, [string]$arguments) {
    $script:sequence++
    $p = New-Object Diagnostics.Process
    $p.StartInfo.FileName = [IO.Path]::GetFullPath($file)
    $p.StartInfo.Arguments = $arguments
    $p.StartInfo.UseShellExecute = $false
    $p.StartInfo.CreateNoWindow = $true
    $p.StartInfo.WindowStyle = 'Hidden'
    # GUI 與清理報告使用測試 profile；公告狀態另在 EXE 旁建立並核對保留。
    # Python 隔離程序必須使用 Windows 真正的 AppContainer profile：
    # 改寫 LOCALAPPDATA 只會改變程式推算的路徑，不會搬動 OS 的 AC\Temp。
    if ($arguments -in @('--demo', '--cleanup-old-python-runtimes')) {
        $p.StartInfo.EnvironmentVariables['LOCALAPPDATA'] = Join-Path $work 'profile'
    }
    $p.StartInfo.RedirectStandardOutput = $true
    $p.StartInfo.RedirectStandardError = $true
    if (-not $p.Start()) { throw "Cannot start: $file" }
    return [pscustomobject]@{process=$p;stdout=$p.StandardOutput.ReadToEndAsync();stderr=$p.StandardError.ReadToEndAsync();id=$script:sequence}
}
function Finish-TestProcess($run, [int]$expected = 0) {
    $p = $run.process
    if (-not $p.WaitForExit(240000)) { $p.Kill(); throw 'Python installer test timed out.' }
    $run.stdout.Result | Set-Content (Join-Path $work "$($run.id).out")
    $run.stderr.Result | Set-Content (Join-Path $work "$($run.id).err")
    if ($p.ExitCode -ne $expected) { throw "Exit $($p.ExitCode), expected $expected; see $work\$($run.id).err" }
    $p.Dispose()
}
function Run-TestProcess([string]$file, [string]$arguments, [int]$expected = 0) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $run = Start-TestProcess $file $arguments
    Finish-TestProcess $run $expected
    return $timer.Elapsed.TotalMilliseconds
}
function Python-Trash { @(Get-ChildItem -LiteralPath $install -Directory | Where-Object Name -Match '^python\.trash-[0-9a-f]{32}$') }
function Read-OnboardingState {
    # 等待狀態時不能自己鎖住公告檔；允許應用程式原子替換，讀取者取得
    # 完整的舊版或新版內容即可，不會讀到半份 JSON。
    $stream = [IO.FileStream]::new($onboardingFile, [IO.FileMode]::Open, [IO.FileAccess]::Read,
        ([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
    $reader = [IO.StreamReader]::new($stream)
    try { return ($reader.ReadToEnd() | ConvertFrom-Json) } finally { $reader.Dispose() }
}
$oldInstallMs = Run-TestProcess $PreviousSetup '/S'
$app = Join-Path $install 'LM_AI.exe'
if ((Get-Item $app).VersionInfo.FileVersion -ne '0.9.18.0') { throw 'Previous Setup is not 0.9.18.' }
$oldHash = (Get-FileHash (Join-Path $install 'python\runtime-manifest.json')).Hash
$userFile = Join-Path $install 'python-upgrade-user-data.txt'
[IO.File]::WriteAllText($userFile, 'synthetic user data must survive')
# 此案例從空目錄開始，這份教學／公告狀態完全由測試建立；也要跨升級與移除保留。
$onboardingFile = Join-Path $install 'onboarding-state.json'
[IO.File]::WriteAllText($onboardingFile, '{"last_version":"0.9.18","release_pending":false,"tutorials":{}}')
$upgradeMs = Run-TestProcess $Setup '/S'
if ((Get-Item $app).VersionInfo.FileVersion -ne '0.9.19.0') { throw 'Upgrade did not install 0.9.19.' }
if ((Get-FileHash $app).Hash -ne (Get-FileHash (Join-Path $root 'dist\LM_AI.exe')).Hash) { throw 'Installed EXE differs.' }
$trash = @(Python-Trash)
if ($trash.Count -ne 1) { throw 'Upgrade must defer exactly one old runtime.' }
$old = $trash[0].FullName
if ((Get-FileHash (Join-Path $old 'runtime-manifest.json')).Hash -ne $oldHash) { throw 'Deferred runtime is not the original 0.9.18 runtime.' }
Run-TestProcess $app '--python-self-check' | Out-Null
# 額外檔案與占用都須保留整棵舊樹，移除本案例標記後才能交給背景清理。
$extra = Join-Path $old 'synthetic-extra.txt'
[IO.File]::WriteAllText($extra, 'keep')
Run-TestProcess $app '--cleanup-old-python-runtimes' 2 | Out-Null
if (-not (Test-Path (Join-Path $old 'python.exe')) -or [IO.File]::ReadAllText($extra) -ne 'keep') { throw 'Extra-file refusal partially deleted runtime.' }
Remove-Item -LiteralPath $extra
$readonlyPath = Join-Path $old 'lm_worker.py'
$originalAttributes = [IO.File]::GetAttributes($readonlyPath)
[IO.File]::SetAttributes($readonlyPath, $originalAttributes -bor [IO.FileAttributes]::ReadOnly)
try { Run-TestProcess $app '--cleanup-old-python-runtimes' 2 | Out-Null } finally { [IO.File]::SetAttributes($readonlyPath, $originalAttributes) }
if (-not (Test-Path (Join-Path $old 'python.exe'))) { throw 'Read-only refusal partially deleted runtime.' }
$held = [IO.File]::Open((Join-Path $old 'lm_worker.py'), 'Open', 'Read', 'Read')
try { Run-TestProcess $app '--cleanup-old-python-runtimes' 2 | Out-Null } finally { $held.Dispose() }
if (-not (Test-Path (Join-Path $old 'python.exe'))) { throw 'Busy refusal partially deleted runtime.' }
$gui = Start-TestProcess $app '--demo'
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    while ((Test-Path -LiteralPath $old) -and -not $gui.process.HasExited -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
    if (Test-Path -LiteralPath $old) { throw 'Normal application startup did not clean verified Python trash.' }
    if ($gui.process.HasExited) { throw 'Demo application exited before startup cleanup verification.' }
    $stateDeadline = [DateTime]::UtcNow.AddSeconds(120)
    do {
        $onboarding = Read-OnboardingState
        if ($onboarding.last_version -eq '0.9.19') { break }
        Start-Sleep -Milliseconds 200
    } while (-not $gui.process.HasExited -and [DateTime]::UtcNow -lt $stateDeadline)
    if ($onboarding.last_version -ne '0.9.19' -or -not $onboarding.release_pending -or $onboarding.release_from -ne '0.9.18') { throw 'Upgrade startup did not retain the expected cumulative release range.' }
} finally {
    # 只結束此測試啟動的 demo；不尋找或關閉其他 LM_AI／Office 程序。
    if (-not $gui.process.HasExited) { $gui.process.Kill(); $gui.process.WaitForExit() }
    $gui.stdout.Result | Set-Content (Join-Path $work 'demo.out')
    $gui.stderr.Result | Set-Content (Join-Path $work 'demo.err')
    $gui.process.Dispose()
}
# 安裝前的 PID 必須正常退出並釋放真正的全域 cache handles；不能只測一個人工檔案鎖。
$probe = Join-Path $install 'python_cache_probe.exe'
Copy-Item -LiteralPath $CacheProbe -Destination $probe
Run-TestProcess $probe '--prepare' | Out-Null
$lockRun = Start-TestProcess $probe '--hold'
# 以檔案寫入拒絕確認真實驗證已持鎖，再等待足夠時間讓兩次量測完成。
$worker = Join-Path $install 'python\lm_worker.py'
$deadline = [DateTime]::UtcNow.AddSeconds(60)
$locked = $false
while (-not $locked -and -not $lockRun.process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
    try { $handle=[IO.File]::Open($worker,'Open','Write','Read'); $handle.Dispose() } catch [IO.IOException] { $locked=$true }
    if (-not $locked) { Start-Sleep -Milliseconds 100 }
}
if (-not $locked) { throw 'Cache probe did not hold runtime files.' }
$pidMs = Run-TestProcess $Setup ("/S /UPDATEPID=" + $lockRun.process.Id)
Finish-TestProcess $lockRun
if ($lockRun.stdout.Result -notmatch 'RUNTIME_LOCKS_HELD') { throw 'Cache probe did not finish both analyses before exiting.' }
Remove-Item -LiteralPath $probe
if (@(Python-Trash).Count -ne 0) { throw 'Unchanged Python should be reused during PID handoff.' }
# 再放回已知舊 runtime，驗證解除安裝確實呼叫相同清理入口。
$uninstallTrash = Join-Path $install ('python.trash-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $uninstallTrash | Out-Null
Get-ChildItem -LiteralPath $PreviousRuntime | Copy-Item -Destination $uninstallTrash -Recurse
if ((Get-FileHash (Join-Path $uninstallTrash 'runtime-manifest.json')).Hash -ne $oldHash) { throw 'Uninstall fixture differs from previous runtime.' }
Run-TestProcess (Join-Path $install 'Uninstall.exe') '/S' | Out-Null
$deadline = [DateTime]::UtcNow.AddSeconds(120)
while (((Test-Path $key) -or (Test-Path $uninstallTrash)) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
if ((Test-Path $key) -or (Test-Path $uninstallTrash) -or (Test-Path $app)) { throw 'Uninstall left verified runtime or app.' }
if ([IO.File]::ReadAllText($userFile) -ne 'synthetic user data must survive') { throw 'User data changed during upgrade/uninstall.' }
if ((Get-Content -LiteralPath $onboardingFile -Raw | ConvertFrom-Json).last_version -ne '0.9.19') { throw 'Uninstall removed onboarding data.' }
# 保留測試產生的狀態作證據，不能為了清空目錄而讓安裝器刪除使用者設定。
$savedOnboarding = [IO.Path]::GetFullPath((Join-Path $work 'preserved-onboarding-state.json'))
if ([IO.Path]::GetDirectoryName($onboardingFile) -cne $install -or [IO.Path]::GetDirectoryName($savedOnboarding) -cne $work) { throw 'Unsafe fixture state move.' }
Move-Item -LiteralPath $onboardingFile -Destination $savedOnboarding
Remove-Item -LiteralPath $userFile
[IO.Directory]::Delete($install, $false)
[ordered]@{
    result = 'PASS'
    version = '0.9.19'
    previous = '0.9.18'
    old_install_ms = $oldInstallMs
    upgrade_ms = $upgradeMs
    cached_pid_handoff_ms = $pidMs
    startup_cleanup = $true
    extra_file_and_busy_refusal = $true
    read_only_refusal = $true
    uninstall_cleanup = $true
    user_data_preserved = $true
    release_range_preserved = $true
    work = $work
    setup_sha256 = (Get-FileHash $Setup).Hash.ToLowerInvariant()
} | ConvertTo-Json | Set-Content (Join-Path $root '.build\python-upgrade-0919.json')
Get-Content (Join-Path $root '.build\python-upgrade-0919.json')
