# 在目前電腦產生 VS Code 專案設定；搬移資料夾後重新執行即可。
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Enter-DevShell.ps1')

# 只傳遞 Rust / MSVC 所需環境，不把所有環境變數（可能含金鑰）寫入設定。
# 使用實際路徑，避免擴充套件對變數替換的支援差異。
$developmentEnvironment = [ordered]@{}
foreach ($name in @('Path','INCLUDE','LIB','LIBPATH','CARGO_HOME','RUSTUP_HOME','RUSTC','RUSTDOC','RUSTUP_AUTO_INSTALL','CARGO_NET_OFFLINE','CARGO_TARGET_DIR','CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER')) {
    $value = [Environment]::GetEnvironmentVariable($name, 'Process')
    if ($value) { $developmentEnvironment[$name] = $value }
}
$settingsDirectory = Join-Path $projectRoot '.vscode'
$settingsPath = Join-Path $settingsDirectory 'settings.json'
$settings = [ordered]@{}
if (Test-Path -LiteralPath $settingsPath) {
    # 保留既有的其他設定；遇到無法解析的 JSONC 時停止，避免覆蓋使用者資料。
    try { $existing = Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json }
    catch { throw 'Existing .vscode/settings.json could not be parsed. Preserve it and merge the Rust settings manually, or move it aside before retrying.' }
    if ($null -eq $existing -or $existing -isnot [pscustomobject]) { throw 'VS Code settings must be a JSON object.' }
    foreach ($property in $existing.PSObject.Properties) { $settings[$property.Name] = $property.Value }
}

# Server 環境負責啟動後的工具搜尋；Cargo 環境同時涵蓋 metadata 與儲存時檢查。
# 合併對應設定，保留使用者另外指定的環境變數。
foreach ($settingName in @('rust-analyzer.server.extraEnv','rust-analyzer.cargo.extraEnv','terminal.integrated.env.windows')) {
    $merged = [ordered]@{}
    if ($settings.Contains($settingName) -and $settings[$settingName]) {
        foreach ($property in $settings[$settingName].PSObject.Properties) { $merged[$property.Name] = $property.Value }
    }
    foreach ($name in $developmentEnvironment.Keys) { $merged[$name] = $developmentEnvironment[$name] }
    $settings[$settingName] = $merged
}
$cargoArguments = @($settings['rust-analyzer.cargo.extraArgs'] | Where-Object { $_ })
if ($cargoArguments -notcontains '--frozen') { $cargoArguments += '--frozen' }
$settings['rust-analyzer.cargo.extraArgs'] = $cargoArguments
New-Item -ItemType Directory -Path $settingsDirectory -Force | Out-Null
# 設定只屬於這台電腦，不進 Git，也不放入離線 ZIP。
$settings | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $settingsPath -Encoding UTF8
Write-Host "Configured: $settingsPath"
Write-Host 'Open this project folder in VS Code, then run Developer: Reload Window. Recreate existing terminals.'
