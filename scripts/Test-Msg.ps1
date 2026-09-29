# 僅建立未寄出的本機 fixture；不存入信箱、不寄信、不改動 Outlook 安全設定。
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Exe, [Parameter(Mandatory=$true)][string]$Probe)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$fixtureRoot = Join-Path $projectRoot ('.build\msg-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixtureRoot | Out-Null
# 先確認已有正常視窗，不讓測試自動跑進首次帳號設定；原生讀取同樣只取 active object。
if (-not (Get-Process OUTLOOK -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -and $_.MainWindowTitle -notmatch '歡迎|Welcome' })) {
  throw 'Open a configured Classic Outlook before the MSG test.'
}
$app = New-Object -ComObject Outlook.Application
$mail = $null
try {
  $mail = $app.CreateItem(0)
  $mail.Subject = 'CompanyAI MSG fixture'
  $mail.Body = "測試正文 W40`r`n第二行文字"
  $mail.SaveAs((Join-Path $fixtureRoot 'fixture.msg'), 9)
} finally {
  if ($null -ne $mail) { $mail.Close(1); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($mail) }
  [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($app)
}
& $Probe $Exe $fixtureRoot
if ($LASTEXITCODE -ne 0) { throw 'MSG broker verification failed.' }
[ordered]@{ result='PASS'; version=(Get-Item $Exe).VersionInfo.FileVersion; fixture=$fixtureRoot; company_encryption_tested=$false; checks=@('Classic Outlook MSG header/body read','TXT output/readback','original unchanged','no mailbox enumeration or send') } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $projectRoot '.build\msg-verification.json') -Encoding UTF8
