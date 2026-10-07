[CmdletBinding()]
param([switch]$IncludeInstaller, [switch]$EmptyCargoCache, [switch]$ValidateOnly, [switch]$TestVnc, [switch]$TestOffice, [string]$NetworkTestRoot)
$ErrorActionPreference = 'Stop'
if ($ValidateOnly -and $IncludeInstaller) { throw 'ValidateOnly cannot be combined with IncludeInstaller.' }
# 統一載入 v142 與指定 SDK，讓手動執行及未來網頁呼叫使用相同編譯環境。
. (Join-Path $PSScriptRoot 'Enter-DevShell.ps1')
# 空快取驗證直接讀取專案 vendor，不依賴開發機已有的 registry 下載。
if ($EmptyCargoCache) {
    $env:CARGO_HOME = Join-Path $projectRoot ('.build\cargo-empty-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $env:CARGO_HOME | Out-Null
}
Push-Location $projectRoot
try {
    # 不只相信環境變數：以指定 cl.exe 編譯小型探針，確認真正的 _MSC_VER 與 x64。
    # 此檔僅供建置驗證，不連入 Rust EXE，也不加入交付包。
    $probeRoot = Join-Path $projectRoot '.build\compiler-check'
    New-Item -ItemType Directory -Path $probeRoot -Force | Out-Null
    $probeSource = Join-Path $probeRoot 'v142.c'
    $probeObject = Join-Path $probeRoot 'v142.obj'
    @'
#if !defined(_MSC_VER) || _MSC_VER < 1920 || _MSC_VER > 1929
#error CompanyAI requires MSVC v142 (_MSC_VER 1920-1929).
#endif
#ifndef _M_X64
#error CompanyAI requires the x64 compiler.
#endif
#define PROBE_STRING_INNER(value) #value
#define PROBE_STRING(value) PROBE_STRING_INNER(value)
#pragma message("Verified _MSC_FULL_VER=" PROBE_STRING(_MSC_FULL_VER) " x64")
int company_ai_toolset_probe(void) { return _MSC_VER; }
'@ | Set-Content -LiteralPath $probeSource -Encoding ASCII
    $compiler = Join-Path $env:VCToolsInstallDir 'bin\Hostx64\x64\cl.exe'
    $compilerReport = & $compiler /nologo /c /WX "/Fo$probeObject" $probeSource
    if ($LASTEXITCODE -ne 0) { throw 'Actual compiler is not a working MSVC v142 x64 toolset.' }
    $compilerReport | ForEach-Object { Write-Host $_ }
    # 先檢查格式及常見錯誤；任一步失敗就停止，避免回報過期的編譯結果。
    & cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'Formatting check failed.' }
    & cargo clippy --workspace --all-targets --frozen -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Clippy failed.' }
    # 測試涵蓋登入兌換、重複登入碼、API Header 與 Windows 憑證加密。
    & cargo test --workspace --frozen
    if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
    & cargo build --workspace --release --frozen
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    $exe = Join-Path $projectRoot 'target\x86_64-pc-windows-msvc\release\company-ai.exe'
    & (Join-Path $PSScriptRoot 'Stage-Python.ps1') -ExecutableDirectory (Split-Path $exe -Parent)
    foreach ($argument in @('--prepare-python-runtime','--python-self-check')) {
        $pythonCheck = Start-Process -FilePath $exe -ArgumentList $argument -PassThru -Wait -WindowStyle Hidden
        if ($pythonCheck.ExitCode -ne 0) { throw "Python runtime check failed: $argument" }
    }
    $composerReport = Join-Path ([IO.Path]::GetTempPath()) 'CompanyAI-ui-smoke\composer-verification.json'
    if (Test-Path -LiteralPath $composerReport) { Remove-Item -LiteralPath $composerReport }
    # GUI 程式以隱藏的自我檢查模式驗證控制項，避免編譯腳本停在主視窗。
    $smokeCheck = New-Object Diagnostics.Process
    $smokeCheck.StartInfo.FileName = $exe
    $smokeCheck.StartInfo.Arguments = '--self-check'
    $smokeCheck.StartInfo.UseShellExecute = $false
    $smokeCheck.StartInfo.CreateNoWindow = $true
    $smokeCheck.StartInfo.WindowStyle = 'Hidden'
    $smokeCheck.StartInfo.RedirectStandardError = $true
    try {
        if (-not $smokeCheck.Start()) { throw 'Could not start executable UI smoke check.' }
        if (-not $smokeCheck.WaitForExit(60000)) {
            $smokeCheck.Kill()
            throw 'Executable UI smoke check timed out.'
        }
        if ($smokeCheck.ExitCode -ne 0) { throw ('Executable UI smoke check failed: ' + $smokeCheck.StandardError.ReadToEnd()) }
    } finally { $smokeCheck.Dispose() }
    # 此檔由本次 EXE 原生控制器自檢產生；缺檔或失敗不能沿用舊驗證紀錄。
    $composerCheck = Get-Content -LiteralPath $composerReport -Raw | ConvertFrom-Json
    if ($composerCheck.result -ne 'PASS' -or $composerCheck.cases.Count -lt 14) {
        throw 'Native project composer verification failed.'
    }
    # 真正啟動隔離 EXE，驗證 OS 邊界及副本編輯；失敗不可發布看似可用的文件功能。
    & cargo build --example project_smoke --frozen
    if ($LASTEXITCODE -ne 0) { throw 'Project integration harness build failed.' }
    $projectProbe = Join-Path $projectRoot 'target\x86_64-pc-windows-msvc\debug\examples\project_smoke.exe'
    & $projectProbe $exe (Join-Path $projectRoot '.build\project-smoke')
    if ($LASTEXITCODE -ne 0) { throw 'Real AppContainer/project file integration failed.' }
    & cargo build --example python_smoke --release --frozen
    if ($LASTEXITCODE -ne 0) { throw 'Python integration harness build failed.' }
    $pythonProbe = Join-Path $projectRoot 'target\x86_64-pc-windows-msvc\release\examples\python_smoke.exe'
    & (Join-Path $PSScriptRoot 'Stage-Python.ps1') -ExecutableDirectory (Split-Path $pythonProbe -Parent)
    $pythonTestRoot = Join-Path $projectRoot ('.build\python-smoke-' + [guid]::NewGuid().ToString('N'))
    $pythonArguments = @($exe, $pythonTestRoot)
    if ($TestOffice) { $pythonArguments += '--office' }
    # 合成標記確認任意父程序環境不會流入模型程式；不使用真正憑證。
    $oldPythonMarker = $env:LM_PYTHON_SMOKE_SECRET
    try {
        $env:LM_PYTHON_SMOKE_SECRET = 'synthetic-test-only'
        & $pythonProbe @pythonArguments
        if ($LASTEXITCODE -ne 0) { throw 'Python analysis/isolation integration failed.' }
    } finally { $env:LM_PYTHON_SMOKE_SECRET = $oldPythonMarker }
    if (-not $ValidateOnly) {
        Copy-Item -LiteralPath (Join-Path $pythonTestRoot 'python-verification.json') -Destination (Join-Path $projectRoot 'offline\python-verification.json') -Force
    }
    if ($TestOffice) {
        & cargo build --example office_smoke --frozen
        if ($LASTEXITCODE -ne 0) { throw 'Office harness build failed.' }
        $officeProbe = Join-Path $projectRoot 'target\x86_64-pc-windows-msvc\debug\examples\office_smoke.exe'
        # 獨立程序確保 PowerShell COM 包裝物件一併釋放；只 Quit 不保證實例退出，
        # 否則後續圖片測試會被正確的「不接管既有 PowerPoint」保護擋下。
        $powerShellExe = (Get-Process -Id $PID).Path
        & $powerShellExe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'Test-Office.ps1') -Exe $exe -Probe $officeProbe
        if ($LASTEXITCODE -ne 0) { throw 'Isolated native Office verification failed.' }
        & cargo build --example office_images --frozen
        if ($LASTEXITCODE -ne 0) { throw 'Office image harness build failed.' }
        $imageProbe = Join-Path $projectRoot 'target\x86_64-pc-windows-msvc\debug\examples\office_images.exe'
        & $imageProbe $exe (Join-Path $projectRoot ('.build\office-images-' + [guid]::NewGuid().ToString('N')))
        if ($LASTEXITCODE -ne 0) { throw 'Office PNG embed/save/reopen verification failed.' }
    }
    if ($NetworkTestRoot) {
        & (Join-Path $PSScriptRoot 'Test-Network.ps1') -Exe $exe -TestRoot $NetworkTestRoot
        if (-not $ValidateOnly) {
            Copy-Item -LiteralPath (Join-Path $projectRoot '.build\network-verification.json') -Destination (Join-Path $projectRoot 'offline\network-verification.json') -Force
        }
    }
    # 明確選用才啟動本機已安裝的 Viewer；一般自檢不會接觸 VNC。
    if ($TestVnc) {
        & cargo build --example vnc_smoke --frozen
        if ($LASTEXITCODE -ne 0) { throw 'VNC integration harness build failed.' }
        $probeExe = Join-Path $projectRoot 'target\x86_64-pc-windows-msvc\debug\examples\vnc_smoke.exe'
        $caseReports = @()
        $oldHold = $env:LM_VNC_SMOKE_HOLD_SECONDS
        $oldReport = $env:LM_VNC_SMOKE_REPORT
        try {
            $env:LM_VNC_SMOKE_HOLD_SECONDS = '2'
            foreach ($mode in @('default', 'fullscreen-viewonly')) {
                $env:LM_VNC_SMOKE_REPORT = Join-Path $probeRoot ("vnc-$mode.json")
                if ($mode -eq 'default') { & $probeExe }
                else { & $probeExe --fullscreen --viewonly --no-autoscaling }
                if ($LASTEXITCODE -ne 0) { throw "Real UltraVNC integration test failed: $mode" }
                $caseReports += Get-Content -LiteralPath $env:LM_VNC_SMOKE_REPORT -Raw | ConvertFrom-Json
            }
        } finally {
            $env:LM_VNC_SMOKE_HOLD_SECONDS = $oldHold
            $env:LM_VNC_SMOKE_REPORT = $oldReport
        }
        $viewerPath = $caseReports[0].viewer
        $vncReport = [ordered]@{
            recorded_at = (Get-Date -Format o)
            result = 'PASS'
            app_version = $caseReports[0].app_version
            exe_sha256 = (Get-FileHash $exe -Algorithm SHA256).Hash.ToLowerInvariant()
            viewer_version = (Get-Item -LiteralPath $viewerPath).VersionInfo.FileVersion
            viewer_sha256 = (Get-FileHash -LiteralPath $viewerPath -Algorithm SHA256).Hash.ToLowerInvariant()
            cases = $caseReports
            screenshots_verified = $false
            company_machines_tested = $false
        }
        $vncReportPath = if ($ValidateOnly) { Join-Path $probeRoot 'vnc-verification.json' } else { Join-Path $projectRoot 'offline\vnc-verification.json' }
        $vncReport | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $vncReportPath -Encoding UTF8
    }
    # 僅驗證尚未發行的修改：保留 target 建置結果，不改 dist、offline 驗證記錄或簽署清單。
    if ($ValidateOnly) {
        Write-Host 'Source validation passed; release artifacts and manifests were not updated.'
        return
    }
    if ($TestOffice) {
        Copy-Item -LiteralPath (Join-Path $projectRoot '.build\office-verification.json') -Destination (Join-Path $projectRoot 'offline\office-verification.json') -Force
    }
    # 記錄實際版本與 DLL 依賴，之後可與公司的環境直接比較。
    $dependencies = & $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER /dump /dependents $exe
    if ($LASTEXITCODE -ne 0) { throw 'DLL inspection failed.' }
    $report = @(
        "Recorded: $(Get-Date -Format o)"
        "Host OS: $([Environment]::OSVersion.VersionString)"
        "Visual Studio: $VisualStudioPath"
        "MSVC: $env:VCToolsVersion"
        "Compiler: $compiler"
        $compilerReport
        "Windows SDK: $env:WindowsSDKVersion"
        "Target: x86_64-pc-windows-msvc"
        "CRT: static"
        "Rust binaries: $toolchainBin"
        "Cargo home: $env:CARGO_HOME"
        "EXE SHA256: $((Get-FileHash $exe -Algorithm SHA256).Hash)"
        ''
        (& rustc -vV)
        (& cargo -V)
        ''
        $dependencies
    )
    New-Item -ItemType Directory -Path (Join-Path $projectRoot 'offline') -Force | Out-Null
    $report | Set-Content -LiteralPath (Join-Path $projectRoot 'offline\environment.txt') -Encoding UTF8
    # 編譯更新 EXE；完整離線 ZIP 由 Prepare-Delivery.ps1 負責建立及驗證。
    $dist = Join-Path $projectRoot 'dist'
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    Copy-Item -LiteralPath $exe -Destination (Join-Path $dist 'LM_AI.exe') -Force
    Copy-Item -LiteralPath $composerReport -Destination (Join-Path $projectRoot 'offline\composer-verification.json') -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot '.build\project-smoke\log-verification.json') -Destination (Join-Path $projectRoot 'offline\log-verification.json') -Force
    # 0.8.1 起僅交付 LM_AI.exe；移除已停用的相容檔名，不再產生第二份主程式。
    $legacyExe = Join-Path $dist 'CompanyAI.exe'
    if (Test-Path -LiteralPath $legacyExe) { Remove-Item -LiteralPath $legacyExe }
    # 本版 Python 需要完整 NSIS；不發布可能漏掉 runtime 的獨立 EXE 更新清單。
    # 本版已授權完整 NSIS 發行；未選用安裝包的建置不更新任何簽署清單。
    if ($IncludeInstaller) {
        & (Join-Path $PSScriptRoot 'Build-Installer.ps1')
        & (Join-Path $PSScriptRoot 'Test-Installer.ps1')
        Copy-Item -LiteralPath (Join-Path $dist 'update-manifest.json') -Destination (Join-Path $dist 'update-manifest-exe.json') -Force
    }
    [ordered]@{
        recorded_at = (Get-Date -Format o)
        version = (Get-Item $exe).VersionInfo.FileVersion
        result = 'PASS'
        msvc = $env:VCToolsVersion
        compiler_report = $compilerReport
        empty_cargo_cache = [bool]$EmptyCargoCache
        image_read_checks = @('same owner/project/run/conversation parent binding for images and native delegation','5 MB image accepted and 5 MB plus one byte rejected','20-request budget, fast-model prohibition and definite-rejection cleanup','ordinary file-list discovery and read_file metadata without upload','list-only task does not submit images','native Outlook/image quick actions and identity binding','JPEG/PNG exact bytes in text+image_url JSON through existing agent API','same-model tool-free child, text-only parent result and source hash','image cache and unknown request resume without repost','out-of-project and oversize images rejected without image POST')
        chart_transform_checks = @('locked Excel CSV keeps A/Index and D/Mean source roles while deriving Index 1','physical X/Y offset and numbering with horizontal-bar roles','all-series missing-row removal before shared numbering, zero preserved','native HTTP transform_chart and updated chart UI event','DPAPI persistence and original source preservation','PNG cache invalidation when transform changes','WebView2 editor validation, transformed plotting and PNG export','three five-point missing-value examples with unchanged choice submission')
        excel_chart_checks = @('locked source/header/time/X/Y plans, CSV schema and role enforcement','local half-open clock/elapsed filters with explicit scan limits','three actual Excel files with different column orders x five windows, fifteen exact charts','native chart style DPAPI persistence, original data and reset','six chart kinds, user axis ranges, reference lines and colors','native customized PNG callback and verified manual _AI_Output write')
        cargo_home = $env:CARGO_HOME
        checks = @('weekly wizard prepare/confirm/cancel and conversation binding','online inbox COM fixture','two ordinary repairs plus two compact-context recovery attempts','v142 x64 compiler probe','fmt','Clippy','workspace tests','cargo build --release --frozen','WebView2 DOM self-check','native composer persistence, cancellation, cross-conversation continuation and restart recovery','AppContainer OS file/network isolation','project copy/edit/publish integration','HTTP skills/tool loops with bounded JSON repair and activity history','DPAPI project memory, versioned notes and reserved .lmai boundary','persistent PDF cache, source/profile/hash invalidation','built-in skills, bounded cross-file search and chart dedup','fast model delegation, multi-section summaries, encrypted cache and pause without repost','offline ECharts canvas and data table','desktop-agent-v1 native HTTP roles, capability checks, cancellation and parent/child request recovery','large Excel headers, selected columns, row paging and native chart values','30 approximately 10 MiB LOG files, time and station filters, pagination and source revision checks','in-flight user instructions, stale tool and finish suppression, persistent original instructions','Outlook consent rejection with no mailbox access, fake-source folder/header/body paging and dedup','file lock close/retry and encrypted resume without repost','chart anomaly defaults and per-cell evidence table','24-hour policy with controlled deadlines, automatic multi-batch continuation and checkpoint interruption recovery','bounded context, preserved user requirements, encrypted operation archive and on-demand old results','local LOG/Excel CSV datasets with bounded previews, native charting and source provenance','explicit context handoff and correction compaction without losing user instructions or archived evidence','Outlook folder picker, DPAPI policy and ancestor enforcement, native forged/stale selection rejection','local mail coverage with 1000-message limit, no raw-body tool output, independent 50-message AI limit, branches and gaps')
        exe_sha256 = (Get-FileHash $exe -Algorithm SHA256).Hash.ToLowerInvariant()
        installer_built = [bool]$IncludeInstaller
        real_vnc_tested = [bool]$TestVnc
        real_office_tested = [bool]$TestOffice
        real_loopback_smb_tested = [bool]$NetworkTestRoot
        offline_zip_built = $false
    } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $projectRoot 'offline\exe-verification.json') -Encoding UTF8
    Write-Host "Ready: $dist\LM_AI.exe"
} finally { Pop-Location }
