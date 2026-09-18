$ErrorActionPreference = 'Stop'
try {
$plan = Get-Content -LiteralPath $env:LM_AI_PLAN -Raw -Encoding UTF8 | ConvertFrom-Json
$process = Get-Process -Id ([int]$plan.pid) -ErrorAction SilentlyContinue
if ($process -and -not $process.WaitForExit(60000)) { throw '解除安裝程序尚未退出。' }
$base = [IO.Path]::GetFullPath($env:LOCALAPPDATA)
$target = [IO.Path]::GetFullPath((Join-Path $base 'Programs\LM_AI'))
$data = [IO.Path]::GetFullPath((Join-Path $base 'CompanyAI'))
if (Get-Process LM_AI -ErrorAction SilentlyContinue) { throw '請先從系統托盤退出 LM_AI，再解除安裝。' }
# 刪除前驗證固定絕對路徑；拒絕連結以免跨出指定目錄。
foreach ($path in @($target,$data)) {
    if (-not $path.StartsWith($base + '\',[StringComparison]::OrdinalIgnoreCase)) { throw '解除安裝路徑超出使用者資料目錄。' }
    if ((Test-Path -LiteralPath $path) -and ((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw '解除安裝目錄不可為連結。' }
}
# 程式目錄只移除已知檔案，不刪除使用者自行放入的其他內容。
foreach ($name in @('LM_AI.exe','LM_AI_Uninstall.exe','LM_AI.previous.exe','LM_AI.pending.exe')) {
    $file = Join-Path $target $name
    if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force }
}
foreach ($folder in @([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('Programs'))) {
    $shortcut = Join-Path $folder 'LM_AI.lnk'
    if (Test-Path -LiteralPath $shortcut) { Remove-Item -LiteralPath $shortcut -Force }
}
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN.LM_AI'
if (Test-Path $key) { Remove-Item -LiteralPath $key }
if ($plan.clear_data -and (Test-Path -LiteralPath $data)) {
    if (Get-ChildItem -LiteralPath $data -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint } | Select-Object -First 1) { throw '本機資料包含連結，請手動確認後刪除。' }
    Remove-Item -LiteralPath $data -Recurse -Force
}
elseif (Test-Path -LiteralPath (Join-Path $data 'updates')) {
    $updates = [IO.Path]::GetFullPath((Join-Path $data 'updates'))
    if ($updates.StartsWith($data + '\',[StringComparison]::OrdinalIgnoreCase) -and -not (Get-ChildItem -LiteralPath $updates -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint } | Select-Object -First 1) -and -not ((Get-Item -LiteralPath $updates).Attributes -band [IO.FileAttributes]::ReparsePoint)) { Remove-Item -LiteralPath $updates -Recurse -Force }
}
} catch {
    # 主程序已退出，隱藏的助手必須自行顯示失敗原因，避免解除安裝無聲失敗。
    Add-Type -AssemblyName System.Windows.Forms
    [Windows.Forms.MessageBox]::Show($_.Exception.Message, 'LM_AI 解除安裝未完成') | Out-Null
    exit 1
}
