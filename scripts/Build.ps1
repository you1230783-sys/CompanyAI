[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$probeRoot = Split-Path $PSScriptRoot -Parent
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$probeVs = & $vswhere -all -products '*' -property installationPath | Where-Object { Test-Path (Join-Path $_ 'VC\Tools\MSVC\14.29.*\bin\Hostx64\x64\cl.exe') } | Select-Object -First 1
if (-not $probeVs) { throw 'MSVC v142 x64 is required.' }
$envLines = & $env:ComSpec /d /c ('call "{0}" "{1}"' -f (Join-Path $PSScriptRoot 'vs-env.cmd'),$probeVs)
if ($LASTEXITCODE -ne 0) { throw 'vcvars64 initialization failed.' }
foreach ($line in $envLines) {
    if ($line -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($matches[1],$matches[2],'Process') }
}
if ($env:VCToolsVersion -notlike '14.2*') { throw 'Expected MSVC v142.' }
$compiler = Join-Path $env:VCToolsInstallDir 'bin\Hostx64\x64\cl.exe'
$linker = Join-Path $env:VCToolsInstallDir 'bin\Hostx64\x64\link.exe'
$nsis = Join-Path $probeRoot '.tools\nsis-3.12\makensis.exe'
if (-not (Test-Path -LiteralPath $nsis)) { throw 'Extract the official NSIS 3.12 ZIP into .tools first.' }
New-Item -ItemType Directory -Path (Join-Path $probeRoot 'build'),(Join-Path $probeRoot 'dist') -Force | Out-Null
Push-Location $probeRoot
try {
    & rc.exe /nologo /c65001 /fo build\app.res src\app.rc
    if ($LASTEXITCODE -ne 0) { throw 'Resource compilation failed.' }
    # cl 本身的編譯期檢查保證 _MSC_VER 1920-1929 及 _M_X64，不能只看環境變數。
    $compilerOutput = & $compiler /nologo /std:c++17 /utf-8 /W4 /WX /O2 /MT /EHsc /DUNICODE /D_UNICODE /c /Fobuild\main.obj src\main.cpp
    $compilerOutput | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -ne 0) { throw 'v142 compilation failed.' }
    & $linker /nologo /MACHINE:X64 /SUBSYSTEM:WINDOWS /DYNAMICBASE /NXCOMPAT /MANIFEST:EMBED '/MANIFESTUAC:level=''asInvoker'' uiAccess=''false''' /OUT:dist\LARGAN_NSIS_Probe.exe build\main.obj build\app.res user32.lib shell32.lib
    if ($LASTEXITCODE -ne 0) { throw 'Link failed.' }
    $check = New-Object Diagnostics.Process
    $check.StartInfo.FileName = Join-Path $probeRoot 'dist\LARGAN_NSIS_Probe.exe'
    $check.StartInfo.Arguments = '--self-check'
    $check.StartInfo.UseShellExecute = $false
    $check.StartInfo.CreateNoWindow = $true
    $check.StartInfo.WindowStyle = 'Hidden'
    $check.StartInfo.RedirectStandardError = $true
    if (-not $check.Start()) { throw 'Cannot start native UI self-check.' }
    if (-not $check.WaitForExit(15000)) {
        $check.Kill()
        throw 'Native UI self-check timed out; rerun in a normal Windows desktop session.'
    }
    $check.StandardError.ReadToEnd() | Set-Content -LiteralPath build\self-check.log -Encoding UTF8
    if ($check.ExitCode -ne 0) { throw "Native UI self-check failed: $($check.ExitCode)" }
    $check.Dispose()
    & $nsis /NOCONFIG /V3 installer\probe.nsi
    if ($LASTEXITCODE -ne 0) { throw 'NSIS packaging failed.' }
    $imports = & $linker /dump /imports dist\LARGAN_NSIS_Probe.exe
    if ($LASTEXITCODE -ne 0) { throw 'Import inspection failed.' }
    $imports | Set-Content -LiteralPath build\imports.txt -Encoding UTF8
    if ($imports -match 'CreateProcess|ShellExecute|WinExec|system\b|popen') { throw 'Unexpected process-launch import in the native tool.' }
    @("Recorded: $(Get-Date -Format o)","MSVC: $env:VCToolsVersion","Compiler: $compiler",$compilerOutput,"NSIS: $(& $nsis /VERSION)",'CRT: static; target: x64','Native UI self-check: PASS','No process-launch imports in native tool: PASS') | Set-Content -LiteralPath build\build-report.txt -Encoding UTF8
    Get-ChildItem dist\*.exe | ForEach-Object { '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(),$_.Name } | Set-Content -LiteralPath dist\SHA256SUMS.txt -Encoding ASCII
    Write-Host 'Build and packaging: PASS'
} finally { Pop-Location }
