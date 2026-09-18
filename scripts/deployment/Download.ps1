# 此腳本內嵌於 EXE。HTTP 上的 SHA256 只檢查完整性；信任來源由有效簽章與既有發行者驗證。
$ErrorActionPreference = 'Stop'
$plan = Get-Content -LiteralPath $env:LM_AI_PLAN -Raw -Encoding UTF8 | ConvertFrom-Json
$current = Get-AuthenticodeSignature -LiteralPath $plan.target
if ($current.Status -ne 'Valid' -or -not $current.SignerCertificate) { throw '目前程式尚未具有效的發行者簽章，無法安全啟用自動更新；請由 IT 安裝已簽章版本。' }
Add-Type -AssemblyName System.Net.Http
$handler = New-Object Net.Http.HttpClientHandler
$handler.AllowAutoRedirect = $false
$client = New-Object Net.Http.HttpClient($handler)
$client.Timeout = [TimeSpan]::FromMinutes(10)
$response = $null
$output = $null
$inputStream = $null
try {
    $response = $client.GetAsync([string]$plan.url, [Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
    if ([int]$response.StatusCode -ne 200) { throw "更新下載失敗：HTTP $([int]$response.StatusCode)" }
    if ($response.Content.Headers.ContentLength -gt 209715200) { throw '更新檔過大。' }
    $inputStream = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
    # ResponseHeadersRead 的 HttpClient.Timeout 不涵蓋後續同步 Read，另設讀取逾時。
    if ($inputStream.CanTimeout) { $inputStream.ReadTimeout = 60000 }
    $output = [IO.File]::Create([string]$plan.source)
    $buffer = New-Object byte[] 65536
    $total = 0L
    while (($count = $inputStream.Read($buffer,0,$buffer.Length)) -gt 0) {
        $total += $count
        if ($total -gt 209715200) { throw '更新檔過大。' }
        $output.Write($buffer,0,$count)
    }
    $output.Dispose(); $output = $null
    if ((Get-FileHash -LiteralPath $plan.source -Algorithm SHA256).Hash -ne $plan.sha256) { throw '更新檔 SHA256 不符。' }
    $next = Get-AuthenticodeSignature -LiteralPath $plan.source
    if ($next.Status -ne 'Valid' -or $next.SignerCertificate.Thumbprint -ne $current.SignerCertificate.Thumbprint) { throw '更新檔簽章或發行者不符。' }
    $info = [Diagnostics.FileVersionInfo]::GetVersionInfo([string]$plan.source)
    if ($info.ProductName -ne 'LM_AI' -or [version]$info.ProductVersion -ne [version]($plan.version + '.0')) { throw '更新檔產品或版本不符。' }
} finally {
    if ($output) { $output.Dispose() }
    if ($inputStream) { $inputStream.Dispose() }
    if ($response) { $response.Dispose() }
    $client.Dispose(); $handler.Dispose()
}
