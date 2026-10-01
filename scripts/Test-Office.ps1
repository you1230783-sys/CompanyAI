# 固定驗收文件；不讀取使用者文件、不改變 Office 安全中心設定。
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Exe, [Parameter(Mandatory=$true)][string]$Probe)
$ErrorActionPreference = 'Stop'
if (Get-Process WINWORD,EXCEL,POWERPNT -ErrorAction SilentlyContinue) { throw 'Close Office before running isolated Office fixture checks.' }
$projectRoot = Split-Path $PSScriptRoot -Parent
$fixtureRoot = Join-Path $projectRoot ('.build\office-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixtureRoot -Force | Out-Null
# 使用 Rust 的固定 COM 參數建立測試資料，避免 PowerShell 動態 SaveAs 參數繫結停滯。
& $Probe $Exe $fixtureRoot --create-fixtures
if ($LASTEXITCODE -ne 0) { throw 'Native Office fixture creation failed.' }
Write-Output "Office fixtures: $fixtureRoot"
& $Probe $Exe $fixtureRoot
if ($LASTEXITCODE -ne 0) { throw 'Native Office/broker verification failed.' }
# 獨立重新開啟成果，核對未要求修改的格式及公式；不以只看 ZIP 結構冒充 Office 驗證。
$wordResult = Get-ChildItem -LiteralPath (Join-Path $fixtureRoot '_AI_Output') -Recurse -Filter '修訂_2.docx' | Select-Object -First 1
$excelResult = Get-ChildItem -LiteralPath (Join-Path $fixtureRoot '_AI_Output') -Recurse -Filter '修訂_2.xlsx' | Select-Object -First 1
$pptResult = Get-ChildItem -LiteralPath (Join-Path $fixtureRoot '_AI_Output') -Recurse -Filter '修訂_2.pptx' | Select-Object -First 1
$word = New-Object -ComObject Word.Application
try {
  $doc = $word.Documents.Open($wordResult.FullName, $false, $true, $false)
  if ($doc.Paragraphs.Item(1).Range.Font.Bold -ne -1) { throw 'Word bold format lost.' }
  if ($doc.Paragraphs.Item(2).Range.Text.Trim() -ne '第二段') { throw 'Word untouched paragraph changed.' }
  $doc.Close(0)
} finally { $word.Quit() }
$excel = New-Object -ComObject Excel.Application
try {
  $book = $excel.Workbooks.Open($excelResult.FullName, 0, $true)
  $sheet = $book.Worksheets.Item(1)
  if ($sheet.Range('A1').Value2 -ne '最終文字' -or $sheet.Range('A1').Font.Bold -ne $true) { throw 'Excel text or style mismatch.' }
  if ($sheet.Range('B1').Value2 -ne 24 -or $sheet.Range('C1').Formula -ne '=B1*2') { throw 'Excel number/formula mismatch.' }
  $book.Close($false)
} finally { $excel.Quit() }
$ppt = New-Object -ComObject PowerPoint.Application
try {
  $deck = $ppt.Presentations.Open($pptResult.FullName, -1, 0, 0)
  if ($deck.Slides.Item(1).Shapes.Item(2).TextFrame.TextRange.Text -ne '第二區塊') { throw 'PowerPoint untouched shape changed.' }
  $deck.Close()
} finally { $ppt.Quit() }
[ordered]@{ version=(Get-Item $Exe).VersionInfo.FileVersion; result='PASS'; office_version=(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Office\ClickToRun\Configuration' -ErrorAction SilentlyContinue).VersionToReport; fixture=$fixtureRoot; checks=@('DOC/DOCX/DOCM/XLS/XLSX/XLSM/XLSB/PPT/PPTX/PPTM COM read/edit/save/reopen','source bytes unchanged','readable filename and numbered second revision','duplicate save idempotent','Excel formula protected and literal = text','Excel numeric edit','Word and Excel bold retained','untouched paragraph/shape retained','blank DOCX/XLSX/PPTX creation','Word paragraphs/table/styles and Chinese font','Excel worksheets/ranges/number formats/borders','PPT title/content/two-column layouts and text styles','Office batch commit and failed-batch rollback','Excel chart reads numeric cells from checked snapshot'); company_encryption_tested=$false; screenshots_verified=$false } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $projectRoot '.build\office-verification.json') -Encoding UTF8
Write-Host 'PASS: native Office verification'
