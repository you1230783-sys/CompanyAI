# 產生可攜帶的完整專案 ZIP，並在解壓後以空 Cargo 快取驗證。
[CmdletBinding()]
param([switch]$RefreshDependencies)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Enter-DevShell.ps1')
$deliveryRoot = $projectRoot
$sourceToolchain = Split-Path $toolchainBin -Parent
$workBase = Join-Path $deliveryRoot '.build'
$workRoot = Join-Path $workBase ('delivery-' + [guid]::NewGuid().ToString('N'))
$stage = Join-Path $workRoot 'stage'
$verified = Join-Path $workRoot 'verified'
New-Item -ItemType Directory -Path $stage,$verified -Force | Out-Null

function Copy-Tree([string]$SourcePath, [string]$DestinationPath) {
    # 不使用 /MIR，避免清除目的地的其他內容；只複製目前選定的樹。
    & robocopy.exe $SourcePath $DestinationPath /E /XF '*.pdb' /R:1 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "Copy failed: $SourcePath" }
}

Push-Location $deliveryRoot
try {
    # 現有 vendor 可直接離線重建；變更依賴後可改從開發機 registry 快取準備。
    $vendorNext = Join-Path $workRoot 'vendor-next'
    $vendorNext = [IO.Path]::GetFullPath($vendorNext)
    if (-not $vendorNext.StartsWith($workBase + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe new vendor path.' }
    $vendorArgs = @('vendor', '--frozen', '--versioned-dirs')
    if ((Test-Path -LiteralPath 'vendor') -and -not $RefreshDependencies) { $vendorArgs += '--respect-source-config' }
    & cargo @vendorArgs $vendorNext
    if ($LASTEXITCODE -ne 0) { throw 'Vendoring failed. Update Cargo.lock and download dependencies before packaging.' }
    # 先完整產生新版本再替換；這兩個目錄均為本專案指定的產生物。
    $vendorPath = [IO.Path]::GetFullPath((Join-Path $deliveryRoot 'vendor'))
    if (-not $vendorPath.StartsWith($deliveryRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe vendor path.' }
    if (Test-Path -LiteralPath $vendorPath) {
        if ((Get-Item -LiteralPath $vendorPath).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'vendor must not be a link.' }
        $vendorBackup = [IO.Path]::GetFullPath((Join-Path $workRoot 'vendor-previous'))
        if (-not $vendorBackup.StartsWith($workBase + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe vendor backup path.' }
        Move-Item -LiteralPath $vendorPath -Destination $vendorBackup
    }
    Move-Item -LiteralPath $vendorNext -Destination $vendorPath

    & (Join-Path $PSScriptRoot 'Build.ps1')

    # 明確列舉交付內容；不帶入 .git、快取、登入資料或另一個專案的資源。
    $projectItems = @('src','ui','assets','examples','scripts','docs','.cargo','dist','build.rs','Cargo.toml','Cargo.lock','rust-toolchain.toml','README.md','AGENTS.md','.gitignore','.gitattributes')
    foreach ($item in $projectItems) { Copy-Item -LiteralPath (Join-Path $deliveryRoot $item) -Destination $stage -Recurse -Force }
    Copy-Tree $vendorPath (Join-Path $stage 'vendor')
    $portableToolchain = Join-Path $stage 'toolchain'
    foreach ($part in @('bin','lib','libexec','etc')) {
        $partPath = Join-Path $sourceToolchain $part
        if (Test-Path -LiteralPath $partPath) { Copy-Tree $partPath (Join-Path $portableToolchain $part) }
    }
    # 保留 Rust 發行版的著作權資訊；不攜帶數萬份 API HTML 說明與 PDB。
    $licenseSource = Join-Path $sourceToolchain 'licenses'
    if (Test-Path -LiteralPath $licenseSource) { Copy-Tree $licenseSource (Join-Path $portableToolchain 'licenses') }
    else {
        $licenseSource = Join-Path $sourceToolchain 'share\doc\rust'
        New-Item -ItemType Directory -Path (Join-Path $portableToolchain 'licenses') -Force | Out-Null
        foreach ($name in @('COPYRIGHT.html','COPYRIGHT-library.html','README.md')) {
            Copy-Item -LiteralPath (Join-Path $licenseSource $name) -Destination (Join-Path $portableToolchain 'licenses')
        }
    }
    New-Item -ItemType Directory -Path (Join-Path $stage 'offline') -Force | Out-Null
    Copy-Item -LiteralPath 'offline\environment.txt' -Destination (Join-Path $stage 'offline\environment.txt')

    # 記錄原始碼與 EXE 的 SHA256，使 ZIP 與 Git 中的版本可互相比對。
    $files = @()
    foreach ($item in $projectItems) {
        $entry = Get-Item -LiteralPath (Join-Path $stage $item)
        $selected = if ($entry.PSIsContainer) { Get-ChildItem -LiteralPath $entry.FullName -Recurse -File } else { @($entry) }
        foreach ($file in $selected) {
            $files += [ordered]@{ path = $file.FullName.Substring($stage.Length + 1).Replace('\','/'); sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
        }
    }
    $manifest = [ordered]@{
        generated_at = (Get-Date -Format o)
        rust = '1.98.1'
        target = 'x86_64-pc-windows-msvc'
        msvc = $env:VCToolsVersion
        windows_sdk = $env:WindowsSDKVersion.TrimEnd('\')
        prerequisite = 'Installed MSVC v142 (14.29), Windows SDK 10.0.19041.0 and WebView2 Runtime (offline installer included)'
        files = $files
    }
    $manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $stage 'offline\manifest.json') -Encoding UTF8

    $candidate = Join-Path $workRoot 'CompanyAI-offline.zip'
    & tar.exe -a -c -f $candidate -C $stage @projectItems vendor toolchain offline
    if ($LASTEXITCODE -ne 0) { throw 'ZIP creation failed.' }
    & tar.exe -x -f $candidate -C $verified
    if ($LASTEXITCODE -ne 0) { throw 'ZIP extraction failed.' }
    if (Test-Path -LiteralPath (Join-Path $verified '.tools')) { throw 'Verification must start with an empty Cargo home.' }
    if (Test-Path -LiteralPath (Join-Path $verified 'target')) { throw 'Verification must start without build outputs.' }

    # 子程序從解壓後的根目錄執行，原有 CARGO_TARGET_DIR / RUSTC 由包內腳本重設。
    $verifyScript = Join-Path $verified 'scripts\Build.ps1'
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $verifyScript
    if ($LASTEXITCODE -ne 0) { throw 'Offline ZIP verification failed. Previous delivery is retained.' }
    # 在解壓後的位置驗證 VS Code 設定產生；本機路徑不寫進候選 ZIP。
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $verified 'scripts\Configure-VSCode.ps1')
    if ($LASTEXITCODE -ne 0) { throw 'Offline VS Code configuration failed.' }
    $verificationText = Get-Content -LiteralPath (Join-Path $verified 'offline\environment.txt') -Raw
    if (-not $verificationText.Contains((Join-Path $verified 'toolchain\bin'))) { throw 'Verification did not use the bundled Rust toolchain.' }

    # 全部驗證成功才更新固定的 ZIP 檔名；失敗時須重跑成功才能提交整組產物。
    Copy-Item -LiteralPath $candidate -Destination 'offline\CompanyAI-offline.zip' -Force
    Copy-Item -LiteralPath (Join-Path $stage 'offline\manifest.json') -Destination 'offline\manifest.json' -Force
    $checksum = (Get-FileHash -LiteralPath 'offline\CompanyAI-offline.zip' -Algorithm SHA256).Hash.ToLowerInvariant()
    "$checksum  CompanyAI-offline.zip" | Set-Content -LiteralPath 'offline\CompanyAI-offline.zip.sha256' -Encoding ASCII
    [ordered]@{ verified_at = (Get-Date -Format o); archive_sha256 = $checksum; result = 'PASS'; rust_source = 'bundled toolchain'; cargo_cache = 'empty at start'; network = 'Cargo --frozen; local HTTP/WebSocket tests; embedded UI without CDN'; checks = 'fmt, clippy, workspace tests, release build, WebView2 Markdown and scrolling self-check'; prerequisite = $manifest.prerequisite } |
        ConvertTo-Json | Set-Content -LiteralPath 'offline\verification.json' -Encoding UTF8
    Write-Host "Ready for Git: $deliveryRoot\offline\CompanyAI-offline.zip"

    # 只清理此次建立的工作資料夾；先核對最終絕對路徑，避免遞迴刪錯位置。
    $resolvedWork = (Resolve-Path -LiteralPath $workRoot).Path
    $resolvedBase = (Resolve-Path -LiteralPath $workBase).Path
    if (-not $resolvedWork.StartsWith($resolvedBase.TrimEnd('\') + '\delivery-', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe cleanup path.' }
    Remove-Item -LiteralPath $resolvedWork -Recurse -Force
} finally { Pop-Location }
