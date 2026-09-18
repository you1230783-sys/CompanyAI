# 使用者層級安裝；不修改其他使用者或共用 WebView2 的解除安裝資料。
$ErrorActionPreference = 'Stop'
$plan = Get-Content -LiteralPath $env:LM_AI_PLAN -Raw -Encoding UTF8 | ConvertFrom-Json
$target = [IO.Path]::GetFullPath([string]$plan.target)
$expected = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Programs\LM_AI'))
if ($target -ne $expected) { throw '安裝位置不正確。' }
if ((Test-Path -LiteralPath $target) -and ((Get-Item -LiteralPath $target).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw '安裝目錄不可為連結。' }
$installed = Join-Path $target 'LM_AI.exe'
if (Get-Process LM_AI -ErrorAction SilentlyContinue) { throw '請先從系統托盤退出 LM_AI，再執行安裝。' }
# 依已安裝的 WebView2 登錄資料避免不必要的重裝；啟動後仍由 Loader 驗證可用性。
$runtimePresent = $false
foreach ($key in @('HKCU:\Software\Microsoft\EdgeUpdate\Clients','HKLM:\Software\WOW6432Node\Microsoft\EdgeUpdate\Clients','HKLM:\Software\Microsoft\EdgeUpdate\Clients')) {
    if (Test-Path $key) {
        foreach ($child in Get-ChildItem $key) {
            $entry = Get-ItemProperty $child.PSPath
            if ($entry.name -like '*WebView2*' -and $entry.pv -and $entry.pv -ne '0.0.0.0') { $runtimePresent = $true }
        }
    }
}
if (-not $runtimePresent) {
    $signature = Get-AuthenticodeSignature -LiteralPath $plan.runtime
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') { throw 'WebView2 安裝檔的 Microsoft 簽章不正確。' }
    $runtime = Start-Process -FilePath $plan.runtime -ArgumentList '/silent','/install' -WindowStyle Hidden -Wait -PassThru
    if ($runtime.ExitCode -notin @(0,3010)) { throw "WebView2 安裝失敗：$($runtime.ExitCode)" }
}
New-Item -ItemType Directory -Path $target -Force | Out-Null
$pending = Join-Path $target 'LM_AI.pending.exe'
Copy-Item -LiteralPath $plan.source -Destination $pending -Force
if (Test-Path -LiteralPath $installed) { [IO.File]::Replace($pending,$installed,(Join-Path $target 'LM_AI.previous.exe'),$true) }
else { Move-Item -LiteralPath $pending -Destination $installed }
Copy-Item -LiteralPath $installed -Destination (Join-Path $target 'LM_AI_Uninstall.exe') -Force
$shell = New-Object -ComObject WScript.Shell
foreach ($folder in @([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('Programs'))) {
    $shortcut = $shell.CreateShortcut((Join-Path $folder 'LM_AI.lnk'))
    $shortcut.TargetPath = $installed
    $shortcut.WorkingDirectory = $target
    $shortcut.Description = '公司 AI 助理（Dev: 1230783）'
    $shortcut.Save()
}
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN.LM_AI'
New-Item -Path $key -Force | Out-Null
$values = @{DisplayName='LM_AI'; Publisher='LARGAN'; DisplayVersion=[string]$plan.version; InstallLocation=$target; DisplayIcon=$installed; UninstallString=('"' + (Join-Path $target 'LM_AI_Uninstall.exe') + '" --uninstall')}
foreach ($name in $values.Keys) { Set-ItemProperty -LiteralPath $key -Name $name -Value $values[$name] }
New-ItemProperty -LiteralPath $key -Name NoModify -Value 1 -PropertyType DWord -Force | Out-Null
New-ItemProperty -LiteralPath $key -Name NoRepair -Value 1 -PropertyType DWord -Force | Out-Null
Start-Process -FilePath $installed -WindowStyle Hidden
