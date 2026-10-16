[CmdletBinding()]
param(
    [ValidateSet('2008', '2022')][string]$Version = '2022',
    [string]$Executable = (Join-Path $PSScriptRoot '..\dist\LM_AI.exe'),
    [string]$ReportRoot = (Join-Path $PSScriptRoot '..\.build\cad-live')
)
$ErrorActionPreference = 'Stop'
# 明確啟動此腳本即執行固定測試：目前圖面僅讀取／ZOOM，新建及副本才寫入。
# 請先手動開啟選定 CAD，完成試用／授權提示，並保留一份空白文件。
$exe = (Resolve-Path -LiteralPath $Executable).Path
$root = [IO.Path]::GetFullPath($ReportRoot)
New-Item -ItemType Directory -Path $root -Force | Out-Null
$id = [guid]::NewGuid().ToString('N')
$stdout = Join-Path $root "$id.stdout.txt"
$stderr = Join-Path $root "$id.stderr.txt"
$progid = if ($Version -eq '2008') { 'Icad.Application.1' } else { 'GStarCAD.Application.22' }
Write-Host "CAD $Version offline test: $root"
# Start-Process 會重新拼接參數；路徑個別加引號，支援安裝目錄內的空白。
$process = Start-Process -FilePath $exe -ArgumentList @('--cad-offline-test', ('"' + $root + '"'), $progid) `
    -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
Get-Content -LiteralPath $stdout -Encoding UTF8
if ($process.ExitCode -ne 0) {
    Get-Content -LiteralPath $stderr -Encoding UTF8
    throw "CAD offline test failed (exit $($process.ExitCode)); see offline-report.json."
}
Write-Host 'PASS: data checks and test-file cleanup. Visual appearance still requires review.'
