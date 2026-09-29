// 설정 앱 아이콘(AppIcon.icns)을 그린다: 둥근 사각형에 세 배열의 머리글자 ㅊ · G · 月.
// 사용: swift tools/mac/make-settings-icon.swift mac/Settings/Resources/AppIcon.icns
import AppKit

let out = CommandLine.arguments[1]

func render(_ pixels: Int) -> Data {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8,
        samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: 1024, height: 1024)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    // macOS 앱 아이콘 격자: 1024 안에 824 몸통, 모서리 185.
    let body = NSBezierPath(roundedRect: NSRect(x: 100, y: 100, width: 824, height: 824), xRadius: 185, yRadius: 185)
    NSGradient(colors: [
        NSColor(calibratedRed: 0.20, green: 0.33, blue: 0.82, alpha: 1),
        NSColor(calibratedRed: 0.06, green: 0.58, blue: 0.54, alpha: 1),
    ])!.draw(in: body, angle: -90)
    func draw(_ text: String, size: CGFloat, weight: NSFont.Weight, centerX: CGFloat, centerY: CGFloat, alpha: CGFloat = 1) {
        let s = NSAttributedString(string: text, attributes: [
            .font: NSFont.systemFont(ofSize: size, weight: weight),
            .foregroundColor: NSColor.white.withAlphaComponent(alpha),
        ])
        let b = s.size()
        s.draw(at: NSPoint(x: centerX - b.width / 2, y: centerY - b.height / 2))
    }
    draw("ㅊ", size: 430, weight: .bold, centerX: 512, centerY: 575)
    draw("G", size: 170, weight: .semibold, centerX: 372, centerY: 250, alpha: 0.9)
    draw("月", size: 170, weight: .semibold, centerX: 652, centerY: 250, alpha: 0.9)
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

let fm = FileManager.default
let iconset = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("cssgsg-settings.iconset")
try? fm.removeItem(at: iconset)
try! fm.createDirectory(at: iconset, withIntermediateDirectories: true)
for base in [16, 32, 128, 256, 512] {
    try! render(base).write(to: iconset.appendingPathComponent("icon_\(base)x\(base).png"))
    try! render(base * 2).write(to: iconset.appendingPathComponent("icon_\(base)x\(base)@2x.png"))
}
let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", "-o", out, iconset.path]
try! iconutil.run()
iconutil.waitUntilExit()
try? fm.removeItem(at: iconset)
print(iconutil.terminationStatus == 0 ? "wrote \(out)" : "iconutil failed")
