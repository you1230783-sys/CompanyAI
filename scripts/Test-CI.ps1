# Hosted CI 的範圍明確小於公司／Office／NSIS 完整發行驗證。
[CmdletBinding()]
param([switch]$HostedRunner)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if ($HostedRunner -and $env:GITHUB_ACTIONS -ne 'true') { throw 'Hosted exclusions are only allowed on GitHub Actions.' }
$reportRoot = Join-Path $projectRoot '.build\ci\reports'
New-Item -ItemType Directory -Path $reportRoot -Force | Out-Null
$skipped = @()
if ($HostedRunner) {
    # Explorer 通知需要互動桌面；本機完整 Build.ps1 不排除這兩項。
    $skipped = @('ui::reminders::tests::windows_shell_accepts_native_reminder_notification', 'pet::tests::native_notification_visibility_and_settings_survive_restart')
}
Push-Location $projectRoot
try {
    # BuildOnly 不部署或封裝，但沿用正式 v142 x64 探針與空 Cargo 快取 frozen 建置。
    & (Join-Path $PSScriptRoot 'Build.ps1') -BuildOnly -EmptyCargoCache
    & cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'CI formatting failed.' }
    $testArguments = @('--')
    foreach ($test in $skipped) { $testArguments += @('--skip', $test) }
    foreach ($configuration in @('default', 'core')) {
        $features = @()
        if ($configuration -eq 'core') { $features = @('--no-default-features') }
        & cargo clippy --workspace --all-targets --frozen @features -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw "CI Clippy failed: $configuration" }
        & cargo test --workspace --frozen @features @testArguments
        if ($LASTEXITCODE -ne 0) { throw "CI Rust tests failed: $configuration" }
    }
    $build = Get-Content -LiteralPath .build/build-only.json -Raw | ConvertFrom-Json
    [ordered]@{
        recorded_at = (Get-Date -Format o)
        result = 'PASS_CI_SCOPE'
        msvc = $build.msvc
        compiler_report = $build.compiler_report
        empty_cargo_cache = $build.empty_cargo_cache
        lock_sha256 = (Get-FileHash -LiteralPath Cargo.lock -Algorithm SHA256).Hash.ToLowerInvariant()
        checks = @('v142 x64 release build --frozen', 'fmt', 'default/core all-targets Clippy -D warnings', 'default/core workspace Rust tests')
        excluded_interactive_tests = $skipped
        not_validated = @('Office integration', 'company models/mailbox/network', 'NSIS lifecycle', 'interactive desktop/DPI')
        installer_built = $false
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $reportRoot 'windows-ci.json') -Encoding utf8
} finally { Pop-Location }
