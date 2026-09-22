# 僅供開發端製作測試包；使用者端不需要腳本、WinRAR 或壓縮軟體。
# 官方 WinRAR 7.23 繁體中文工具只存於 .build，不隨產品提交或重新散布。
[CmdletBinding()]
param([string]$WinRarDirectory = '')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (-not $WinRarDirectory) { $WinRarDirectory = Join-Path $root '.build\winrar-7.23-tc' }
$rar = Join-Path $WinRarDirectory 'Rar.exe'
$module = Join-Path $WinRarDirectory 'Default.SFX'
# 使用官方標準模組，不修補 PE、不加殼、不加密、不加入執行命令。
$expectedTools = @{
    $rar = 'CC318F27C07557D52C2F17556C962CE2173A752DAA3C423734579DEF59064A71'
    $module = 'E99B77DD5662E1D738FAA55E5F3244D2F0C52BFEE16E9DCCD24835FC8A363C05'
}
foreach ($path in $expectedTools.Keys) {
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $expectedTools[$path]) {
        throw "Unexpected WinRAR 7.23 TC tool: $path"
    }
}
$app = Join-Path $root 'dist\LM_AI.exe'
$compiledApp = Join-Path $root 'target\x86_64-pc-windows-msvc\release\company-ai.exe'
# 先執行 Build.ps1 -EmptyCargoCache -ValidateOnly；只打包與此次建置完全相同的交付 EXE。
if ((Get-FileHash $app).Hash -ne (Get-FileHash $compiledApp).Hash) { throw 'Release EXE differs from the validated build.' }
if ((Get-Item $app).VersionInfo.FileVersion -ne '0.8.8.0') { throw 'Update the SFX text before packaging another version.' }
$work = Join-Path $root ('.build\sfx-build-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$comment = Join-Path $work 'sfx-comment.txt'
$text = [IO.File]::ReadAllText((Join-Path $root 'installer\LM_AI-sfx.txt'))
# WinRAR -scuc 明確以 UTF-16 解讀註解，避免繁中說明受系統語系影響。
[IO.File]::WriteAllText($comment, $text, [Text.Encoding]::Unicode)
$package = Join-Path $work 'LM_AI_SFX_Test.exe'
& $rar a -cfg- -idq -m3 -ep1 -scuc "-sfx$module" "-z$comment" $package $app
if ($LASTEXITCODE -ne 0) { throw 'SFX creation failed.' }
& $rar t -cfg- -idq $package
if ($LASTEXITCODE -ne 0) { throw 'SFX archive integrity test failed.' }
$entries = @(& $rar lb -cfg- $package)
if ($LASTEXITCODE -ne 0 -or $entries.Count -ne 1 -or $entries[0] -cne 'LM_AI.exe') {
    throw 'SFX must contain only LM_AI.exe; no user configuration or additional executable is allowed.'
}
Copy-Item -LiteralPath $package -Destination (Join-Path $root 'dist\LM_AI_SFX_Test.exe') -Force
Write-Host 'Ready: dist/LM_AI_SFX_Test.exe (manual test only; existing NSIS/manifests unchanged).'
