# 不修改正式常數；用暫存來源驗證佔位、正式格式與發布閘門。
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$work = Join-Path $root ('.build\pin-config-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work -Force | Out-Null
$fixture = Join-Path $work 'config.rs'
$checker = Join-Path $PSScriptRoot 'Get-HttpsPinStatus.ps1'
$cases = [Collections.Generic.List[string]]::new()
function Test-Pair([string]$Name, [string]$Primary, [string]$Backup, [bool]$Required, [string]$Expected) {
    $content = 'pub(crate) const PRIMARY_SPKI: &str = "' + $Primary + '";' + "`n" +
        'pub(crate) const BACKUP_SPKI: &str = "' + $Backup + '";' + "`n"
    [IO.File]::WriteAllText($fixture, $content)
    $result = $null
    $failure = $null
    try { $result = & $checker -SourceFile $fixture -RequireHttpsPins:$Required } catch { $failure = $_ }
    if ($Expected -eq 'error') {
        if (-not $failure) { throw "$Name should have rejected the source." }
    } elseif ($failure -or $result.mode -cne $Expected) { throw "$Name returned an unexpected pin status: $failure" }
    $cases.Add($Name)
}
$a = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA='
$b = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAE='
Test-Pair 'placeholders allowed for local verification' 'LMAI-PIN-PLACEHOLDER-PRIMARY' 'LMAI-PIN-PLACEHOLDER-BACKUP' $false 'placeholder'
Test-Pair 'production gate rejects placeholders' 'LMAI-PIN-PLACEHOLDER-PRIMARY' 'LMAI-PIN-PLACEHOLDER-BACKUP' $true 'error'
Test-Pair 'configured distinct hashes accepted' $a $b $true 'configured'
Test-Pair 'mixed placeholder rejected' $a 'LMAI-PIN-PLACEHOLDER-BACKUP' $false 'error'
Test-Pair 'malformed hash rejected without gate' 'invalid' $b $false 'error'
Test-Pair 'duplicate decoded hashes rejected' $a $a $true 'error'
Test-Pair 'wrong decoded length rejected' 'AAAA' $b $true 'error'
Test-Pair 'whitespace rejected' (' ' + $a) $b $false 'error'
# 腳本預設來源必須也能通過，避免測試只覆蓋合成常數。
$actual = & $checker
[ordered]@{ result = 'PASS'; cases = @($cases); source_https_pins = $actual } |
    ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root '.build\https-pin-configuration.json') -Encoding UTF8
Write-Host "HTTPS pin configuration: $($cases.Count) cases passed; source=$($actual.mode)."
