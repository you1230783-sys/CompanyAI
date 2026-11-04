# 在獨立程序觸發真實背景／Win32／WebView2回呼panic，不接觸正式帳號或文件。
[CmdletBinding()]
param([string]$Probe, [switch]$NoBuild)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
if (-not $NoBuild) {
    . (Join-Path $PSScriptRoot 'Enter-DevShell.ps1')
    Push-Location $project
    try {
        & cargo build --example reliability_smoke --frozen
        if ($LASTEXITCODE -ne 0) { throw 'Reliability harness build failed.' }
    } finally { Pop-Location }
}
if (-not $Probe) { $Probe = Join-Path $project 'target\x86_64-pc-windows-msvc\debug\examples\reliability_smoke.exe' }
$reportRoot = Join-Path $project ('.build\reliability-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $reportRoot | Out-Null
$results = @()
foreach ($mode in @('background','callback','com','abort')) {
    $caseRoot = Join-Path $reportRoot $mode
    $stdout = Join-Path $reportRoot "$mode.out.txt"
    $stderr = Join-Path $reportRoot "$mode.err.txt"
    $process = Start-Process -FilePath $Probe -ArgumentList @($mode, ('"' + $caseRoot + '"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    if (-not $process.WaitForExit(90000)) { $process.Kill(); throw "Panic test timed out: $mode" }
    $process.WaitForExit()
    if (($mode -eq 'abort' -and $process.ExitCode -eq 0) -or ($mode -ne 'abort' -and $process.ExitCode -ne 0)) {
        throw "Panic test exit mismatch: $mode ($($process.ExitCode)); see $stderr"
    }
    $logs = @(Get-ChildItem -LiteralPath (Join-Path $caseRoot 'panic-events') -Filter '*.json' -File)
    if ($logs.Count -eq 0) { throw "No durable panic record: $mode" }
    foreach ($log in $logs) {
        $raw = Get-Content -LiteralPath $log.FullName -Raw
        $event = $raw | ConvertFrom-Json
        if ($raw.Contains('fixture-private') -or $raw.Contains($caseRoot) -or -not $event.source_hash) { throw "Invalid/private panic record: $mode" }
    }
    $results += [ordered]@{ mode=$mode; exit_code=$process.ExitCode; records=$logs.Count; result='PASS' }
}
[ordered]@{recorded_at=(Get-Date -Format o); results=$results; scope='native background, WndProc, WebView2, abort persistence; no company account'} |
    ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $reportRoot 'verification.json') -Encoding UTF8
Write-Host "Reliability verification: $reportRoot"
Copy-Item -LiteralPath (Join-Path $reportRoot 'verification.json') -Destination (Join-Path $project '.build\reliability-verification.json') -Force
