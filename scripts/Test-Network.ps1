# 只用明確指定的本機 loopback 分享測試；不建立分享、不連公司伺服器。
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Exe, [Parameter(Mandatory=$true)][string]$TestRoot)
$ErrorActionPreference = 'Stop'
if ($TestRoot -notmatch '^\\\\localhost\\[^\\]+\\.+') { throw 'Use an existing localhost SMB test subfolder.' }
$projectRoot = Split-Path $PSScriptRoot -Parent
$fixture = Join-Path $TestRoot ('network-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
& cargo build --example network_smoke --frozen
if ($LASTEXITCODE -ne 0) { throw 'Network harness build failed.' }
$probe = Join-Path $projectRoot 'target\x86_64-pc-windows-msvc\debug\examples\network_smoke.exe'
& $probe $Exe $fixture
if ($LASTEXITCODE -ne 0) { throw 'UNC broker/Office test failed.' }
# 選未使用的磁碟代號，測試結束只移除本次建立且仍指向原 fixture 的映射。
$occupied = @((Get-PSDrive -PSProvider FileSystem).Name) + @(Get-SmbMapping | Where-Object { $_.LocalPath } | ForEach-Object { $_.LocalPath.TrimEnd(':') })
$letter = @('R','S','T','U','V','W') | Where-Object { $_ -notin $occupied } | Select-Object -First 1
if (-not $letter) { throw 'No unused drive letter for mapped-drive test.' }
$drive = $letter + ':'
$mapped = $false
try {
    New-SmbMapping -LocalPath $drive -RemotePath $fixture -Persistent $false | Out-Null
    $mapped = $true
    & $probe $Exe ($drive + '\')
    if ($LASTEXITCODE -ne 0) { throw 'Mapped-drive broker/Office test failed.' }
} finally {
    if ($mapped) {
        $current = Get-SmbMapping -LocalPath $drive
        if ($current.RemotePath -ne $fixture) { throw 'Mapping changed; refusing to remove another mapping.' }
        Remove-SmbMapping -LocalPath $drive -Force -Confirm:$false
    }
}
[ordered]@{version=(Get-Item $Exe).VersionInfo.FileVersion;result='PASS';fixture=$fixture;
 checks=@('existing localhost SMB UNC','temporary mapped drive','unique project directories','TXT read and publish','DPAPI note reload','Word and Excel create/save/reopen','path escape rejection','original unchanged');company_share_tested=$false} |
 ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $projectRoot '.build\network-verification.json') -Encoding UTF8
