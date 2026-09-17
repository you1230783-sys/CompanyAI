# 將使用者提供的 PNG 轉為 Windows 多尺寸 ICO；只需 Windows 內建的 .NET。
# 日常 Rust 建置直接使用 assets/app.ico，不需每次重新轉換。
[CmdletBinding()]
param(
    [string]$SourcePath,
    [string]$OutputPath
)
$ErrorActionPreference = 'Stop'
if (-not $SourcePath) { $SourcePath = Join-Path $PSScriptRoot '..\assets\app.png' }
if (-not $OutputPath) { $OutputPath = Join-Path $PSScriptRoot '..\assets\app.ico' }
Add-Type -AssemblyName System.Drawing

# 涵蓋一般與高 DPI 工作列、托盤、Alt+Tab 及檔案總管常用尺寸。
$sizes = @(16, 20, 24, 32, 40, 48, 64, 96, 128, 256)
$frames = [Collections.Generic.List[byte[]]]::new()
$source = [Drawing.Image]::FromFile([IO.Path]::GetFullPath($SourcePath))
try {
    foreach ($size in $sizes) {
        $bitmap = [Drawing.Bitmap]::new($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
        try {
            $graphics = [Drawing.Graphics]::FromImage($bitmap)
            try {
                $graphics.Clear([Drawing.Color]::Transparent)
                $graphics.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
                $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
                # 等比例置中，保留透明背景與完整圖案，不裁切或拉伸原圖。
                $scale = [Math]::Min($size / $source.Width, $size / $source.Height)
                $width = [Math]::Max(1, [int][Math]::Round($source.Width * $scale))
                $height = [Math]::Max(1, [int][Math]::Round($source.Height * $scale))
                $left = [int][Math]::Floor(($size - $width) / 2)
                $top = [int][Math]::Floor(($size - $height) / 2)
                $graphics.DrawImage($source, $left, $top, $width, $height)
            } finally { $graphics.Dispose() }
            $png = [IO.MemoryStream]::new()
            try {
                $bitmap.Save($png, [Drawing.Imaging.ImageFormat]::Png)
                $frames.Add($png.ToArray())
            } finally { $png.Dispose() }
        } finally { $bitmap.Dispose() }
    }
} finally { $source.Dispose() }

# ICO = 6 位元組標頭 + 每尺寸 16 位元組索引 + 各尺寸 PNG。
# Windows 11 支援 PNG 圖框；直接保存 alpha，避免黑底或失去透明邊緣。
$buffer = [IO.MemoryStream]::new()
$writer = [IO.BinaryWriter]::new($buffer)
try {
    $writer.Write([uint16]0)
    $writer.Write([uint16]1)
    $writer.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($index = 0; $index -lt $sizes.Count; $index++) {
        # ICO 索引以 0 表示 256 像素。
        $dimension = if ($sizes[$index] -eq 256) { 0 } else { $sizes[$index] }
        $writer.Write([byte]$dimension)
        $writer.Write([byte]$dimension)
        $writer.Write([byte]0)
        $writer.Write([byte]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]32)
        $writer.Write([uint32]$frames[$index].Length)
        $writer.Write([uint32]$offset)
        $offset += $frames[$index].Length
    }
    foreach ($frame in $frames) { $writer.Write($frame) }
    $writer.Flush()
    [IO.File]::WriteAllBytes([IO.Path]::GetFullPath($OutputPath), $buffer.ToArray())
} finally {
    $writer.Dispose()
    $buffer.Dispose()
}
Write-Host "Created: $OutputPath ($($sizes -join ', ') px)"
