# Desenha o ícone-base do app (1024x1024) com System.Drawing, sem depender de nada instalado.
# Uso:  powershell -ExecutionPolicy Bypass -File scripts\make-icon.ps1
# Depois: npm run tauri icon src-tauri/icon-source.png
# Para usar outra arte, basta rodar o segundo comando com o seu PNG quadrado de 1024 px.
param(
    [string]$Out = "src-tauri\icon-source.png"
)

Add-Type -AssemblyName System.Drawing

$size = 1024
$bitmap = New-Object System.Drawing.Bitmap $size, $size
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$graphics.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$graphics.Clear([System.Drawing.Color]::Transparent)

function New-RoundedRect([float]$x, [float]$y, [float]$w, [float]$h, [float]$r) {
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = 2 * $r
    $path.AddArc($x, $y, $d, $d, 180, 90)
    $path.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $path.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $path.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    return $path
}

# Fundo: quadrado arredondado em degradê azul (a cor de destaque do app).
$bounds = New-Object System.Drawing.Rectangle 64, 64, 896, 896
$gradient = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    $bounds,
    [System.Drawing.Color]::FromArgb(255, 77, 134, 245),
    [System.Drawing.Color]::FromArgb(255, 31, 79, 196),
    45.0)
$graphics.FillPath($gradient, (New-RoundedRect 64 64 896 896 200))

# Dois balões de fala: o de cima com "EN", o de baixo com "PT".
$white = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 255, 255, 255))
$softWhite = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 226, 236, 255))
$ink = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 31, 79, 196))
$format = New-Object System.Drawing.StringFormat
$format.Alignment = [System.Drawing.StringAlignment]::Center
$format.LineAlignment = [System.Drawing.StringAlignment]::Center
$font = New-Object System.Drawing.Font("Segoe UI", 190, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)

$graphics.FillPath($white, (New-RoundedRect 156 214 520 330 90))
$graphics.FillPolygon($white, @(
    (New-Object System.Drawing.PointF 230, 530),
    (New-Object System.Drawing.PointF 230, 640),
    (New-Object System.Drawing.PointF 340, 540)))
$graphics.DrawString("EN", $font, $ink, (New-Object System.Drawing.RectangleF 156, 214, 520, 330), $format)

$graphics.FillPath($softWhite, (New-RoundedRect 348 480 520 330 90))
$graphics.FillPolygon($softWhite, @(
    (New-Object System.Drawing.PointF 794, 796),
    (New-Object System.Drawing.PointF 794, 906),
    (New-Object System.Drawing.PointF 684, 806)))
$graphics.DrawString("PT", $font, $ink, (New-Object System.Drawing.RectangleF 348, 480, 520, 330), $format)

$target = [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $Out))
$bitmap.Save($target, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
Write-Output "Ícone gravado em $target"
