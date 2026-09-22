[CmdletBinding()]
param([switch]$IncludeInstaller, [switch]$EmptyCargoCache, [switch]$ValidateOnly, [switch]$TestVnc)
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
    # 0.8.1 起僅交付 LM_AI.exe；移除已停用的相容檔名，不再產生第二份主程式。
    $legacyExe = Join-Path $dist 'CompanyAI.exe'
    if (Test-Path -LiteralPath $legacyExe) { Remove-Item -LiteralPath $legacyExe }
    if (Test-Path (Join-Path $projectRoot '.private\update-key.dpapi')) {
        & (Join-Path $PSScriptRoot 'Update-Signing.ps1') -Kind exe
    } else {
        $oldManifest = Join-Path $dist 'update-manifest-exe.json'
        if (Test-Path -LiteralPath $oldManifest) { Remove-Item -LiteralPath $oldManifest }
        Write-Host 'No private update key: executable rebuilt without a publishable EXE manifest.'
    }
    # 安裝包為明確選用；EXE 發行不重建 NSIS，也不產生离線 ZIP。
    if ($IncludeInstaller) {
        & (Join-Path $PSScriptRoot 'Build-Installer.ps1')
        & (Join-Path $PSScriptRoot 'Test-Installer.ps1')
    }
    [ordered]@{
        recorded_at = (Get-Date -Format o)
        version = (Get-Item $exe).VersionInfo.FileVersion
        result = 'PASS'
        msvc = $env:VCToolsVersion
        compiler_report = $compilerReport
        empty_cargo_cache = [bool]$EmptyCargoCache
        cargo_home = $env:CARGO_HOME
        checks = @('v142 x64 compiler probe','fmt','Clippy','workspace tests','cargo build --release --frozen','WebView2 DOM self-check')
        exe_sha256 = (Get-FileHash $exe -Algorithm SHA256).Hash.ToLowerInvariant()
        installer_built = [bool]$IncludeInstaller
        real_vnc_tested = [bool]$TestVnc
        offline_zip_built = $false
    } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $projectRoot 'offline\exe-verification.json') -Encoding UTF8
    Write-Host "Ready: $dist\LM_AI.exe"
} finally { Pop-Location }
