import Cocoa
import InputMethodKit

/// IMKit 입력 컨트롤러. 키를 러스트 엔진에 넘기고, 엔진이 돌려준 결과(EngineOutput)대로 앱에 글자를 넣는다.
///
/// NRIME에서 옮긴 규칙
/// - 확정은 insertText만 쓴다. 그 앞뒤에 setMarkedText("")를 부르지 않는다(Chromium·JS 에디터에서 글자가 사라진다).
/// - 언어 전환(수식키)은 비밀번호 칸 판정보다 먼저 처리한다. 막히면 사용자가 빠져나올 길이 없다.
/// - ⌘/Ctrl/Option+키가 조합을 확정했으면, 확정하고 조금 기다린 뒤 그 키를 CGEvent로 다시 보낸다.
/// - 조합 중 Shift+Enter는 Chromium이면 확정하고 조금 기다린 뒤 줄바꿈을 직접 넣는다(KeyEventReposter).
/// - 입력기 활성화 때는 확정하지 않고 버린다("사과" → "사과과" 중복 방지). 비활성화 때는 sender에 확정한다.
/// - 전역 마우스 클릭 감시로 조합을 확정한다(포커스 이동 때 commitComposition을 안 부르는 앱이 있다).
@objc(CssgsgInputController)
final class CssgsgInputController: IMKInputController {

    private static weak var activeController: CssgsgInputController?
    private static var mouseMonitor: Any?
    /// 수식키 상태(좌우, 눌림). 컨트롤러는 클라이언트마다 따로 생기므로 프로세스 전체에서 하나를 같이 쓴다.
    private static var modifiers = ModifierState()

    private let secureInput = SecureInputDetector()
    /// 마우스 감시 콜백 때는 self.client()가 이미 nil일 수 있어서 마지막 클라이언트를 들고 있는다.
    private var cachedClient: (any IMKTextInput)?
    private var lastSecureState = false

    override func recognizedEvents(_ sender: Any!) -> Int {
        Int(NSEvent.EventTypeMask([.keyDown, .flagsChanged]).rawValue)
    }

    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event, let client = sender as? (any IMKTextInput) else { return false }
        // 우리가 다시 보낸 키(KeyEventReposter)도 여기로 온다. 그때는 조합이 없어서 엔진이 앱으로 넘긴다.
        cachedClient = client

        // 기다리던 줄바꿈은 이 키보다 먼저 들어가야 한다. 늦게 넣으면 이 키가 시작한 조합을 먹는다(NSNotFound 범위).
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
            // 바뀐 것이 없으면(IMKit이 같은 flagsChanged를 두 번 보낸다) 엔진에 넘기지 않는다.
            guard let keyEvent = Self.modifiers.flagsChanged(event) else { return false }
            let out = CoreEngine.shared.handle(keyEvent, secureField: secure)
            logKey(event, keyEvent, out)
            // 전환은 되지만, 확정할 글자를 비밀번호 칸에 넣지는 않는다.
            apply(out, client: client, allowCommit: !secure)
            return false

        case .keyDown:
            // Codex에 다시 보낼 Shift+Enter가 아직 기다리는 중: 이 키는 그 줄바꿈 뒤에 친 것이다. 잡아 뒀다가 줄바꿈 다음에
            // 보낸다(돌아오면 평범한 키로 여기를 다시 지난다). 엔진은 이 키를 못 보니 탭 무효만 알린다: Shift를 누른 채
            // 친 대문자라면 Shift는 탭이 아니다(알리지 않으면 언어가 바뀐다).
            if !secure, KeyEventReposter.holdForPendingReplay(event, client: client as AnyObject) {
                CoreEngine.shared.cancelTap()
                DeveloperLogger.shared.log("Controller", "key held for the newline replay")
                return true
            }
            let keyEvent = Self.modifiers.keyDown(event)
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

        // ⌘/Ctrl/Option+키가 조합을 확정했다: performKeyEquivalent 경로라 false로는 앱에 안 가는 경우가 있다.
        if !out.consumed && commandLike && !out.commit.isEmpty {
            apply(out, client: client)
            // 권한이 없으면 다시 보낸 키가 버려진다. 그럴 바엔 원래 키를 앱에 넘긴다.
            guard KeyEventReposter.canPostEvents else { return false }
            KeyEventReposter.repost(event)
            return true
        }

