[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Enter-DevShell.ps1')
Push-Location $projectRoot
try {
    # 四個獨立feature共16種組合；全部檢查，避免只驗證全開／全關漏掉交互依賴。
    $names = @('cad', 'pet', 'tarot', 'vnc')
    $checks = @()
    for ($mask = 0; $mask -lt 16; $mask++) {
        $selected = @()
        for ($index = 0; $index -lt $names.Count; $index++) {
            if ($mask -band (1 -shl $index)) { $selected += $names[$index] }
        }
        $arguments = @('clippy', '--workspace', '--all-targets', '--frozen', '--no-default-features')
        if ($selected.Count -gt 0) { $arguments += @('--features', ($selected -join ',')) }
        $arguments += @('--', '-D', 'warnings')
        $log = Join-Path $projectRoot ('.build\feature-matrix-{0:D2}.log' -f $mask)
        & cargo @arguments *> $log
        if ($LASTEXITCODE -ne 0) {
            Get-Content -LiteralPath $log -Tail 50
            throw "Feature combination failed: $($selected -join ',')"
        }
        $checks += [ordered]@{features = @($selected); result = 'PASS'}
        Write-Host "Feature combination $mask/15 PASS: $($selected -join ',')"
    }
    $coreLog = Join-Path $projectRoot '.build\feature-core-tests.log'
    & cargo test --frozen --lib --no-default-features *> $coreLog
    if ($LASTEXITCODE -ne 0) {
        Get-Content -LiteralPath $coreLog -Tail 50
        throw 'Core feature tests failed.'
    }
    $summary = @(Get-Content -LiteralPath $coreLog | Select-String '^test result: ok\.')[-1].Line
    [ordered]@{
        result = 'PASS'; checked_at = [DateTime]::UtcNow.ToString('o')
        toolset = $env:VCToolsVersion; matrix = $checks; core_tests = $summary
        scope = 'All 16 combinations: frozen Clippy all-targets with denied warnings; core Rust tests. Full default build, native smoke and installer are separate.'
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath '.build\feature-matrix-verification.json' -Encoding utf8
} finally { Pop-Location }
