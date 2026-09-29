import Cocoa
import InputMethodKit

/// IMKit 입력 컨트롤러. 키를 러스트 엔진에 넘기고, 엔진이 돌려준 결과(EngineOutput)대로 앱에 글자를 넣는다.
///
/// NRIME에서 옮긴 규칙
/// - 확정은 insertText만 쓴다. 그 앞뒤에 setMarkedText("")를 부르지 않는다(Chromium·JS 에디터에서 글자가 사라진다).
/// - 언어 전환(수식키)은 비밀번호 칸 판정보다 먼저 처리한다. 막히면 사용자가 빠져나올 길이 없다.
/// - ⌘/Ctrl/Option+키가 조합을 확정했으면, 확정 뒤 그 키를 태그 달린 CGEvent로 다시 보낸다.
/// - 조합 중 Shift+Enter는 Chromium이면 확정 뒤 잠깐 기다렸다 줄바꿈을 직접 넣는다.
/// - 입력기 활성화 때는 확정하지 않고 버린다("사과" → "사과과" 중복 방지). 비활성화 때는 sender에 확정한다.
/// - 전역 마우스 클릭 감시로 조합을 확정한다(포커스 이동 때 commitComposition을 안 부르는 앱이 있다).
@objc(CssgsgInputController)
final class CssgsgInputController: IMKInputController {

    private static weak var activeController: CssgsgInputController?
    private static var mouseMonitor: Any?
    /// 수식키 눌림/뗌은 앞 이벤트와 비교해야 안다. 프로세스 전체에서 하나.
    private static var lastModifierFlags: NSEvent.ModifierFlags = []
    /// ⌘+키 재전송과 Shift+Enter 줄바꿈 전에 기다리는 시간(NRIME 기본 15ms).
    static var shiftEnterDelay: TimeInterval = 0.015

    private let secureInput = SecureInputDetector()
    /// 마우스 감시 콜백 때는 self.client()가 이미 nil일 수 있어서 마지막 클라이언트를 들고 있는다.
    private var cachedClient: (any IMKTextInput)?
    private var lastSecureState = false

    override func recognizedEvents(_ sender: Any!) -> Int {
        Int(NSEvent.EventTypeMask([.keyDown, .flagsChanged]).rawValue)
    }

    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event, let client = sender as? (any IMKTextInput) else { return false }

        // 우리가 다시 보낸 키는 그대로 앱으로.
        if let cg = event.cgEvent, cg.getIntegerValueField(.eventSourceUserData) == KeyEventReposter.repostTag {
            return false
        }
        cachedClient = client

        // 기다리던 줄바꿈은 이 키보다 먼저 들어가야 한다.
        if event.type == .keyDown {
            KeyEventReposter.flushPendingNewline()
        }

        let secure = secureInput.shouldSuppressComposition() || secureInput.isAuthenticationClient(client.bundleIdentifier())
        if secure != lastSecureState {
            lastSecureState = secure
            DeveloperLogger.shared.log("Controller", "secure field", metadata: [
                "secure": "\(secure)",
                "holder": secureInput.secureInputHolderBundleID() ?? "unknown",
                "app": client.bundleIdentifier() ?? "unknown",
            ])
        }

