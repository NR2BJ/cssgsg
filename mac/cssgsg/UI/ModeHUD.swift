// NRIME(github.com/NR2BJ/NRIME)의 UI/InlineIndicator.swift를 가져와 줄였다(1.0.12-beta.11: 커서 찾기, 늘 보이기).
// 자리는 호출하는 쪽이 정한다(확정하기 전의 커서 자리. 확정한 뒤에 물으면 앱마다 어긋난다).
import Cocoa
import InputMethodKit

/// 모드를 바꿀 때 커서 근처에 G / ㅊ / 月(Graphite, 참신세벌식, 新月)을 잠깐 보이는 작은 창.
/// 마우스 클릭을 받지 않고 포커스도 가져가지 않는다.
/// 0.6.4는 입력칸이 활성화될 때도 지금 모드를 보였는데 0.6.5에서 뺐다(NRIME 1.0.12-beta.12): 입력칸이 아닌 창을 클릭해도
/// 입력기가 그 창에 활성화되고, 입력기는 그 둘을 가리지 못해서 꽤 자주 떴다. 도움보다 방해였다.
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

    /// `caret`이 있으면 그 위(화면 위쪽이 모자라면 아래), 없으면 마우스 옆에 보인다. 늘 보인다: 커서를 못 찾았다고
    /// 안 보이면 무슨 모드인지 모르는 채 치게 된다(NRIME 1.0.12-beta.11, 0.6.3까지는 커서 위 설정에서 못 찾으면 안 보였다).
    func show(_ text: String, caret: NSRect?) {
        let size = NSSize(width: text.count > 1 ? 36 : 26, height: 24)
        let origin: NSPoint
        if let caret {
            let above = caret.maxY + gap
            if let screen = TextInputGeometry.screenFrame(containing: caret), above + size.height > screen.maxY {
                origin = NSPoint(x: caret.minX + gap, y: caret.minY - size.height - gap)
            } else {
                origin = NSPoint(x: caret.minX + gap, y: above)
            }
        } else {
            let mouse = NSEvent.mouseLocation
            origin = NSPoint(x: mouse.x + gap, y: mouse.y + gap)
        }

        let panel = ensurePanel()
        panel.setContentSize(size)
        panel.contentView?.frame = NSRect(origin: .zero, size: size)
        if let label {
            label.stringValue = text
            // 글자 칸은 글자 높이만큼만 두고 가운데에 놓는다(칸을 상자 높이로 두면 글자가 위로 붙는다).
            let height = min(size.height, ceil(label.intrinsicContentSize.height))
            label.frame = NSRect(x: 0, y: ((size.height - height) / 2).rounded(), width: size.width, height: height)
        }
        DeveloperLogger.shared.log("HUD", "shown", metadata: [
            "caret": caret.map { String(format: "(%.0f,%.0f,%.0f,%.0f)", $0.minX, $0.minY, $0.width, $0.height) } ?? "-",
            "origin": String(format: "(%.0f,%.0f)", origin.x, origin.y),
        ])

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

    /// 모드 표시를 둘 커서 자리. 모르거나 미심쩍으면 nil(그러면 마우스 옆).
    ///
    /// 후보창과 같은 찾기(TextInputGeometry.caretRect: 앱이 알려 주는 커서 자리, 손쉬운 사용, 같은 입력칸에서 마지막으로 맞았던
    /// 자리)를 쓰되, 두 가지는 버린다(NRIME 1.0.12-beta.11).
    /// - 0번 글자 자리(attributesAtZero): 높이는 맞아도 줄 맨 앞이라 가로가 틀리다.
    /// - 그 앱의 창 밖: 다른 창의 글자, 낡은 자리, 화면 구석. 창 자리는 창 서버에서 얻어서 권한이 필요 없다.
    static func caretRect(for client: any IMKTextInput) -> NSRect? {
        guard let result = TextInputGeometry.caretRect(for: client),
              result.source != .attributesAtZero,
              TextInputGeometry.isUsableRect(result.rect) else { return nil }
        if let bundleID = client.bundleIdentifier(),
           let pid = NSRunningApplication.runningApplications(withBundleIdentifier: bundleID).first?.processIdentifier,
           !TextInputGeometry.caretIsInside(result.rect, windowFrames: TextInputGeometry.windowFrames(ofPID: pid)) {
            DeveloperLogger.shared.log("HUD", "caret outside the app's windows", metadata: ["source": "\(result.source)"])
            return nil
        }
        return result.rect
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
