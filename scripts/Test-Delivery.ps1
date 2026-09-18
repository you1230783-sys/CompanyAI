[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$probeRoot = Split-Path $PSScriptRoot -Parent
$localRoot = [IO.Path]::GetFullPath($env:LOCALAPPDATA)
$installRoot = [IO.Path]::GetFullPath((Join-Path $localRoot 'Programs\LARGAN_NSIS_Probe'))
$startMenu = Join-Path ([Environment]::GetFolderPath('Programs')) 'LARGAN NSIS Probe'
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN_NSIS_Probe'
# 不覆蓋既有測試安裝；清理只交給本次生成、固定路徑的解除安裝程式。
if (-not $installRoot.StartsWith($localRoot + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe installation path.' }
if ((Test-Path -LiteralPath $installRoot) -or (Test-Path -LiteralPath $startMenu) -or (Test-Path -LiteralPath $key)) { throw 'A previous probe installation exists. Uninstall it manually before testing.' }
function Run-Checked([string]$File, [string]$Arguments) {
    $process = New-Object Diagnostics.Process
    $process.StartInfo.FileName = $File
    $process.StartInfo.Arguments = $Arguments
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.CreateNoWindow = $true
    $process.StartInfo.WindowStyle = 'Hidden'
    if (-not $process.Start()) { throw "Cannot start: $File" }
    if (-not $process.WaitForExit(60000)) { $process.Kill(); throw "Process did not finish within 60 seconds: $File" }
    if ($process.ExitCode -ne 0) { throw "Process failed ($($process.ExitCode)): $File" }
    $process.Dispose()
}
$installed = $false
try {
    Run-Checked (Join-Path $probeRoot 'dist\LARGAN_NSIS_Probe.exe') '--self-check'
    Run-Checked (Join-Path $probeRoot 'dist\LARGAN_NSIS_Probe_Setup.exe') '/S'
    $installed = $true
    $app = Join-Path $installRoot 'LARGAN_NSIS_Probe.exe'
    $uninstaller = Join-Path $installRoot 'Uninstall.exe'
    foreach ($path in @($app,$uninstaller,(Join-Path $startMenu '安裝測試小工具.lnk'),(Join-Path $startMenu '解除安裝.lnk'))) {
        if (-not (Test-Path -LiteralPath $path)) { throw "Missing installed artifact: $path" }
    }
    if ((Get-Item -LiteralPath $installRoot).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Unexpected installation link.' }
    if ((Get-FileHash -LiteralPath $app).Hash -ne (Get-FileHash -LiteralPath (Join-Path $probeRoot 'dist\LARGAN_NSIS_Probe.exe')).Hash) { throw 'Installed payload mismatch.' }
    $registration = Get-ItemProperty -LiteralPath $key
    if ($registration.Publisher -ne 'LARGAN' -or $registration.InstallLocation -ne $installRoot) { throw 'Incorrect uninstall registration.' }
    Run-Checked $app '--self-check'
    Run-Checked $uninstaller '/S'
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while ((Test-Path -LiteralPath $installRoot) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
    foreach ($path in @($installRoot,$startMenu,$key)) {
        if (Test-Path -LiteralPath $path) { throw "Uninstall did not remove its artifact: $path" }
    }
    $installed = $false
    [ordered]@{
        recorded_at = (Get-Date -Format o)
        result = 'PASS'
        checks = @('portable native window self-check','NSIS per-user install','installed payload hash','start menu shortcuts','uninstall registration','installed native window self-check','NSIS uninstall and cleanup')
        install_directory = $installRoot
        scope = 'Local development computer; corporate antivirus result is not yet known.'
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $probeRoot 'build\verification.json') -Encoding UTF8
    Write-Host 'Install, installed-app self-check and uninstall: PASS'
} finally {
    if ($installed) { Write-Warning "Test failed; any remaining probe files are retained at $installRoot for diagnosis. LM_AI was not modified." }
}
