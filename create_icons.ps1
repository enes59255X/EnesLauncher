Add-Type -AssemblyName System.Drawing

function Create-LogoBitmap($size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)

    # Arka plan yuvarlak kutu
    $bgBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 11, 16, 29))
    $rect = New-Object System.Drawing.Rectangle(2, 2, ($size - 4), ($size - 4))
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $r = [int]($size * 0.22)
    $d = $r * 2
    $path.AddArc($rect.X, $rect.Y, $d, $d, 180, 90)
    $path.AddArc($rect.Right - $d, $rect.Y, $d, $d, 270, 90)
    $path.AddArc($rect.Right - $d, $rect.Bottom - $d, $d, $d, 0, 90)
    $path.AddArc($rect.X, $rect.Bottom - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    $g.FillPath($bgBrush, $path)

    # Z�mr�t D�� Kenarl�k
    $borderPen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(255, 52, 211, 153), [Math]::Max(1.5, $size * 0.03))
    $g.DrawPath($borderPen, $path)

    # 3D �zometrik Z�mr�t K�p
    $cx = $size / 2.0
    $cy = $size / 2.0
    $w = $size * 0.35
    $h = $size * 0.20

    $top = [System.Drawing.PointF]::new($cx, $cy - $h * 1.7)
    $mid = [System.Drawing.PointF]::new($cx, $cy)
    $bot = [System.Drawing.PointF]::new($cx, $cy + $h * 1.7)
    $left = [System.Drawing.PointF]::new($cx - $w, $cy - $h * 0.85)
    $right = [System.Drawing.PointF]::new($cx + $w, $cy - $h * 0.85)
    $botLeft = [System.Drawing.PointF]::new($cx - $w, $cy + $h * 0.85)
    $botRight = [System.Drawing.PointF]::new($cx + $w, $cy + $h * 0.85)

    # �st y�zey (A��k Z�mr�t)
    $topBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 52, 211, 153))
    $g.FillPolygon($topBrush, @($top, $right, $mid, $left))

    # Sol y�zey (Orta Z�mr�t)
    $leftBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 16, 185, 129))
    $g.FillPolygon($leftBrush, @($left, $mid, $bot, $botLeft))

    # Sa� y�zey (Koyu Z�mr�t)
    $rightBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 4, 120, 87))
    $g.FillPolygon($rightBrush, @($mid, $right, $botRight, $bot))

    # K�p Hatlar�
    $cubePen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(255, 167, 243, 208), [Math]::Max(1.0, $size * 0.02))
    $g.DrawPolygon($cubePen, @($top, $right, $botRight, $bot, $botLeft, $left))
    $g.DrawLine($cubePen, $mid, $top)
    $g.DrawLine($cubePen, $mid, $bot)
    $g.DrawLine($cubePen, $mid, $left)
    $g.DrawLine($cubePen, $mid, $right)

    # Beyaz 'E' Harfi
    $fontSize = [Math]::Max(10.0, $size * 0.32)
    $font = New-Object System.Drawing.Font("Arial", $fontSize, [System.Drawing.FontStyle]::Bold)
    $whiteBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
    $sf = New-Object System.Drawing.StringFormat
    $sf.Alignment = [System.Drawing.StringAlignment]::Center
    $sf.LineAlignment = [System.Drawing.StringAlignment]::Center
    $g.DrawString("E", $font, $whiteBrush, [System.Drawing.PointF]::new($cx, $cy), $sf)

    $g.Dispose()
    return $bmp
}

$iconsPath = "c:\Users\Enes\Desktop\Masa�st� Klas�r�\Opencode\EnesLauncher\apps\desktop\src-tauri\icons"
$publicPath = "c:\Users\Enes\Desktop\Masa�st� Klas�r�\Opencode\EnesLauncher\apps\desktop\public"

$sizes = @(32, 128, 256)
$bmps = @{}
foreach ($s in $sizes) {
    $bmps[$s] = Create-LogoBitmap $s
}

$bmps[32].Save("$iconsPath\32x32.png", [System.Drawing.Imaging.ImageFormat]::Png)
$bmps[128].Save("$iconsPath\128x128.png", [System.Drawing.Imaging.ImageFormat]::Png)
$bmps[256].Save("$iconsPath\128x128@2x.png", [System.Drawing.Imaging.ImageFormat]::Png)
$bmps[256].Save("$iconsPath\icon.png", [System.Drawing.Imaging.ImageFormat]::Png)
$bmps[256].Save("$publicPath\icon.png", [System.Drawing.Imaging.ImageFormat]::Png)

# Windows .ico �retimi (Icon handle'dan)
$iconHandle = $bmps[256].GetHicon()
$icon = [System.Drawing.Icon]::FromHandle($iconHandle)
$fs = New-Object System.IO.FileStream("$iconsPath\icon.ico", [System.IO.FileMode]::Create)
$icon.Save($fs)
$fs.Close()
$icon.Dispose()

Write-Host "T�m ikonlar ba�ar�yla �retildi!"