        // 조합 중 Shift+Enter: Chromium은 확정과 같은 키로 줄을 바꾸면 확정한 글자를 잃는다.
        if !out.consumed && isEnter && event.modifierFlags.contains(.shift) && !out.commit.isEmpty {
            apply(out, client: client)
            if ChromiumDetector.isFrontmostAppChromium {
                KeyEventReposter.performChromiumNewline(keyCode: event.keyCode, client: client)
                return true
            }
            return false
        }

        apply(out, client: client)
        return out.consumed
    }

    private func apply(_ out: EngineOutput, client: any IMKTextInput, allowCommit: Bool = true) {
        // 모드가 바뀌면 HUD를 띄운다. 자리는 확정하기 전에 잰다(확정한 뒤에는 앱마다 커서 자리가 어긋난다).
        let settings = CoreEngine.shared.macSettings
        let hudCaret: NSRect? = out.mode != nil && settings.hud != 0 && settings.hud_at_mouse == 0
            ? ModeHUD.caretRect(for: client) : nil
        if !out.commit.isEmpty && !allowCommit {
            DeveloperLogger.shared.log("Controller", "commit dropped in secure field", metadata: ["length": "\(out.commit.count)"])
        }
        TextApplier.apply(out, to: IMKTextClient(client: client), allowCommit: allowCommit)
        applyUI(out, client: client, hudCaret: hudCaret)
        scheduleTimer(out.timerMs)
    }

    /// 빠른 탭 전환 보정이 글자를 잡아 두었다. 수식키를 떼지 않고 시간이 지나면 엔진이 그 글자를 누른 그대로 친다.
    /// 엔진은 늦게 온 타이머를 스스로 걸러 내므로(아직이면 남은 시간을 다시 청한다) 여러 번 걸어도 된다.
    /// 지금 실제로 누르고 있는 수식키를 같이 준다: 이미 뗐으면 엔진이 늦게 오는 뗌 이벤트를 기다린다(바쁜 앱).
    private func scheduleTimer(_ ms: UInt32) {
        guard ms > 0 else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(Int(ms))) { [weak self] in
            let out = CoreEngine.shared.timer(now: ProcessInfo.processInfo.systemUptime, held: ModifierState.physicalMods())
            guard let self, let client = self.cachedClient ?? self.client() else { return }
            self.apply(out, client: client, allowCommit: self.canCommit(to: client))
        }
    }

    /// 글자 밖의 것: 후보창, 모드 표시와 HUD, Caps Lock, 한자 학습 저장.
    private func applyUI(_ out: EngineOutput, client: any IMKTextInput, hudCaret: NSRect? = nil) {
        let settings = CoreEngine.shared.macSettings
        switch out.candidates {
        case .unchanged:
            break
        case .hide:
            NSApp.candidatePanel?.hide()
        case let .show(items, notes, selected, grid):
            NSApp.candidatePanel?.show(candidates: items, notes: notes, selectedIndex: selected ?? 0, grid: grid, client: client)
        }
        if let mode = out.mode {
            (NSApp.delegate as? AppDelegate)?.updateStatus(mode)
            if settings.hud != 0 {
                ModeHUD.shared.show(mode.label, caret: hudCaret)
            }
        }
        if out.capsLockOff {
            CapsLock.set(false)
        }
        if out.learningChanged {
            HanjaLearningStore.shared.scheduleSave()
        }
    }

    // MARK: - 활성화 / 비활성화

    override func activateServer(_ sender: Any!) {
        super.activateServer(sender)
        Self.activeController = self
        PermissionMonitor.refreshIfStale()
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
            "raw": String(format: "0x%X", event.modifierFlags.rawValue),
            "t": String(format: "%.3f", key.time),
            "consumed": "\(out.consumed)",
            "commitLen": "\(out.commit.count)",
            "preeditLen": "\(out.preedit?.text.count ?? -1)",
            "mode": out.mode.map { "\($0.rawValue)" } ?? "-",
        ])
    }
}

/// IMK 클라이언트를 TextApplier가 쓰는 모양으로 감싼다.
struct IMKTextClient: TextClient {
    let client: any IMKTextInput

    func insertText(_ text: String, replacementRange: NSRange) {
        client.insertText(text as NSString, replacementRange: replacementRange)
    }

    func setMarkedText(_ text: Any, selectionRange: NSRange, replacementRange: NSRange) {
        client.setMarkedText(text, selectionRange: selectionRange, replacementRange: replacementRange)
    }

}
