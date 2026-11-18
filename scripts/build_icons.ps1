Add-Type -AssemblyName System.Drawing

function New-IconImage([int]$size, [string]$theme = "active") {
    $bmp = [System.Drawing.Bitmap]::new($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)

    $center = $size / 2.0
    $radius = ($size / 2.0) - [Math]::Max(1.0, $size * 0.04)

    # Theme colors
    $primaryColor = switch ($theme) {
        "active" { [System.Drawing.Color]::FromArgb(255, 0, 240, 255) }     # Cyan
        "bypassed" { [System.Drawing.Color]::FromArgb(255, 245, 158, 11) }   # Amber
        "error" { [System.Drawing.Color]::FromArgb(255, 239, 68, 68) }       # Red
        Default { [System.Drawing.Color]::FromArgb(255, 0, 240, 255) }
    }
    $bgColor = [System.Drawing.Color]::FromArgb(255, 10, 12, 18)
    $rimColor = [System.Drawing.Color]::FromArgb(255, 30, 41, 59)

    # 1. Dark outer base
    $bgBrush = [System.Drawing.SolidBrush]::new($bgColor)
    $g.FillEllipse($bgBrush, ($center - $radius), ($center - $radius), ($radius * 2), ($radius * 2))
    $bgBrush.Dispose()

    # 2. Outer rim
    $rimPen = [System.Drawing.Pen]::new($rimColor, [Math]::Max(1.0, $size * 0.05))
    $g.DrawEllipse($rimPen, ($center - $radius), ($center - $radius), ($radius * 2), ($radius * 2))
    $rimPen.Dispose()

    # 3. Inner frequency ring
    $innerRadius = $radius * 0.72
    $innerPen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(80, $primaryColor.R, $primaryColor.G, $primaryColor.B), [Math]::Max(1.0, $size * 0.03))
    $g.DrawEllipse($innerPen, ($center - $innerRadius), ($center - $innerRadius), ($innerRadius * 2), ($innerRadius * 2))
    $innerPen.Dispose()

    # 4. Central sine/signal wave
    $wavePen = [System.Drawing.Pen]::new($primaryColor, [Math]::Max(1.5, $size * 0.08))
    $wavePen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $wavePen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round

    $points = [System.Collections.Generic.List[System.Drawing.PointF]]::new()
    $numSteps = 24
    $startX = $center - ($radius * 0.65)
    $endX = $center + ($radius * 0.65)
    $spanX = $endX - $startX
    $amp = $radius * 0.35

    for ($i = 0; $i -le $numSteps; $i++) {
        $t = $i / $numSteps
        $x = $startX + ($t * $spanX)
        $phase = $t * [Math]::PI * 2.0
        $y = $center - ([Math]::Sin($phase) * $amp)
        $points.Add([System.Drawing.PointF]::new($x, $y))
    }
    $g.DrawCurve($wavePen, $points.ToArray(), 0.5)
    $wavePen.Dispose()

    # 5. Center pulse dot (at 180 Hz node)
    $dotRadius = [Math]::Max(1.5, $size * 0.07)
    $dotBrush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::White)
    $g.FillEllipse($dotBrush, ($center - $dotRadius), ($center - $dotRadius), ($dotRadius * 2), ($dotRadius * 2))
    $dotBrush.Dispose()

    # 6. Top 180 Hz marker indicator
    $topMarkPen = [System.Drawing.Pen]::new($primaryColor, [Math]::Max(1.5, $size * 0.07))
    $g.DrawLine($topMarkPen, [float]$center, [float]($center - $radius), [float]$center, [float]($center - $radius + ($size * 0.12)))
    $topMarkPen.Dispose()

    $g.Dispose()
    return $bmp
}

function Save-Ico([System.Drawing.Bitmap[]]$bitmaps, [string]$outputPath) {
    $fs = [System.IO.File]::Create($outputPath)
    $bw = [System.IO.BinaryWriter]::new($fs)

    # ICONDIR header: idReserved (0), idType (1 for ICO), idCount
    $bw.Write([uint16]0)
    $bw.Write([uint16]1)
    $bw.Write([uint16]$bitmaps.Length)

    $pngStreams = @()
    $offset = 6 + ($bitmaps.Length * 16)

    # Encode all bitmaps to PNG in memory
    foreach ($bmp in $bitmaps) {
        $ms = [System.IO.MemoryStream]::new()
        $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
        $pngStreams += $ms
    }

    # Write ICONDIRENTRY for each image
    for ($i = 0; $i -lt $bitmaps.Length; $i++) {
        $bmp = $bitmaps[$i]
        $ms = $pngStreams[$i]
        $w = if ($bmp.Width -ge 256) { 0 } else { [byte]$bmp.Width }
        $h = if ($bmp.Height -ge 256) { 0 } else { [byte]$bmp.Height }

        $bw.Write([byte]$w)          # bWidth
        $bw.Write([byte]$h)          # bHeight
        $bw.Write([byte]0)           # bColorCount
        $bw.Write([byte]0)           # bReserved
        $bw.Write([uint16]1)         # wPlanes
        $bw.Write([uint16]32)        # wBitCount
        $bw.Write([uint32]$ms.Length)# dwBytesInRes
        $bw.Write([uint32]$offset)   # dwImageOffset

        $offset += $ms.Length
    }

    # Write PNG byte data
    for ($i = 0; $i -lt $bitmaps.Length; $i++) {
        $ms = $pngStreams[$i]
        $bw.Write($ms.ToArray())
        $ms.Dispose()
    }

    $bw.Flush()
    $bw.Close()
    $fs.Close()
}

$sizes = @(16, 24, 32, 48, 64, 128, 256)

# Generate Main App Icon
$appBitmaps = $sizes | ForEach-Object { New-IconImage -size $_ -theme "active" }
Save-Ico -bitmaps $appBitmaps -outputPath "crates\quietmic-app\icons\icon.ico"
Save-Ico -bitmaps $appBitmaps -outputPath "crates\quietmic-installer\resources\icon.ico"
$appBitmaps[-1].Save("crates\quietmic-app\icons\icon.png", [System.Drawing.Imaging.ImageFormat]::Png)

# Generate Tray Icons (Active, Bypassed, Error)
$trayActive = @(16, 24, 32, 48) | ForEach-Object { New-IconImage -size $_ -theme "active" }
Save-Ico -bitmaps $trayActive -outputPath "crates\quietmic-app\icons\tray-active.ico"

$trayBypassed = @(16, 24, 32, 48) | ForEach-Object { New-IconImage -size $_ -theme "bypassed" }
Save-Ico -bitmaps $trayBypassed -outputPath "crates\quietmic-app\icons\tray-bypassed.ico"

$trayError = @(16, 24, 32, 48) | ForEach-Object { New-IconImage -size $_ -theme "error" }
Save-Ico -bitmaps $trayError -outputPath "crates\quietmic-app\icons\tray-error.ico"

Write-Host "Icons successfully generated!"
