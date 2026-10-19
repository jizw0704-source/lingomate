param(
    [Parameter(Mandatory=$true)][string]$Output,
    [ValidateSet("brand", "chinese", "english")][string]$Kind = "brand"
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$Sizes = @(16,20,24,32,40,48,64,128,256)
$Images = @()
$Glyph = switch ($Kind) { "chinese" { "中" } "english" { "英" } default { "果" } }
foreach ($Size in $Sizes) {
    $Bitmap = New-Object Drawing.Bitmap($Size,$Size)
    $Graphics = [Drawing.Graphics]::FromImage($Bitmap)
    $Graphics.Clear([Drawing.Color]::Transparent)
    $Graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $Graphics.TextRenderingHint = [Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $Background = New-Object Drawing.SolidBrush([Drawing.Color]::FromArgb(255,25,25,25))
    $Path = New-Object Drawing.Drawing2D.GraphicsPath
    $Radius = [single]($Size * 0.38)
    $Edge = [single]($Size - 1)
    $Path.AddArc(0,0,$Radius,$Radius,180,90)
    $Path.AddArc(($Edge-$Radius),0,$Radius,$Radius,270,90)
    $Path.AddArc(($Edge-$Radius),($Edge-$Radius),$Radius,$Radius,0,90)
    $Path.AddArc(0,($Edge-$Radius),$Radius,$Radius,90,90)
    $Path.CloseFigure()
    $Graphics.FillPath($Background,$Path)
    $Font = New-Object Drawing.Font('Microsoft YaHei UI',[single]($Size * 0.72),[Drawing.FontStyle]::Bold,[Drawing.GraphicsUnit]::Pixel)
    $Format = New-Object Drawing.StringFormat
    $Format.Alignment = [Drawing.StringAlignment]::Center
    $Format.LineAlignment = [Drawing.StringAlignment]::Center
    $Graphics.DrawString($Glyph,$Font,[Drawing.Brushes]::White,[Drawing.RectangleF]::new(0,-1,$Size,$Size),$Format)
    $Stream = New-Object IO.MemoryStream
    $Bitmap.Save($Stream,[Drawing.Imaging.ImageFormat]::Png)
    $Images += ,$Stream.ToArray()
    $Stream.Dispose(); $Format.Dispose(); $Font.Dispose(); $Path.Dispose(); $Background.Dispose(); $Graphics.Dispose(); $Bitmap.Dispose()
}
$File = [IO.File]::Create($Output)
$Writer = New-Object IO.BinaryWriter($File)
try {
    $Writer.Write([uint16]0); $Writer.Write([uint16]1); $Writer.Write([uint16]$Sizes.Count)
    $Offset = 6 + 16 * $Sizes.Count
    for ($i=0; $i -lt $Sizes.Count; $i++) {
        $Dimension = if ($Sizes[$i] -eq 256) { 0 } else { $Sizes[$i] }
        $Writer.Write([byte]$Dimension); $Writer.Write([byte]$Dimension)
        $Writer.Write([byte]0); $Writer.Write([byte]0); $Writer.Write([uint16]1); $Writer.Write([uint16]32)
        $Writer.Write([uint32]$Images[$i].Length); $Writer.Write([uint32]$Offset)
        $Offset += $Images[$i].Length
    }
    foreach ($Image in $Images) { $Writer.Write([byte[]]$Image) }
} finally { $Writer.Dispose(); $File.Dispose() }