        switch event.type {
        case .flagsChanged:
            let previous = Self.lastModifierFlags
            Self.lastModifierFlags = event.modifierFlags
            guard let keyEvent = KeyTranslation.flagsChanged(event, previous: previous) else { return false }
            let out = CoreEngine.shared.handle(keyEvent, secureField: secure)
            logKey(event, keyEvent, out)
            // 전환은 되지만, 확정할 글자를 비밀번호 칸에 넣지는 않는다.
            apply(out, client: client, allowCommit: !secure)
            return false

        case .keyDown:
            let keyEvent = KeyTranslation.keyDown(event)
            let out = CoreEngine.shared.handle(keyEvent, secureField: secure)
            logKey(event, keyEvent, out)
            if secure {
                return false
            }
            return finish(out, event: event, client: client)

        default:
            return false
        }
    }

    /// keyDown 결과를 앱에 반영하고 키를 먹을지 정한다.
    private func finish(_ out: EngineOutput, event: NSEvent, client: any IMKTextInput) -> Bool {
        let commandLike = !event.modifierFlags.intersection([.command, .control, .option]).isEmpty
        let isEnter = event.keyCode == 0x24 || event.keyCode == 0x4C

        // ⌘/Ctrl/Option+키가 조합을 확정했다: performKeyEquivalent 경로라 false로는 앱에 안 간다.
        if !out.consumed && commandLike && !out.commit.isEmpty {
            apply(out, client: client)
            KeyEventReposter.repost(event, after: Self.shiftEnterDelay)
            return true
        }

        // 조합 중 Shift+Enter: Chromium은 확정과 같은 키로 줄을 바꾸면 확정한 글자를 잃는다.
        if !out.consumed && isEnter && event.modifierFlags.contains(.shift) && !out.commit.isEmpty {
            apply(out, client: client)
            if ChromiumDetector.isFrontmostAppChromium {
                KeyEventReposter.performChromiumNewline(keyCode: event.keyCode, client: client, delay: Self.shiftEnterDelay)
                return true
            }
            return false
        }

        apply(out, client: client)
        return out.consumed
    }

    private func apply(_ out: EngineOutput, client: any IMKTextInput, allowCommit: Bool = true) {
        let replacement = NSRange(location: NSNotFound, length: NSNotFound)
        let committed = !out.commit.isEmpty && allowCommit
        if committed {
            client.insertText(out.commit as NSString, replacementRange: replacement)
        } else if !out.commit.isEmpty {
            DeveloperLogger.shared.log("Controller", "commit dropped in secure field", metadata: ["length": "\(out.commit.count)"])
        }
        if let preedit = out.preedit {
            if !preedit.text.isEmpty {
                client.setMarkedText(
                    Self.marked(preedit),
                    selectionRange: NSRange(location: preedit.caret, length: 0),
                    replacementRange: replacement
                )
            } else if !committed {
                // 확정 없이 조합이 사라졌을 때만 지운다. insertText가 이미 조합 글자를 대신했으면 부르지 않는다.
                client.setMarkedText(
                    "" as NSString,
                    selectionRange: NSRange(location: 0, length: 0),
                    replacementRange: replacement
                )
            }
        }
        switch out.candidates {
        case .unchanged:
            break
        case .hide:
            NSApp.candidatePanel?.hide()
        case let .show(items, selected):
            NSApp.candidatePanel?.show(candidates: items, selectedIndex: selected ?? 0, client: client)
        }
        if let mode = out.mode {
            (NSApp.delegate as? AppDelegate)?.updateStatus(mode)
        }
        if out.capsLockOff {
            CapsLock.set(false)
        }
    }

    /// 조합 중 글자에 밑줄. 변환 중이면 문절마다 나누고 포커스된 문절은 굵게.
    private static func marked(_ p: PreeditUpdate) -> NSAttributedString {
        let s = NSMutableAttributedString(string: p.text)
        let full = NSRange(location: 0, length: s.length)
        if p.segments.isEmpty {
            s.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: full)
        }
        for (i, seg) in p.segments.enumerated() where NSMaxRange(seg.range) <= s.length {
            let style: NSUnderlineStyle = seg.focused ? .thick : .single
            s.addAttributes([.underlineStyle: style.rawValue, .markedClauseSegment: i], range: seg.range)
        }
        return s
    }

    // MARK: - 활성화 / 비활성화

    override func activateServer(_ sender: Any!) {
        super.activateServer(sender)
        Self.activeController = self
        if Self.mouseMonitor == nil {
            Self.mouseMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { _ in
                Self.activeController?.commitOnMouseClick()
            }
        }
        // 앞 세션의 조합은 확정하지 않고 버린다. 앞 클라이언트는 이미 사라졌을 수 있고,
        // 새 클라이언트에 넣으면 Electron에서 글자가 겹친다.
        _ = CoreEngine.shared.reset()
        (NSApp.delegate as? AppDelegate)?.updateStatus(CoreEngine.shared.mode)
        let client = sender as? (any IMKTextInput)
        DeveloperLogger.shared.log("Controller", "activateServer", metadata: ["app": client?.bundleIdentifier() ?? "unknown"])
    }

    override func deactivateServer(_ sender: Any!) {
        // self.client()는 이미 nil일 수 있으니 sender에 확정한다.
        commit(into: sender)
        NSApp.candidatePanel?.hide()
        DeveloperLogger.shared.log("Controller", "deactivateServer")
        super.deactivateServer(sender)
    }

    override func commitComposition(_ sender: Any!) {
        commit(into: sender)
        super.commitComposition(sender)
    }

    private func commit(into sender: Any?) {
        guard let client = (sender as? (any IMKTextInput)) ?? self.client() else { return }
        let out = CoreEngine.shared.commitAll()
        apply(out, client: client, allowCommit: canCommit(to: client))
    }

    private func commitOnMouseClick() {
        guard let client = cachedClient ?? self.client() else { return }
        let out = CoreEngine.shared.mouseDown()
        apply(out, client: client, allowCommit: canCommit(to: client))
    }

    /// 인증 창과 비밀번호 칸에는 확정 글자를 넣지 않는다.
    private func canCommit(to client: any IMKTextInput) -> Bool {
        !secureInput.isAuthenticationClient(client.bundleIdentifier()) && !secureInput.shouldSuppressComposition()
    }

    // MARK: - 기록

    /// 키 코드·수식키·결과 모양만 남긴다. 글자 내용은 남기지 않는다.
    private func logKey(_ event: NSEvent, _ key: CssgsgKeyEvent, _ out: EngineOutput) {
        guard DeveloperLogger.shared.isEnabled else { return }
        DeveloperLogger.shared.log("Key", event.type == .flagsChanged ? "flags" : "down", metadata: [
            "keyCode": String(format: "0x%02X", event.keyCode),
            "hid": String(format: "0x%02X", key.key),
            "down": "\(key.down)",
            "mods": String(format: "0x%03X", key.mods),
            "t": String(format: "%.3f", key.time),
            "consumed": "\(out.consumed)",
            "commitLen": "\(out.commit.count)",
            "preeditLen": "\(out.preedit?.text.count ?? -1)",
            "mode": out.mode.map { "\($0.rawValue)" } ?? "-",
        ])
    }
}
