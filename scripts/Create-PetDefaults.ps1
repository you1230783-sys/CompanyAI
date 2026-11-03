# 可重製的基本角色：使用程式繪製簡單向量形狀，輸出六種狀態的透明PNG。
# 使用者匯入圖片不會執行此腳本；執行時只更新專案內ui/pets的內建圖檔。
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
$destination=Join-Path (Split-Path $PSScriptRoot -Parent) 'ui/pets'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
foreach($animal in @('cat','dog')) {
 foreach($state in @('idle','thinking','working','waiting','success','error')) {
  $bitmap=New-Object Drawing.Bitmap 256,256
  $graphics=[Drawing.Graphics]::FromImage($bitmap)
  $graphics.SmoothingMode=[Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $graphics.Clear([Drawing.Color]::Transparent)
  $body=New-Object Drawing.SolidBrush ([Drawing.ColorTranslator]::FromHtml($(if($animal -eq 'cat'){'#F3C47B'}else{'#BC8967'})))
  $cream=New-Object Drawing.SolidBrush ([Drawing.ColorTranslator]::FromHtml('#FFF1D8'))
  $ink=New-Object Drawing.Pen ([Drawing.ColorTranslator]::FromHtml('#4B3835')),6
  $pink=New-Object Drawing.SolidBrush ([Drawing.ColorTranslator]::FromHtml('#F3A5A0'))
  try {
   $graphics.FillEllipse($body,65,110,126,116);$graphics.FillEllipse($cream,90,151,76,66)
   if($animal -eq 'cat') {
    $graphics.FillPolygon($body,[Drawing.Point[]]@([Drawing.Point]::new(49,88),[Drawing.Point]::new(53,23),[Drawing.Point]::new(99,56)))
    $graphics.FillPolygon($body,[Drawing.Point[]]@([Drawing.Point]::new(157,56),[Drawing.Point]::new(203,23),[Drawing.Point]::new(207,88)))
    $graphics.FillPolygon($pink,[Drawing.Point[]]@([Drawing.Point]::new(61,66),[Drawing.Point]::new(62,39),[Drawing.Point]::new(84,57)))
    $graphics.FillPolygon($pink,[Drawing.Point[]]@([Drawing.Point]::new(172,57),[Drawing.Point]::new(194,39),[Drawing.Point]::new(195,66)))
   }else{$graphics.FillEllipse($body,31,46,45,105);$graphics.FillEllipse($body,180,46,45,105)}
   $graphics.FillEllipse($body,47,44,162,128);$graphics.FillEllipse($cream,85,112,86,47)
   if($state -in @('success','waiting')){$graphics.DrawArc($ink,83,83,25,18,185,170);$graphics.DrawArc($ink,149,83,25,18,185,170)}
   else{$graphics.DrawLine($ink,97,90,97,105);$graphics.DrawLine($ink,159,90,159,105)}
   $graphics.FillEllipse($pink,68,111,21,12);$graphics.FillEllipse($pink,168,111,21,12)
   $graphics.DrawLine($ink,124,119,132,119);$graphics.DrawArc($ink,114,119,28,22,0,180)
   $graphics.FillEllipse($body,61,206,58,26);$graphics.FillEllipse($body,138,206,58,26)
   $armY=if($state -eq 'success'){125}else{167}
   $graphics.FillEllipse($body,39,$armY,49,29);$graphics.FillEllipse($body,168,$armY,49,29)
   $symbol=switch($state){'thinking'{'?'} 'working'{'▰'} 'waiting'{'…'} 'success'{'★'} 'error'{'!'} default{''}}
   if($symbol){$font=New-Object Drawing.Font 'Segoe UI',27,([Drawing.FontStyle]::Bold);try{$graphics.DrawString($symbol,$font,[Drawing.Brushes]::DarkSlateBlue,185,2)}finally{$font.Dispose()}}
   $bitmap.Save((Join-Path $destination ($animal+'_'+$state+'.png')),[Drawing.Imaging.ImageFormat]::Png)
  } finally {$graphics.Dispose();$bitmap.Dispose();$body.Dispose();$cream.Dispose();$ink.Dispose();$pink.Dispose()}
 }
}
