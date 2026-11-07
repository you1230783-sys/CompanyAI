# 建置前驗證來源中的兩個 SPKI SHA-256；只輸出公開雜湊，沒有任何私鑰。
# SourceFile 供合成測試使用，Build.ps1 固定讀取正式 src/config.rs。
[CmdletBinding()]
param(
    [string]$SourceFile = (Join-Path (Split-Path $PSScriptRoot -Parent) 'src\config.rs'),
    [switch]$RequireHttpsPins
)
$ErrorActionPreference = 'Stop'
$source = [IO.File]::ReadAllText($SourceFile)
$values = foreach ($name in @('PRIMARY_SPKI', 'BACKUP_SPKI')) {
    $pattern = '(?m)^pub\(crate\) const ' + $name + ': &str = "([^"\r\n]+)";\r?$'
    $matchesFound = [regex]::Matches($source, $pattern)
    if ($matchesFound.Count -ne 1) { throw "Expected one literal $name in config.rs; inspect the pin declaration before building." }
    $matchesFound[0].Groups[1].Value
}
$placeholders = @($values | Where-Object { $_.StartsWith('LMAI-PIN-PLACEHOLDER-', [StringComparison]::Ordinal) }).Count
if ($placeholders -eq 2) {
    if ($RequireHttpsPins) { throw 'RequireHttpsPins: HTTPS pins are placeholders; production packaging requires both configured hashes.' }
    $mode = 'placeholder'
} else {
    # 混合正式值／佔位值也必須失敗，不可默默停用 v2。規則與 pins::configured 一致。
    $digests = foreach ($value in $values) {
        if ($value -cnotmatch '^[A-Za-z0-9+/]{43}=$') { throw 'HTTPS pins must both be Base64 SHA-256 hashes (32 bytes).' }
        $bytes = [Convert]::FromBase64String($value)
        if ($bytes.Length -ne 32) { throw 'HTTPS pin must decode to exactly 32 bytes.' }
        [Convert]::ToBase64String($bytes)
    }
    if ($digests[0] -ceq $digests[1]) { throw 'Primary and backup HTTPS pins must be different.' }
    $mode = 'configured'
}
[pscustomobject][ordered]@{
    mode = $mode
    primary_spki = $values[0]
    backup_spki = $values[1]
    config_sha256 = (Get-FileHash -LiteralPath $SourceFile -Algorithm SHA256).Hash.ToLowerInvariant()
    required = [bool]$RequireHttpsPins
    # 格式通過不代表已向公司伺服器驗證金鑰所有權或備用切換。
    server_verified = $false
}
