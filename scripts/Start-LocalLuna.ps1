# 開發機專用。只啟動 localhost 轉接，不變更正式端點、版本或發行檔。
[CmdletBinding()]
param(
    [string]$CodexPath,
    [int]$Port = 8766,
    [int]$MaxTurns = 30,
    [switch]$OpenApp
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (-not $CodexPath) {
    $CodexPath = Get-Process codex -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty Path -Unique | Select-Object -First 1
}
if (-not $CodexPath -or -not (Test-Path -LiteralPath $CodexPath)) {
    throw '請用 -CodexPath 指定本機官方 codex.exe。'
}
$python = (Get-Command python.exe -ErrorAction Stop).Source
$output = Join-Path $projectRoot ('.build\luna-local-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $output | Out-Null
# Start-Process 將參數串成字串；所有可能含空白的路徑各自加引號。
$arguments = @('-u', ('"{0}"' -f (Join-Path $projectRoot 'tools\luna_local_server.py')),
    '--codex', ('"{0}"' -f $CodexPath), '--output', ('"{0}"' -f $output),
    '--port', $Port, '--max-turns', $MaxTurns)
$server = Start-Process -FilePath $python -ArgumentList $arguments -WorkingDirectory $projectRoot `
    -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $output 'server.log') `
    -RedirectStandardError (Join-Path $output 'server-error.log')
$server.Id | Set-Content -LiteralPath (Join-Path $output 'server.pid')
$connection = Join-Path $output 'connection.json'
for ($i = 0; $i -lt 50 -and -not (Test-Path -LiteralPath $connection); $i++) {
    if ($server.HasExited) { throw "轉接服務未啟動，請查看 $output\server-error.log" }
    Start-Sleep -Milliseconds 100
}
if (-not (Test-Path -LiteralPath $connection)) { throw '轉接服務啟動逾時。' }
Write-Host "本機服務：http://localhost:$Port"
Write-Host "測試紀錄：$output"
if ($OpenApp) {
    $exe = Join-Path $projectRoot 'target\x86_64-pc-windows-msvc\release\company-ai.exe'
    if (-not (Test-Path -LiteralPath $exe)) { throw '請先用 Build.ps1 建立測試 EXE。' }
    $appRoot = Join-Path $output 'app'
    Start-Process -FilePath $exe -ArgumentList @('--local-ai-test', ('"{0}"' -f $connection),
        ('"{0}"' -f $appRoot)) -WorkingDirectory $projectRoot -WindowStyle Normal
}
