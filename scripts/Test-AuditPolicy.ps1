[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Audit-Policy.ps1')
$fixture = @'
{"database":{"last-commit":"test-revision"},"lockfile":{"dependency-count":125},"settings":{"ignore":[],"target_arch":[],"target_os":[],"severity":null,"informational_warnings":["unmaintained","unsound","notice"]},"vulnerabilities":{"found":false,"count":0},"warnings":{}}
'@
$notice = '{"package":{"name":"ttf-parser","version":"0.25.1"},"advisory":{"id":"RUSTSEC-2026-0192"}}'
$cases = @(
    @{ Name='clean'; Change={}; Exit=0; Date='2026-11-05T00:00:00Z'; Expected=$true },
    @{ Name='tool failure'; Change={}; Exit=2; Date='2026-11-05T00:00:00Z'; Expected=$false },
    @{ Name='known maintenance'; Change={ param($r) $r.warnings | Add-Member unmaintained @($notice | ConvertFrom-Json) }; Exit=0; Date='2026-11-05T00:00:00Z'; Expected=$true },
    @{ Name='expired maintenance'; Change={ param($r) $r.warnings | Add-Member unmaintained @($notice | ConvertFrom-Json) }; Exit=0; Date='2027-02-01T00:00:00Z'; Expected=$false },
    @{ Name='new vulnerability'; Change={ param($r) $r.vulnerabilities.found=$true; $r.vulnerabilities.count=1 }; Exit=0; Date='2026-11-05T00:00:00Z'; Expected=$false },
    @{ Name='different version'; Change={ param($r) $r.warnings | Add-Member unmaintained @(($notice.Replace('0.25.1','0.26.0') | ConvertFrom-Json)) }; Exit=0; Date='2026-11-05T00:00:00Z'; Expected=$false },
    @{ Name='yanked'; Change={ param($r) $r.warnings | Add-Member yanked @($notice | ConvertFrom-Json) }; Exit=0; Date='2026-11-05T00:00:00Z'; Expected=$false },
    @{ Name='unknown notice'; Change={ param($r) $r.warnings | Add-Member unmaintained @(($notice.Replace('0192','9999') | ConvertFrom-Json)) }; Exit=0; Date='2026-11-05T00:00:00Z'; Expected=$false }
)
foreach ($case in $cases) {
    $report = $fixture | ConvertFrom-Json
    & $case.Change $report
    $decision = Get-DependencyAuditDecision -Report $report -ToolExitCode $case.Exit -Now ([DateTimeOffset]$case.Date)
    if ($decision.Passed -ne $case.Expected) { throw "Audit policy regression: $($case.Name)" }
}
$rejected = $false
try { Get-DependencyAuditDecision -Report ([pscustomobject]@{}) -ToolExitCode 0 | Out-Null } catch { $rejected = $true }
if (-not $rejected) { throw 'Incomplete audit data was accepted.' }
$filtered = $fixture | ConvertFrom-Json
$filtered.settings.ignore = @('RUSTSEC-2026-0192')
$rejected = $false
try { Get-DependencyAuditDecision -Report $filtered -ToolExitCode 0 | Out-Null } catch { $rejected = $true }
if (-not $rejected) { throw 'Hidden advisory filtering was accepted.' }
foreach ($filter in @('severity', 'informational_warnings')) {
    $filtered = $fixture | ConvertFrom-Json
    if ($filter -eq 'severity') { $filtered.settings.severity = 'critical' }
    else { $filtered.settings.informational_warnings = @('notice') }
    $rejected = $false
    try { Get-DependencyAuditDecision -Report $filtered -ToolExitCode 0 | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw "Hidden advisory filtering was accepted: $filter" }
}
Write-Host 'PASS audit policy: 12 cases, including expiry, new warnings, tool failure and advisory filtering.'
