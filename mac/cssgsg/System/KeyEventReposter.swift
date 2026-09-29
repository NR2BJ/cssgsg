// NRIME(github.com/NR2BJ/NRIME)의 KeyEventReposter를 가져왔다. 태그 값만 cssgsg 것으로 바꿨다.
import Cocoa
import InputMethodKit

/// 조합을 확정한 뒤 키를 앱에 다시 보내는 일.
///
/// - ⌘/Ctrl/Option+키: IMKit에서 false를 돌려줘도 앱에 전달되지 않는 경로(performKeyEquivalent)가 있어서,
///   확정한 뒤 태그를 단 CGEvent로 다시 보낸다. 컨트롤러는 태그를 보고 그 이벤트를 그대로 통과시킨다.
/// - Chromium에서 조합 중 Shift+Enter: 확정 뒤 잠깐 기다렸다 insertText("\n")를 넣는다(oldHasMarkedText 문제).
///   Codex처럼 프로그램 "\n"을 전송으로 받는 앱에는 Shift+Enter 키 누름을 다시 보낸다.
enum KeyEventReposter {

    /// 다시 보낸 이벤트 표시(eventSourceUserData). "CSSG"를 ASCII 16진수로.
    static let repostTag: Int64 = 0x4353_5347

    /// 키 이벤트를 보낼 권한(손쉬운 사용)이 있는지. 없으면 보낸 이벤트는 소리 없이 버려진다.
    /// ad-hoc 서명이면 macOS가 앱을 바이너리 해시로 기억해서, 새로 빌드할 때마다 권한이 풀린다.
    static var canPostEvents: Bool { CGPreflightPostEventAccess() }

    /// 태그를 단 키 누름(down+up)을 `delay` 뒤에 보낸다.
    ///
    /// 이벤트 소스는 nil이 아니라 `.hidSystemState`다. 소스가 없으면 HID 상태가 비어 Chromium이 실제 키 누름과
    /// 다르게 다룬다(NRIME 3월 실험).
    static func postKeyPress(keyCode: UInt16, flags: CGEventFlags, after delay: TimeInterval) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
            let source = CGEventSource(stateID: .hidSystemState)
            guard let keyDown = CGEvent(keyboardEventSource: source, virtualKey: keyCode, keyDown: true) else { return }
            keyDown.flags = flags
            keyDown.setIntegerValueField(.eventSourceUserData, value: repostTag)
            keyDown.post(tap: .cghidEventTap)

            DispatchQueue.main.asyncAfter(deadline: .now() + 0.02) {
                guard let keyUp = CGEvent(keyboardEventSource: source, virtualKey: keyCode, keyDown: false) else { return }
                keyUp.flags = flags
                keyUp.setIntegerValueField(.eventSourceUserData, value: repostTag)
                keyUp.post(tap: .cghidEventTap)
            }
        }
    }

    /// ⌘/Ctrl/Option+키를 그대로 다시 보낸다.
    static func repost(_ event: NSEvent, after delay: TimeInterval) {
        let flags = CGEventFlags(rawValue: UInt64(event.modifierFlags.intersection(.deviceIndependentFlagsMask).rawValue))
        postKeyPress(keyCode: event.keyCode, flags: flags, after: delay)
    }

    /// 다시 보낸 키는 renderer가 확정을 끝낸 뒤라야 평범한 Shift+Enter로 받는다. insertText보다 더 기다린다.
    /// 기본 0.12초(NRIME 실험값). 설정 파일 [mac] newline_replay_ms.
    static var replayDelay: TimeInterval = 0.12

    /// Chromium에서 확정 뒤의 Shift+Enter 줄바꿈.
    static func performChromiumNewline(keyCode: UInt16, client: any IMKTextInput, delay: TimeInterval) {
        if ChromiumDetector.frontmostAppTreatsNewlineInsertAsSubmit {
            // 권한이 없으면 다시 보낸 키가 버려지고, "\n"을 넣으면 이 앱은 전송해 버린다.
            // 확정만 하고 줄바꿈은 넣지 않는다(한 번 더 누르면 된다).
            guard canPostEvents else {
                DeveloperLogger.shared.log("Reposter", "newline skipped: no post-event access")
                return
            }
            postKeyPress(keyCode: keyCode, flags: .maskShift, after: max(delay, replayDelay))
        } else {
            scheduleNewlineInsert(into: client, after: delay)
        }
    }

    // MARK: - 대기 중인 줄바꿈

    /// 기다리는 동안 다음 키가 새 조합을 시작하면, NSNotFound 범위의 줄바꿈이 그 조합을 먹는다.
    /// 그래서 다음 keyDown 전에 먼저 넣는다(취소하면 줄바꿈이 사라진다).
    private struct PendingNewline {
        let client: any IMKTextInput
        let work: DispatchWorkItem
    }

    private static var pendingNewline: PendingNewline?

    private static func scheduleNewlineInsert(into client: any IMKTextInput, after delay: TimeInterval) {
        flushPendingNewline()
        let work = DispatchWorkItem {
            guard pendingNewline != nil else { return }
            pendingNewline = nil
            insertNewline(into: client)
        }
        pendingNewline = PendingNewline(client: client, work: work)
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: work)
    }

    /// 기다리는 줄바꿈이 있으면 지금 넣는다.
    static func flushPendingNewline() {
        guard let pending = pendingNewline else { return }
        pendingNewline = nil
        pending.work.cancel()
        insertNewline(into: pending.client)
    }

    private static func insertNewline(into client: any IMKTextInput) {
        client.insertText("\n" as NSString, replacementRange: NSRange(location: NSNotFound, length: 0))
    }
}
