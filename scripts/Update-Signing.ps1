# 僅供開發者發行更新；此檔不會安裝到使用者電腦，也不由應用程式執行。
[CmdletBinding()]
param([switch]$Initialize, [string]$Url = '/lm_server/desktop/releases/LM_AI_Setup.exe')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$privatePath = Join-Path $root '.private\update-key.dpapi'
$publicPath = Join-Path $root 'assets\update-public-key.blob'
Add-Type -AssemblyName System.Security
$rsa = New-Object Security.Cryptography.RSACryptoServiceProvider 3072
$rsa.PersistKeyInCsp = $false
try {
    if ($Initialize) {
        if ((Test-Path $privatePath) -or (Test-Path $publicPath)) { throw 'Key already exists; rotation requires a planned client migration.' }
        New-Item -ItemType Directory -Path (Split-Path $privatePath) -Force | Out-Null
        $secret = [Text.Encoding]::UTF8.GetBytes($rsa.ToXmlString($true))
        $encrypted = [Security.Cryptography.ProtectedData]::Protect($secret, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
        [IO.File]::WriteAllBytes($privatePath, $encrypted)
        $p = $rsa.ExportParameters($false)
        $blob = New-Object 'System.Collections.Generic.List[byte]'
        foreach ($n in @(0x31415352,3072,$p.Exponent.Length,$p.Modulus.Length,0,0)) { $blob.AddRange([BitConverter]::GetBytes([uint32]$n)) }
        $blob.AddRange($p.Exponent); $blob.AddRange($p.Modulus)
        [IO.File]::WriteAllBytes($publicPath, $blob.ToArray())
        # 固定測試向量只簽非更新格式的訊息，不能當成合法安裝包使用。
        $fixture = [Text.Encoding]::UTF8.GetBytes('LM_AI signature verification test')
        [IO.File]::WriteAllBytes((Join-Path $root 'assets\update-signature-test.bin'), $rsa.SignData($fixture, 'SHA256'))
        Write-Host 'Update key initialized. Private key is DPAPI encrypted and excluded from Git/delivery.'
        return
    }
    $secret = [Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($privatePath), $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    $rsa.FromXmlString([Text.Encoding]::UTF8.GetString($secret))
    $version = [regex]::Match((Get-Content (Join-Path $root 'Cargo.toml') -Raw), '(?m)^version = "([^"]+)"').Groups[1].Value
    $setup = Join-Path $root 'dist\LM_AI_Setup.exe'
    $hash = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLowerInvariant()
    $size = (Get-Item $setup).Length
    $message = "LM_AI_UPDATE_V1`n$version`nwindows-x86_64`nnsis`n$size`n$hash`n"
    $signature = $rsa.SignData([Text.Encoding]::UTF8.GetBytes($message), 'SHA256')
    $manifest = [ordered]@{schema_version=1;version=$version;platform='windows-x86_64';kind='nsis';url=$Url;size=$size;sha256=$hash;signature=([BitConverter]::ToString($signature).Replace('-','').ToLowerInvariant())}
    $json = $manifest | ConvertTo-Json
    [IO.File]::WriteAllText((Join-Path $root 'dist\update-manifest.json'), $json, (New-Object Text.UTF8Encoding $false))
    Write-Host 'Ready: dist/update-manifest.json (publish together with this exact Setup.exe).'
} finally {
    if ($secret) { [Array]::Clear($secret,0,$secret.Length) }
    $rsa.Dispose()
}
