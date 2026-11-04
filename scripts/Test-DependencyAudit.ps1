# 每次刷新官方漏洞資料庫，保留資料庫修訂與完整結果，不能把離線舊資料當作最新檢查。
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
. (Join-Path $PSScriptRoot 'Audit-Policy.ps1')
& (Join-Path $PSScriptRoot 'Test-AuditPolicy.ps1')
& (Join-Path $PSScriptRoot 'Install-CargoAudit.ps1')
$auditExe = Join-Path $projectRoot '.build\audit-tools\bin\cargo-audit-x86_64-pc-windows-msvc-v0.22.2\cargo-audit.exe'
$reportRoot = Join-Path $projectRoot '.build\ci\reports'
New-Item -ItemType Directory -Path $reportRoot -Force | Out-Null
$previousCargoHome = $env:CARGO_HOME
$env:CARGO_HOME = Join-Path $projectRoot '.build\audit-cargo-home'
Push-Location $projectRoot
try {
    $reportPath = Join-Path $reportRoot 'dependency-audit.json'
    # 不過濾非 Windows 依賴，不用 --ignore 隱藏已知警告。
    & $auditExe audit --file Cargo.lock --db .build/advisory-db --json --deny unsound --deny yanked > $reportPath
    $auditExit = $LASTEXITCODE
    $report = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
    $decision = Get-DependencyAuditDecision -Report $report -ToolExitCode $auditExit
    [ordered]@{
        recorded_at = (Get-Date -Format o)
        result = $decision.Result
        tool = 'cargo-audit 0.22.2 (official prebuilt tool)'
        lock_sha256 = (Get-FileHash -LiteralPath Cargo.lock -Algorithm SHA256).Hash.ToLowerInvariant()
        database = $report.database
        exit_code = $auditExit
        policy = 'All targets; vulnerabilities, yanked, unsound and unreviewed warnings fail.'
        reviewed_maintenance_notices = $decision.ReviewedNotices
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $reportRoot 'dependency-audit-summary.json') -Encoding utf8
    if (-not $decision.Passed) { throw "Dependency audit failed ($auditExit); inspect dependency-audit.json before changing any policy." }
    Write-Host "PASS dependency audit policy; reviewed maintenance notices: $($decision.ReviewedNotices.Count). See the complete report."
} finally {
    $env:CARGO_HOME = $previousCargoHome
    Pop-Location
}
