[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
# 統一載入 v142 與指定 SDK，讓手動執行及未來網頁呼叫使用相同編譯環境。
. (Join-Path $PSScriptRoot 'Enter-DevShell.ps1')
Push-Location $projectRoot
try {
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
    $smokeCheck = Start-Process -FilePath $exe -ArgumentList '--self-check' -Wait -PassThru -WindowStyle Hidden
    if ($smokeCheck.ExitCode -ne 0) { throw 'Executable UI smoke check failed.' }
    # 記錄實際版本與 DLL 依賴，之後可與公司的環境直接比較。
    $dependencies = & $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER /dump /dependents $exe
    if ($LASTEXITCODE -ne 0) { throw 'DLL inspection failed.' }
    $report = @(
        "Recorded: $(Get-Date -Format o)"
        "Host OS: $([Environment]::OSVersion.VersionString)"
        "Visual Studio: $VisualStudioPath"
        "MSVC: $env:VCToolsVersion"
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
    Copy-Item -LiteralPath $exe -Destination (Join-Path $dist 'CompanyAI.exe') -Force
    Write-Host "Ready: $dist\CompanyAI.exe"
} finally { Pop-Location }
