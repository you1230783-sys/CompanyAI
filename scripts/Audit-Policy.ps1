# 純資料判定，與下載／執行工具分開，讓未來公告及例外到期可用固定案例驗證。
function Get-DependencyAuditDecision {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]$Report,
        [Parameter(Mandatory)][int]$ToolExitCode,
        [DateTimeOffset]$Now = [DateTimeOffset]::UtcNow
    )
    if (-not $Report.database.'last-commit' -or $Report.lockfile.'dependency-count' -lt 1 -or
        $null -eq $Report.vulnerabilities.found -or $null -eq $Report.warnings -or
        $null -eq $Report.settings -or @($Report.settings.ignore).Count -ne 0 -or
        @($Report.settings.target_arch).Count -ne 0 -or @($Report.settings.target_os).Count -ne 0 -or
        $null -ne $Report.settings.severity -or
        @('unmaintained', 'unsound', 'notice' | Where-Object { $_ -notin $Report.settings.informational_warnings }).Count -ne 0) {
        throw 'Incomplete audit report or unexpected advisory filtering.'
    }
    $passed = $ToolExitCode -eq 0 -and -not $Report.vulnerabilities.found -and $Report.vulnerabilities.count -eq 0
    $accepted = @()
    foreach ($group in $Report.warnings.PSObject.Properties) {
        foreach ($warning in @($group.Value)) {
            # ID、版本、種類任一改變都須重新審查；新漏洞不能套用維護例外。
            if ($group.Name -eq 'unmaintained' -and $warning.advisory.id -eq 'RUSTSEC-2026-0192' -and
                $warning.package.name -eq 'ttf-parser' -and $warning.package.version -eq '0.25.1' -and
                $Now -lt [DateTimeOffset]'2027-02-01T00:00:00Z') {
                $accepted += 'RUSTSEC-2026-0192: ttf-parser 0.25.1; review before 2027-02-01'
            } else { $passed = $false }
        }
    }
    [pscustomobject]@{
        Passed = $passed
        Result = $(if (-not $passed) { 'FAIL' } elseif ($accepted.Count) { 'PASS_WITH_REVIEWED_MAINTENANCE_NOTICE' } else { 'PASS' })
        ReviewedNotices = $accepted
    }
}
