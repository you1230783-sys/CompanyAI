# 將AI生成的3×2透明角色圖集，切成六個同尺寸PNG；不重新繪製角色。
# 原始圖集及提示詞保留於assets/pet-illustrations，方便日後重新輸出。
[CmdletBinding()]
param([string]$SourceDirectory, [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$root = Split-Path $PSScriptRoot -Parent
if (-not $SourceDirectory) { $SourceDirectory = Join-Path $root 'assets/pet-illustrations' }
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $root 'ui/pets' }
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$actions = @('idle','thinking','working','waiting','success','error')
$manifest = [System.Collections.Generic.List[object]]::new()
foreach ($animal in @('cat','dog')) {
    $source = Join-Path $SourceDirectory ($animal + '-atlas.png')
    $atlas = [Drawing.Bitmap]::new($source)
    try {
        if ($atlas.Width -ne 1536 -or $atlas.Height -ne 1024) { throw "Expected a 1536x1024 atlas: $source" }
        for ($index = 0; $index -lt 6; $index++) {
            $bitmap = [Drawing.Bitmap]::new(256,256,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
            $graphics = [Drawing.Graphics]::FromImage($bitmap)
            try {
                $graphics.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
                $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
                $graphics.Clear([Drawing.Color]::Transparent)
                # 固定格子與畫布保留基準線及透明邊距，不依動作個別拉伸或裁掉耳尾。
                $cellX = ($index % 3) * 512
                $cellY = [math]::Floor($index / 3) * 512
                $graphics.DrawImage($atlas,[Drawing.Rectangle]::new(0,0,256,256),$cellX,$cellY,512,512,[Drawing.GraphicsUnit]::Pixel)
                $name = $animal + '_' + $actions[$index] + '.png'
                $path = Join-Path $OutputDirectory $name
                $bitmap.Save($path,[Drawing.Imaging.ImageFormat]::Png)
                $bytes = (Get-Item -LiteralPath $path).Length
                if ($bytes -ge 500000) { throw "PNG exceeds 500KB: $name" }
                $transparent = 0; $solid = 0
                for ($y=0;$y -lt 256;$y++) { for ($x=0;$x -lt 256;$x++) {
                    $alpha = $bitmap.GetPixel($x,$y).A
                    if ($alpha -le 8) { $transparent++ }
                    if ($alpha -ge 240) { $solid++ }
                } }
                if ($transparent -lt 5000 -or $solid -lt 5000) { throw "Unexpected alpha silhouette: $name" }
                $manifest.Add([ordered]@{file=$name;bytes=$bytes;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant();width=256;height=256;near_transparent_pixels=$transparent;solid_pixels=$solid})
            } finally { $graphics.Dispose(); $bitmap.Dispose() }
        }
    } finally { $atlas.Dispose() }
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $SourceDirectory 'manifest.json') -Encoding utf8
Write-Output 'Prepared 12 illustrated pet poses; all 256x256, under 500KB with alpha.'
