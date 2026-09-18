# 僅供開發者發行更新；此檔不會安裝到使用者電腦，也不由應用程式執行。
[CmdletBinding()]
param([switch]$Initialize, [ValidateSet('exe','nsis')][string]$Kind = 'exe', [string]$Url)
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
    $fileName = if ($Kind -eq 'exe') { 'LM_AI.exe' } else { 'LM_AI_Setup.exe' }
    $manifestName = if ($Kind -eq 'exe') { 'update-manifest-exe.json' } else { 'update-manifest.json' }
    if (-not $Url) { $Url = '/lm_server/desktop/releases/' + $fileName }
    $setup = Join-Path $root ('dist\' + $fileName)
    # 防止只編譯 EXE 後，誤把仍是舊版的安裝包標記成新版本。
    if ((Get-Item $setup).VersionInfo.FileVersion -ne "$version.0") { throw 'Artifact version differs from Cargo.toml; build the selected artifact first.' }
    $hash = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLowerInvariant()
    $size = (Get-Item $setup).Length
    $message = "LM_AI_UPDATE_V1`n$version`nwindows-x86_64`n$Kind`n$size`n$hash`n"
    $signature = $rsa.SignData([Text.Encoding]::UTF8.GetBytes($message), 'SHA256')
    $manifest = [ordered]@{schema_version=1;version=$version;platform='windows-x86_64';kind=$Kind;url=$Url;size=$size;sha256=$hash;signature=([BitConverter]::ToString($signature).Replace('-','').ToLowerInvariant())}
    $json = $manifest | ConvertTo-Json
    $manifestPath = Join-Path $root ('dist\' + $manifestName)
    [IO.File]::WriteAllText($manifestPath, $json, (New-Object Text.UTF8Encoding $false))
    # 由主程式內建公鑰與正式驗證程式核對，不只相信發行脚本簽署成功。
    $verify = New-Object Diagnostics.Process
    $verify.StartInfo.FileName = Join-Path $root 'dist\LM_AI.exe'
    $verify.StartInfo.Arguments = '--verify-update-package "' + $manifestPath + '" "' + $setup + '"'
    $verify.StartInfo.UseShellExecute = $false
    $verify.StartInfo.CreateNoWindow = $true
    try {
        if (-not $verify.Start()) { throw 'Cannot start native release verifier.' }
        if (-not $verify.WaitForExit(60000)) { $verify.Kill(); throw 'Native release verification timed out.' }
        if ($verify.ExitCode -ne 0) { throw 'Native updater rejected the signed manifest or artifact.' }
    } finally { $verify.Dispose() }
    Write-Host "Ready: dist/$manifestName (publish together with this exact $fileName)."
} finally {
    if ($secret) { [Array]::Clear($secret,0,$secret.Length) }
    $rsa.Dispose()
}
