# R3-VISUAL-DPI-TRAY: programmatic four-state tray icon generator.
#
# Produces classic (BMP/DIB) multi-size .ico files -- no PNG-compressed ICO,
# so the Windows system_tray plugin can load them with plain LoadImage.
# Shapes/colors encode the four TrayIconStatus values:
#   tray_icon.ico       normal      : gray ring (proxy off, core stopped)
#   tray_icon_core.ico  coreRunning : green disc + white play (core running)
#   tray_icon_proxy.ico proxyActive : blue rounded square + white up arrow
#   tray_icon_pac.ico   proxyPac    : orange diamond
#
# Run: pwsh -NoProfile -File make_tray_icons.ps1
param(
  [string]$OutDir
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

if (-not $OutDir) {
  $repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
  $OutDir = Join-Path $repo 'apps\desktop\assets\tray'
}
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

$canvas = 128
$sizes = @(16, 32, 48)

function New-Base {
  $b = New-Object System.Drawing.Bitmap($canvas, $canvas, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($b)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.Clear([System.Drawing.Color]::Transparent)
  return @($b, $g)
}

function New-Scaled { param([System.Drawing.Bitmap]$Base, [int]$Size)
  $b = New-Object System.Drawing.Bitmap($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($b)
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.DrawImage($Base, 0, 0, $Size, $Size)
  $g.Dispose()
  return $b
}

function Write-Ico { param([string]$Path, [System.Drawing.Bitmap[]]$Bitmaps)
  $images = New-Object System.Collections.Generic.List[byte[]]
  foreach ($bmp in $Bitmaps) {
    $w = $bmp.Width; $h = $bmp.Height
    $xorStride = $w * 4
    $andStride = [math]::Ceiling($w / 8.0)
    while ($andStride % 4 -ne 0) { $andStride++ }
    $xor = New-Object byte[] ($xorStride * $h)
    $and = New-Object byte[] ($andStride * $h)
    for ($y = 0; $y -lt $h; $y++) {
      $srcY = $h - 1 - $y
      for ($x = 0; $x -lt $w; $x++) {
        $c = $bmp.GetPixel($x, $srcY)
        $o = $y * $xorStride + $x * 4
        $xor[$o] = $c.B; $xor[$o + 1] = $c.G; $xor[$o + 2] = $c.R; $xor[$o + 3] = $c.A
      }
    }
    $hdr = New-Object System.IO.MemoryStream
    $hb = New-Object System.IO.BinaryWriter($hdr)
    $hb.Write([uint32]40)
    $hb.Write([int32]$w)
    $hb.Write([int32]($h * 2))
    $hb.Write([uint16]1)
    $hb.Write([uint16]32)
    $hb.Write([uint32]0)
    $hb.Write([uint32]($xor.Length + $and.Length))
    $hb.Write([int32]0); $hb.Write([int32]0)
    $hb.Write([uint32]0); $hb.Write([uint32]0)
    $hb.Flush()
    $hbytes = $hdr.ToArray()
    $img = New-Object byte[] ($hbytes.Length + $xor.Length + $and.Length)
    [array]::Copy($hbytes, 0, $img, 0, $hbytes.Length)
    [array]::Copy($xor, 0, $img, $hbytes.Length, $xor.Length)
    [array]::Copy($and, 0, $img, $hbytes.Length + $xor.Length, $and.Length)
    $images.Add($img)
  }
  $fs = [System.IO.File]::Create($Path)
  $bw = New-Object System.IO.BinaryWriter($fs)
  try {
    $bw.Write([uint16]0); $bw.Write([uint16]1); $bw.Write([uint16]$Bitmaps.Count)
    $offset = 6 + 16 * $Bitmaps.Count
    for ($i = 0; $i -lt $Bitmaps.Count; $i++) {
      $w = $Bitmaps[$i].Width
      $bw.Write([byte]$(if ($w -ge 256) { 0 } else { $w }))
      $bw.Write([byte]$(if ($w -ge 256) { 0 } else { $w }))
      $bw.Write([byte]0); $bw.Write([byte]0)
      $bw.Write([uint16]1); $bw.Write([uint16]32)
      $bw.Write([uint32]$images[$i].Length)
      $bw.Write([uint32]$offset)
      $offset += $images[$i].Length
    }
    foreach ($img in $images) { $bw.Write($img) }
  } finally { $bw.Dispose(); $fs.Dispose() }
}

function Save-State { param([string]$Name, [scriptblock]$Draw)
  $pair = New-Base
  $base = $pair[0]; $g = $pair[1]
  & $Draw $g
  $g.Dispose()
  $bmpList = New-Object System.Collections.Generic.List[System.Drawing.Bitmap]
  foreach ($s in $sizes) { $bmpList.Add((New-Scaled -Base $base -Size $s)) }
  $out = Join-Path $OutDir $Name
  Write-Ico -Path $out -Bitmaps $bmpList.ToArray()
  $base.Dispose()
  foreach ($b in $bmpList) { $b.Dispose() }
  Write-Output ("wrote {0} ({1} bytes)" -f $out, (Get-Item $out).Length)
}

$gray = [System.Drawing.Color]::FromArgb(255, 158, 158, 158)
$green = [System.Drawing.Color]::FromArgb(255, 46, 125, 50)
$blue = [System.Drawing.Color]::FromArgb(255, 21, 101, 192)
$orange = [System.Drawing.Color]::FromArgb(255, 237, 108, 2)
$white = [System.Drawing.Color]::White

Save-State -Name 'tray_icon.ico' -Draw {
  param($g)
  $pen = New-Object System.Drawing.Pen($gray, 20)
  $g.DrawEllipse($pen, 24, 24, 80, 80)
  $pen.Dispose()
}

Save-State -Name 'tray_icon_core.ico' -Draw {
  param($g)
  $b = New-Object System.Drawing.SolidBrush($green)
  $g.FillEllipse($b, 14, 14, 100, 100)
  $b.Dispose()
  $wb = New-Object System.Drawing.SolidBrush($white)
  $pts = @(
    (New-Object System.Drawing.Point(54, 40)),
    (New-Object System.Drawing.Point(54, 88)),
    (New-Object System.Drawing.Point(92, 64))
  )
  $g.FillPolygon($wb, $pts)
  $wb.Dispose()
}

Save-State -Name 'tray_icon_proxy.ico' -Draw {
  param($g)
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $r = 22
  $path.AddArc(14, 14, $r, $r, 180, 90)
  $path.AddArc(114 - $r, 14, $r, $r, 270, 90)
  $path.AddArc(114 - $r, 114 - $r, $r, $r, 0, 90)
  $path.AddArc(14, 114 - $r, $r, $r, 90, 90)
  $path.CloseFigure()
  $b = New-Object System.Drawing.SolidBrush($blue)
  $g.FillPath($b, $path)
  $b.Dispose(); $path.Dispose()
  $wb = New-Object System.Drawing.SolidBrush($white)
  $up = @(
    (New-Object System.Drawing.Point(64, 30)),
    (New-Object System.Drawing.Point(44, 60)),
    (New-Object System.Drawing.Point(84, 60))
  )
  $g.FillPolygon($wb, $up)
  $stem = New-Object System.Drawing.Rectangle(58, 56, 12, 44)
  $g.FillRectangle($wb, $stem)
  $wb.Dispose()
}

Save-State -Name 'tray_icon_pac.ico' -Draw {
  param($g)
  $b = New-Object System.Drawing.SolidBrush($orange)
  $diamond = @(
    (New-Object System.Drawing.Point(64, 10)),
    (New-Object System.Drawing.Point(118, 64)),
    (New-Object System.Drawing.Point(64, 118)),
    (New-Object System.Drawing.Point(10, 64))
  )
  $g.FillPolygon($b, $diamond)
  $b.Dispose()
  $wb = New-Object System.Drawing.SolidBrush($white)
  $g.FillEllipse($wb, 48, 48, 32, 32)
  $wb.Dispose()
}
