// NRIME(github.com/NR2BJ/NRIME)의 UI/InlineIndicator.swift를 가져와 줄였다.
// 자리는 호출하는 쪽이 정한다(확정하기 전의 커서 자리. 확정한 뒤에 물으면 앱마다 어긋난다).
import Cocoa

/// 모드를 바꿀 때 커서 근처에 A / 한 / あ를 잠깐 보이는 작은 창. 마우스 클릭을 받지 않고 포커스도 가져가지 않는다.
final class ModeHUD {
    static let shared = ModeHUD()

    private var panel: NSPanel?
    private var label: NSTextField?
    private var fadeTimer: Timer?
    /// 보일 때마다 올린다. 앞선 표시의 사라짐이 지금 표시를 치우지 않게 한다.
    private var generation = 0
    private let displayDuration: TimeInterval = 1.0
    private let fadeDuration: TimeInterval = 0.3
    private let gap: CGFloat = 4

    private init() {}

    /// `caret`이 있으면 그 위(화면 위쪽이 모자라면 아래), 없으면 마우스 옆에 보인다.
    /// 커서 자리를 물었는데 못 찾은 경우(`caret`이 없고 `fallbackToMouse`가 false)에는 보이지 않는다.
    func show(_ text: String, caret: NSRect?, fallbackToMouse: Bool) {
        let size = NSSize(width: text.count > 1 ? 36 : 26, height: 24)
        let origin: NSPoint
        if let caret {
            let above = caret.maxY + gap
            if let screen = TextInputGeometry.screenFrame(containing: caret), above + size.height > screen.maxY {
                origin = NSPoint(x: caret.minX + gap, y: caret.minY - size.height - gap)
            } else {
                origin = NSPoint(x: caret.minX + gap, y: above)
            }
        } else if fallbackToMouse {
            let mouse = NSEvent.mouseLocation
            origin = NSPoint(x: mouse.x + gap, y: mouse.y + gap)
        } else {
            return
        }

        let panel = ensurePanel()
        label?.stringValue = text
        panel.setContentSize(size)
        panel.contentView?.frame = NSRect(origin: .zero, size: size)
        label?.frame = NSRect(origin: .zero, size: size)

        fadeTimer?.invalidate()
        generation &+= 1
        let current = generation
        panel.setFrameOrigin(origin)
        panel.alphaValue = 1
        panel.orderFrontRegardless()

        fadeTimer = Timer.scheduledTimer(withTimeInterval: displayDuration, repeats: false) { [weak self] _ in
            guard let self, current == self.generation else { return }
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = self.fadeDuration
                self.panel?.animator().alphaValue = 0
            }, completionHandler: { [weak self] in
                guard let self, current == self.generation else { return }
                self.panel?.orderOut(nil)
            })
        }
    }

    /// 한 번 만들어 계속 쓴다(macOS 26부터 창을 버려도 메모리가 돌아오지 않는다, CONCEPT §9).
    private func ensurePanel() -> NSPanel {
        if let panel { return panel }
        let size = NSSize(width: 26, height: 24)
        let p = NSPanel(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        p.level = .floating
        p.ignoresMouseEvents = true
        p.isOpaque = false
        p.backgroundColor = .clear
        p.hasShadow = true
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]

        let background = NSView(frame: NSRect(origin: .zero, size: size))
        background.wantsLayer = true
        background.layer?.backgroundColor = NSColor(white: 0.15, alpha: 0.85).cgColor
        background.layer?.cornerRadius = 6

        let text = NSTextField(labelWithString: "")
        text.font = NSFont.systemFont(ofSize: 14, weight: .medium)
        text.textColor = .white
        text.alignment = .center
        text.frame = NSRect(origin: .zero, size: size)
        background.addSubview(text)

        p.contentView = background
        panel = p
        label = text
        return p
    }
}
