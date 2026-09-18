# 只替換同目錄中的 LM_AI.exe；失敗時還原舊檔。所有參數均為 JSON 資料。
$ErrorActionPreference = 'Stop'
$plan = Get-Content -LiteralPath $env:LM_AI_PLAN -Raw -Encoding UTF8 | ConvertFrom-Json
$target = [IO.Path]::GetFullPath([string]$plan.target)
$source = [IO.Path]::GetFullPath([string]$plan.source)
$stage = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($env:LM_AI_PLAN))
if ([IO.Path]::GetFileName($target) -notin @('LM_AI.exe','CompanyAI.exe') -or [IO.Path]::GetDirectoryName($source) -ne $stage) { throw '更新路徑不正確。' }
foreach ($file in @($target,$source)) {
    if ((Get-Item -LiteralPath $file).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw '更新檔不可為連結。' }
}
$current = Get-AuthenticodeSignature -LiteralPath $target
$next = Get-AuthenticodeSignature -LiteralPath $source
if ($current.Status -ne 'Valid' -or $next.Status -ne 'Valid' -or $current.SignerCertificate.Thumbprint -ne $next.SignerCertificate.Thumbprint) { throw '套用更新前的簽章驗證失敗。' }
if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $plan.sha256) { throw '更新檔已變動。' }
$nextVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($source)
$oldVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($target)
if ($nextVersion.ProductName -ne 'LM_AI' -or [version]$nextVersion.ProductVersion -ne [version]($plan.version + '.0') -or [version]$nextVersion.ProductVersion -le [version]$oldVersion.ProductVersion) { throw '更新產品或版本不正確。' }
$process = Get-Process -Id ([int]$plan.pid) -ErrorAction SilentlyContinue
if ($process -and -not $process.WaitForExit(60000)) { throw '主程式尚未退出，已保留舊版本。' }
$directory = [IO.Path]::GetDirectoryName($target)
$pending = Join-Path $directory 'LM_AI.pending.exe'
$backup = Join-Path $directory 'LM_AI.previous.exe'
$replaced = $false
try {
    Copy-Item -LiteralPath $source -Destination $pending -Force
    [IO.File]::Replace($pending,$target,$backup,$true)
    $replaced = $true
    if ($plan.restart) { Start-Process -FilePath $target -WindowStyle Hidden -ErrorAction Stop }
} catch {
    # 替換尚未成功時不可拿上次更新留下的備份覆蓋仍完好的主程式。
    if ($replaced -and (Test-Path -LiteralPath $backup)) { Copy-Item -LiteralPath $backup -Destination $target -Force }
    if ($plan.restart) { Start-Process -FilePath $target -WindowStyle Hidden }
    throw
}
$uninstall = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN.LM_AI'
if (Test-Path $uninstall) { Set-ItemProperty -LiteralPath $uninstall -Name DisplayVersion -Value ([string]$plan.version) }
