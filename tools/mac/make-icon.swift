// 입력 메뉴 아이콘(icon.tiff)을 그린다: 둥근 사각형 안에 "ㅊ". 16pt(1x, 2x) 템플릿 이미지.
// 사용: swift tools/mac/make-icon.swift mac/cssgsg/Resources/icon.tiff
import AppKit

let out = CommandLine.arguments[1]

func render(pixels: Int) -> NSBitmapImageRep {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8,
        samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: 16, height: 16)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let box = NSBezierPath(roundedRect: NSRect(x: 1.5, y: 1.5, width: 13, height: 13), xRadius: 3, yRadius: 3)
    box.lineWidth = 1.2
    NSColor.black.setStroke()
    box.stroke()
    let text = NSAttributedString(string: "ㅊ", attributes: [
        .font: NSFont.systemFont(ofSize: 10, weight: .semibold),
        .foregroundColor: NSColor.black,
    ])
    let size = text.size()
    text.draw(at: NSPoint(x: (16 - size.width) / 2, y: (16 - size.height) / 2 + 0.3))
    NSGraphicsContext.restoreGraphicsState()
    return rep
}

let data = NSBitmapImageRep.tiffRepresentationOfImageReps(in: [render(pixels: 16), render(pixels: 32)])!
try! data.write(to: URL(fileURLWithPath: out))
print("wrote \(out)")
