// NRIME(github.com/NR2BJ/NRIME)의 KeyEventReposter를 가져왔다(1.0.12: 기다림 없이, 권한 확인 보강).
import Cocoa
import InputMethodKit

/// 조합을 확정한 뒤 키를 앱에 다시 보내는 일.
///
/// - ⌘/Ctrl/Option+키: IMKit에서 false를 돌려줘도 앱에 전달되지 않는 경로(performKeyEquivalent)가 있어서,
///   확정한 뒤 같은 키를 다시 보낸다. 다시 보낸 키는 입력기를 한 번 더 지나는데, 그때는 조합이 없어서 엔진이 앱으로 넘긴다.
///   (0.5.x는 eventSourceUserData 표시로 알아보려 했지만, 그 표시는 IMKit을 지나며 사라진다(NRIME 확인). 필요도 없다.)
/// - Chromium에서 조합 중 Shift+Enter: 확정 뒤 insertText("\n")로 줄을 바꾼다(oldHasMarkedText 문제).
///   Codex처럼 프로그램 "\n"을 전송으로 받는 앱에는 Shift+Enter 키 누름을 다시 보낸다.
///
/// 둘 다 지금 처리 중인 키가 끝난 뒤(다음 런루프 차례)에 하고, 그 밖의 기다림은 없다. Chromium은 handle() 안에서
/// 넣은 줄바꿈을 버린다. 0.5.x는 15ms(Codex 120ms)를 기다렸는데, 그 값은 macOS가 권한 없이 보낸 키를 소리 없이
/// 버리던 때 정한 것이었다. 권한이 있으면 기다리지 않아도 늘 됐다(NRIME 1.0.12: Discord 9/9, Codex 11/11).
/// 앱은 handle()의 답을 받은 뒤에 다음 키를 보내므로, 그사이에 친 키가 줄바꿈을 앞지르지도 않는다.
enum KeyEventReposter {

    /// 키 이벤트를 보낼 수 있는지. 없으면 macOS가 보낸 키를 소리 없이 버린다(확정만 되고 키는 안 간다).
    ///
    /// AXIsProcessTrusted로 본다. 켜고 끄는 것을 바로 따라가고(개발자 기록으로 확인), 손쉬운 사용 권한이 있으면 키도
    /// 보낼 수 있다. macOS 27은 그 권한("기기 제어 및 데이터 접근") 하나뿐이라 그것만 본다.
    /// CGPreflightPostEventAccess는 프로세스에서 처음 물었을 때의 답으로 굳는다. 권한 없이 뜨면 나중에 켜도 "없음"
    /// (NRIME: 허락하고 한 시간 뒤에도 거부), 권한을 켠 채 뜨면 나중에 꺼도 "있음"이다(0.6.0: 끄고도 "허용됨"으로 보였다,
    /// 확인 목록 43). 그래서 손쉬운 사용 없이 키 보내기만 따로 허락할 수 있는 macOS 26 이하에서만 더해 본다.
    static var canPostEvents: Bool {
        if AXIsProcessTrusted() { return true }
        if ProcessInfo.processInfo.operatingSystemVersion.majorVersion >= 27 { return false }
        return CGPreflightPostEventAccess()
    }

    /// ⌘/Ctrl/Option+키를 그대로 다시 보낸다. 부르는 쪽이 먼저 canPostEvents를 본다:
    /// 보낼 수 없으면 원래 키를 앱에 넘기는 편이 아무것도 안 하는 것보다 낫다.
    static func repost(_ event: NSEvent) {
        let flags = CGEventFlags(rawValue: UInt64(event.modifierFlags.intersection(.deviceIndependentFlagsMask).rawValue))
        postKeyPress(keyCode: event.keyCode, flags: flags)
    }

    /// Chromium에서 확정 뒤의 Shift+Enter 줄바꿈.
    static func performChromiumNewline(keyCode: UInt16, client: any IMKTextInput) {
        if ChromiumDetector.frontmostAppTreatsNewlineInsertAsSubmit {
            // 권한이 없으면 다시 보낸 키가 버려지고, "\n"을 넣으면 이 앱은 전송해 버린다.
            // 확정만 하고 줄바꿈은 넣지 않는다(한 번 더 누르면 된다).
            guard canPostEvents else {
                DeveloperLogger.shared.log("Reposter", "newline skipped: no post-event access")
                return
            }
            // Shift를 붙여 보낸다. 그냥 Enter면 전송된다.
            postKeyPress(keyCode: keyCode, flags: .maskShift)
        } else {
            DispatchQueue.main.async {
                client.insertText("\n" as NSString, replacementRange: NSRange(location: NSNotFound, length: 0))
            }
        }
    }

    /// 키 누름(down, up)을 다음 런루프 차례에 보낸다.
    ///
    /// 이벤트 소스는 nil이 아니라 `.hidSystemState`다. 소스가 없으면 HID 상태가 비어 Chromium이 실제 키 누름과
    /// 다르게 다룬다(NRIME 3월 실험).
    private static func postKeyPress(keyCode: UInt16, flags: CGEventFlags) {
        DispatchQueue.main.async {
            let source = CGEventSource(stateID: .hidSystemState)
            for isDown in [true, false] {
                guard let event = CGEvent(keyboardEventSource: source, virtualKey: keyCode, keyDown: isDown) else { continue }
                event.flags = flags
                event.post(tap: .cghidEventTap)
            }
        }
    }
}
