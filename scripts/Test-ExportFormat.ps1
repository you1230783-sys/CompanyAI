# 對 document_work_smoke 本次產生的文件做真正Word格式讀回，不僅檢查ZIP標籤。
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Report)
$ErrorActionPreference = 'Stop'
# 前一步自己的COM程序可能稍晚退出；只等待，不接管或終止現有Office。
for ($attempt = 0; $attempt -lt 100 -and (Get-Process WINWORD,EXCEL,POWERPNT -ErrorAction SilentlyContinue); $attempt++) {
    Start-Sleep -Milliseconds 100
}
if (Get-Process WINWORD,EXCEL,POWERPNT -ErrorAction SilentlyContinue) { throw 'Close Office before export format verification.' }
$evidence = Get-Content -LiteralPath $Report -Raw | ConvertFrom-Json
# 跟隨套件版本，避免每次進版忘記更新測試；仍拒絕重用舊版證據。
$manifest = Get-Content -LiteralPath (Join-Path (Split-Path $PSScriptRoot -Parent) 'Cargo.toml') -Raw
$versionMatch = [regex]::Match($manifest, '(?m)^version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success -or $evidence.result -ne 'PASS' -or $evidence.version -ne $versionMatch.Groups[1].Value) { throw 'Expected current document fixture report.' }
$files = @(Get-ChildItem -LiteralPath (Join-Path $evidence.fixture '_AI_Output') -Recurse -File -Filter 'export.docx')
if ($files.Count -ne 1) { throw 'Expected one exported Word fixture.' }
$word = New-Object -ComObject Word.Application
$doc = $null
try {
    $doc = $word.Documents.Open($files[0].FullName, $false, $true, $false)
    $text = $doc.Content.Text
    function Find-Range([string]$fragment) {
        $offset = $text.IndexOf($fragment, [StringComparison]::Ordinal)
        if ($offset -lt 0) { throw "Missing exported text: $fragment" }
        return $doc.Range($offset, $offset + $fragment.Length)
    }
    if ((Find-Range '粗體😀').Font.Bold -ne -1) { throw 'Bold/UTF16 range mismatch.' }
    if ((Find-Range '斜體').Font.Italic -ne -1) { throw 'Italic range mismatch.' }
    if ((Find-Range 'inline_code').Font.Name -ne 'Consolas') { throw 'Inline code font missing.' }
    if ($text.Contains('**粗體') -or $text.Contains('```')) { throw 'Markdown delimiters leaked into formatted output.' }
    if (-not $text.Contains("    print(42)`r`r    # 保留縮排與空行")) { throw 'Code block whitespace changed.' }
    if ((Find-Range 'print(42)').Shading.BackgroundPatternColor -ne 16184563) { throw 'Code background missing.' }
    $normal = Find-Range '程式碼後的一般文字。'
    if ($normal.Font.Name -eq 'Consolas' -or $normal.Font.Bold -ne 0 -or $normal.Shading.BackgroundPatternColor -eq 16184563) { throw 'Code formatting leaked into following prose.' }
    $boldCell = $doc.Tables.Item(1).Cell(2,1).Range
    $italicCell = $doc.Tables.Item(1).Cell(2,2).Range
    # 排除Word儲存格結尾的段落／cell標記；核對實際文字的直接格式。
    $boldCell.End -= 2
    $italicCell.End -= 2
    if ($boldCell.Font.Bold -ne -1 -or $italicCell.Font.Italic -ne -1) { throw 'Markdown table cell formatting missing.' }
    [ordered]@{result='PASS';version=$evidence.version;checks=@('Word reopen','bold and emoji UTF16 offsets','italic and inline monospace','fenced code indentation and blank line','code background','following paragraph formatting reset','table inline formatting');fixture=$files[0].FullName} | ConvertTo-Json -Depth 3 | Set-Content (Join-Path (Split-Path $Report -Parent) 'export-format-verification.json') -Encoding UTF8
    Write-Output 'PASS Word Markdown export formatting and whitespace readback.'
} finally {
    if ($doc) { $doc.Close(0); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($doc) }
    $word.Quit()
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($word)
}
