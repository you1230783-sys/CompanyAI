# 同一份來源依序量測乾淨建置；不更新dist或把BuildOnly宣稱成完整驗證。
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$runRoot = Join-Path $projectRoot ('.build\release-profiles-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $runRoot | Out-Null
$settings = @('CARGO_PROFILE_RELEASE_LTO', 'CARGO_PROFILE_RELEASE_CODEGEN_UNITS', 'CARGO_PROFILE_RELEASE_PANIC')
$previous = @{}
foreach ($name in $settings) { $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$cases = @(
    @{ name = 'baseline'; lto = 'false'; units = 16 },
    @{ name = 'thin-16'; lto = 'thin'; units = 16 },
    @{ name = 'thin-1'; lto = 'thin'; units = 1 }
)
$results = @()
Push-Location $projectRoot
try {
    $sourceCommit = (& git rev-parse HEAD).Trim()
    $sourceDiff = (& git diff -- src ui assets Cargo.toml Cargo.lock build.rs scripts/Build.ps1 | Out-String)
    foreach ($case in $cases) {
        $env:CARGO_PROFILE_RELEASE_LTO = $case.lto
        $env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = [string]$case.units
        $env:CARGO_PROFILE_RELEASE_PANIC = 'unwind'
        $target = Join-Path $runRoot $case.name
        $log = Join-Path $runRoot ($case.name + '.log')
        $timer = [Diagnostics.Stopwatch]::StartNew()
        & (Join-Path $PSScriptRoot 'Build.ps1') -BuildOnly -EmptyCargoCache -BuildTargetDirectory $target *> $log
        $timer.Stop()
        if ($sourceDiff -cne (& git diff -- src ui assets Cargo.toml Cargo.lock build.rs scripts/Build.ps1 | Out-String)) {
            throw 'Measured sources changed during profile comparison.'
        }
        $build = Get-Content -LiteralPath '.build\build-only.json' -Raw | ConvertFrom-Json
        $exe = Get-Item -LiteralPath $build.exe
        $results += [ordered]@{
            name = $case.name; lto = $case.lto; codegen_units = $case.units; panic = 'unwind'
            seconds = [Math]::Round($timer.Elapsed.TotalSeconds, 3)
            exe_bytes = $exe.Length; exe_sha256 = $build.exe_sha256
            msvc = $build.msvc; compiler_report = $build.compiler_report
            linker = $build.linker; empty_cargo_cache = $build.empty_cargo_cache
            tests_run = $false; installer_built = $false
        }
        [ordered]@{
            source_commit = $sourceCommit; source_diff = $sourceDiff
            method = 'One sequential cold target build per candidate; OS filesystem cache not flushed; build timing includes environment/probe/asset checks.'
            results = $results
        } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $runRoot 'measurements.json') -Encoding utf8
        Write-Host "$($case.name): $($exe.Length) bytes, $($timer.Elapsed.TotalSeconds.ToString('F1')) seconds"
    }
    Write-Host "Measurement report: $runRoot\measurements.json"
} finally {
    foreach ($name in $settings) { [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process') }
    Pop-Location
}
