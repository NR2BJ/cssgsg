# 맥 앱 아이콘(mac/Settings/Resources/AppIcon.icns)에서 윈도우 아이콘을 만든다 → win/settings/Assets/cssgsg.ico
#
#   powershell -ExecutionPolicy Bypass -File tools\win\make-icon.ps1
#
# - icns 안의 가장 큰 PNG(1024)를 크기마다 줄여 PNG로 담는다(윈도우 Vista부터 ICO 안의 PNG를 읽는다).
# - 만든 .ico는 저장소에 넣는다(빌드 때마다 만들지 않는다). 맥 아이콘을 바꾸면 이것을 다시 돌린다.
# - 이 파일은 UTF-8(BOM)이다.
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Icns = Join-Path $Root 'mac\Settings\Resources\AppIcon.icns'
$Out = Join-Path $Root 'win\settings\Assets\cssgsg.ico'
Add-Type -AssemblyName System.Drawing

# icns: 'icns' + 전체 길이, 그 뒤로 (종류 4바이트, 길이 4바이트 빅엔디언, 내용) 덩어리.
$bytes = [IO.File]::ReadAllBytes($Icns)
$best = $null
$i = 8
while ($i -lt $bytes.Length) {
    $len = ([int]$bytes[$i + 4] -shl 24) -bor ([int]$bytes[$i + 5] -shl 16) -bor ([int]$bytes[$i + 6] -shl 8) -bor [int]$bytes[$i + 7]
    $isPng = $bytes[$i + 8] -eq 0x89 -and $bytes[$i + 9] -eq 0x50
    if ($isPng -and ($null -eq $best -or $len -gt $best.Length)) {
        $best = New-Object byte[] ($len - 8)
        [Array]::Copy($bytes, $i + 8, $best, 0, $len - 8)
    }
    $i += $len
}
if ($null -eq $best) { throw "no PNG in $Icns" }
$source = [Drawing.Image]::FromStream((New-Object IO.MemoryStream (, $best)))

$sizes = 16, 20, 24, 32, 40, 48, 64, 256
$images = foreach ($size in $sizes) {
    $bitmap = New-Object Drawing.Bitmap $size, $size
    $g = [Drawing.Graphics]::FromImage($bitmap)
    $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.DrawImage($source, 0, 0, $size, $size)
    $g.Dispose()
    $png = New-Object IO.MemoryStream
    $bitmap.Save($png, [Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
    , $png.ToArray()
}

# ICO: 머리(6바이트) + 항목(16바이트 × n) + 그림들.
$ico = New-Object IO.MemoryStream
$w = New-Object IO.BinaryWriter $ico
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($n = 0; $n -lt $sizes.Count; $n++) {
    $s = $sizes[$n]
    $w.Write([byte]($(if ($s -ge 256) { 0 } else { $s })))
    $w.Write([byte]($(if ($s -ge 256) { 0 } else { $s })))
    $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([uint16]1); $w.Write([uint16]32)
    $w.Write([uint32]$images[$n].Length); $w.Write([uint32]$offset)
    $offset += $images[$n].Length
}
foreach ($image in $images) { $w.Write($image) }
$w.Flush()
New-Item -ItemType Directory -Force (Split-Path $Out) | Out-Null
[IO.File]::WriteAllBytes($Out, $ico.ToArray())
Write-Host ("{0} ({1:N0} bytes, {2})" -f $Out, $ico.Length, ($sizes -join ', '))
