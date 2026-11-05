# 只建立獨立的合成工作簿，驗證排程遇到 Excel 占用時沒有派送或永久停用。
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Exe, [Parameter(Mandatory=$true)][string]$Probe)
$ErrorActionPreference = 'Stop'
# 允許前一個測試的COM實例自行退出；逾時仍停止，絕不關閉使用者文件。
for ($attempt = 0; $attempt -lt 100 -and (Get-Process WINWORD,EXCEL,POWERPNT -ErrorAction SilentlyContinue); $attempt++) {
    Start-Sleep -Milliseconds 100
}
if (Get-Process WINWORD,EXCEL,POWERPNT -ErrorAction SilentlyContinue) { throw 'Close Office before the isolated schedule fixture.' }
$root = Join-Path (Split-Path $PSScriptRoot -Parent) ('.build\schedule-busy-' + [guid]::NewGuid().ToString('N'))
$excel = New-Object -ComObject Excel.Application
$book = $null
try {
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $book = $excel.Workbooks.Add()
    $book.Worksheets.Item(1).Cells.Item(1,1).Value2 = 'schedule-only synthetic data'
    # Office 需要處理訊息以登錄 ROT；測試不改使用者的安全設定或現有文件。
    Start-Sleep -Milliseconds 800
    & $Probe $Exe $root --office-busy
    if ($LASTEXITCODE -ne 0) { throw 'Occupied Office pre-dispatch test failed.' }
    if ($excel.Workbooks.Count -ne 1 -or $book.Worksheets.Item(1).Cells.Item(1,1).Value2 -ne 'schedule-only synthetic data') { throw 'Schedule changed the occupied workbook.' }
    [ordered]@{result='PASS';fixture=$root;checks=@('actual Excel workbook occupied','no tool dispatch or output','original workbook untouched','recurring schedule enabled with next occurrence','skip audit retained')} | ConvertTo-Json -Depth 3 | Set-Content (Join-Path $root 'verification.json') -Encoding UTF8
} finally {
    if ($book) { $book.Close($false); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($book) }
    $excel.Quit()
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
}
