# 建置機資產工具：只縮放／重編碼使用者指定圖片，不重新繪製牌面。
[CmdletBinding()]
param(
    [string]$Source = (Join-Path $PSScriptRoot '../.build/tarot-assets-source'),
    [string]$Back = 'Y:\Rust\午夜靛藍星象曼荼羅.png'
)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
$project=Split-Path $PSScriptRoot -Parent
$destination=Join-Path $project 'ui/tarot'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
$tree=Get-Content -LiteralPath (Join-Path $Source 'tree.json') -Raw | ConvertFrom-Json
$catalog=@(Get-Content -LiteralPath (Join-Path $project 'ui/tarot-data.json') -Raw | ConvertFrom-Json)
if($catalog.Count -ne 78){throw 'Expected 78 Chinese card records'}
$inputs=@(Get-ChildItem -LiteralPath (Join-Path $Source 'cards') -Filter '*.jpg' | Where-Object {$_.Name -match '^\d\d_'} | Sort-Object Name)
foreach($suit in @('Wands','Cups','Swords','Pents')) {
    1..14 | ForEach-Object { $inputs += Get-Item -LiteralPath (Join-Path $Source ('cards/{0}{1:00}.jpg' -f $suit,$_)) }
}
if($inputs.Count -ne 78){throw 'Expected 22 majors and 56 minors'}
$codec=[Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object MimeType -eq 'image/jpeg'
function Resize-Card([string]$inputPath,[string]$outputPath,[Drawing.Color]$background) {
    $original=[Drawing.Image]::FromFile($inputPath)
    $bitmap=New-Object Drawing.Bitmap(360,600,[Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $graphics=[Drawing.Graphics]::FromImage($bitmap)
    $parameters=New-Object Drawing.Imaging.EncoderParameters(1)
    try {
        $graphics.Clear($background)
        $graphics.InterpolationMode=[Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.PixelOffsetMode=[Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $ratio=[Math]::Min(360.0/$original.Width,600.0/$original.Height)
        $width=[int][Math]::Round($original.Width*$ratio)
        $height=[int][Math]::Round($original.Height*$ratio)
        $rectangle=New-Object Drawing.Rectangle([int]((360-$width)/2),[int]((600-$height)/2),$width,$height)
        $graphics.DrawImage($original,$rectangle)
        $parameters.Param[0]=New-Object Drawing.Imaging.EncoderParameter([Drawing.Imaging.Encoder]::Quality,[long]86)
        $bitmap.Save($outputPath,$codec,$parameters)
    } finally { $parameters.Dispose(); $graphics.Dispose(); $bitmap.Dispose(); $original.Dispose() }
    if((Get-Item -LiteralPath $outputPath).Length -ge 500000){throw 'Card exceeds 500 KB'}
}
$manifest=@()
for($index=0;$index -lt 78;$index++) {
    $file=('{0:00}.jpg' -f $index)
    $target=Join-Path $destination $file
    Resize-Card $inputs[$index].FullName $target ([Drawing.Color]::FromArgb(247,240,219))
    $catalog[$index] | Add-Member -NotePropertyName image -NotePropertyValue ('tarot/'+$file) -Force
    $manifest += [ordered]@{index=$index;name=$catalog[$index].name;file=$file;source=$inputs[$index].Name;sha256=(Get-FileHash -LiteralPath $target).Hash.ToLowerInvariant();bytes=(Get-Item -LiteralPath $target).Length}
}
$backTarget=Join-Path $destination 'back.jpg'
Resize-Card $Back $backTarget ([Drawing.Color]::FromArgb(4,13,32))
$manifest += [ordered]@{file='back.jpg';source='user-provided';source_sha256=(Get-FileHash -LiteralPath $Back).Hash.ToLowerInvariant();sha256=(Get-FileHash -LiteralPath $backTarget).Hash.ToLowerInvariant();bytes=(Get-Item -LiteralPath $backTarget).Length}
$manifestJson=[ordered]@{repository='https://github.com/mixvlad/TarotCards';commit=$tree.commit;width=360;height=600;format='JPEG';quality=86;fit='contain, no crop';license='Rider-Waite images: Public Domain per upstream metadata';images=$manifest} | ConvertTo-Json -Depth 6
[IO.File]::WriteAllText((Join-Path $destination 'manifest.json'),$manifestJson,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $project 'ui/tarot-data.json'),($catalog | ConvertTo-Json -Depth 4),[Text.UTF8Encoding]::new($false))
Write-Output ('Prepared 78 faces and one back; total {0:N0} bytes' -f (($manifest | ForEach-Object { $_['bytes'] } | Measure-Object -Sum).Sum))
