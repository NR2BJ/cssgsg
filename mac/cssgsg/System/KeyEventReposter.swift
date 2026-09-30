// NRIME(github.com/NR2BJ/NRIME)의 KeyEventReposter를 가져왔다(1.0.12-beta.9: 대기와 순서 보호, 권한 확인 보강).
import Cocoa
import InputMethodKit

/// 조합을 확정한 뒤 키를 앱에 다시 보내는 일.
///
/// - ⌘/Ctrl/Option+키: IMKit에서 false를 돌려줘도 앱에 전달되지 않는 경로(performKeyEquivalent)가 있어서,
///   확정하고 조금 기다린 뒤 같은 키를 다시 보낸다. 다시 보낸 키는 입력기를 한 번 더 지나는데, 그때는 조합이 없어서
///   엔진이 앱으로 넘긴다. (0.5.x는 eventSourceUserData 표시로 알아보려 했지만, 그 표시는 IMKit을 지나며 사라진다
///   (NRIME 확인). 필요도 없다.)
/// - Chromium에서 조합 중 Shift+Enter: 확정하고 조금 기다린 뒤 insertText("\n")로 줄을 바꾼다(oldHasMarkedText 문제).
///   Codex처럼 줄바꿈 글자를 넣으면 메시지를 보내 버리는 앱에는 Shift+Enter 키 누름을 다시 보낸다.
///
/// 기다리는 시간은 설정이다: 줄바꿈 넣기와 ⌘ 다시 보내기는 [mac] newline_insert_wait_ms(기본 20ms), Shift+Enter
/// 다시 보내기는 newline_key_press_wait_ms(기본 50ms). 0이면 지금 처리 중인 키가 끝난 뒤(다음 런루프 차례)에 한다.
/// Chromium은 handle() 안에서 넣은 줄바꿈을 버린다.
/// 0.6.0·0.6.1은 기다리지 않았다(NRIME 1.0.12-beta.5: 빠른 맥에서는 Discord·Codex가 대기 없이 됐다). 그런데 느린 맥
/// (M2 맥북)에서 조합 중이던 글자가 사라졌다. Claude는 매번, Codex는 가끔. 줄바꿈이나 다시 보낸 키가 앱의 편집기가
/// 확정을 다 받기 전에 닿았다. 0.5.x의 15ms로는 Claude에서 몇 달 동안 한 번도 안 사라졌다(NRIME 1.0.12-beta.8).
/// 알맞은 값은 맥마다 달라서 설정 앱 일반 탭에서 고른다(NRIME beta.9).
/// 기다리는 동안 친 키가 줄바꿈을 앞지르지 않게 순서도 지킨다(대기 중인 줄바꿈, 다시 보낼 Shift+Enter).
enum KeyEventReposter {

    /// 줄바꿈 글자를 넣기 전과 ⌘ 단축키를 다시 보내기 전의 대기(설정 [mac] newline_insert_wait_ms).
    static var insertWait: TimeInterval = 0.02
    /// Shift+Enter 키를 다시 보내기 전의 대기(설정 [mac] newline_key_press_wait_ms).
    static var keyPressWait: TimeInterval = 0.05

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

    /// ⌘/Ctrl/Option+키를 insertWait 뒤에 그대로 다시 보낸다. 부르는 쪽이 먼저 canPostEvents를 본다:
    /// 보낼 수 없으면 원래 키를 앱에 넘기는 편이 아무것도 안 하는 것보다 낫다.
    static func repost(_ event: NSEvent) {
        let flags = CGEventFlags(rawValue: UInt64(event.modifierFlags.intersection(.deviceIndependentFlagsMask).rawValue))
        let keyCode = event.keyCode
        DispatchQueue.main.asyncAfter(deadline: .now() + insertWait) {
            postKeySequence([(keyCode, flags)])
        }
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
            scheduleReplay(keyCode: keyCode, client: client, after: keyPressWait)
        } else {
            scheduleNewlineInsert(into: client, after: insertWait)
        }
    }

    /// 키 누름(down, up)을 차례대로 보낸다.
    ///
    /// 이벤트 소스는 nil이 아니라 `.hidSystemState`다. 소스가 없으면 HID 상태가 비어 Chromium이 실제 키 누름과
    /// 다르게 다룬다(NRIME 3월 실험).
    private static func postKeySequence(_ keys: [(keyCode: UInt16, flags: CGEventFlags)]) {
        guard !keys.isEmpty else { return }
        let source = CGEventSource(stateID: .hidSystemState)
        for key in keys {
            for isDown in [true, false] {
                guard let event = CGEvent(keyboardEventSource: source, virtualKey: key.keyCode, keyDown: isDown) else {
                    continue
                }
                event.flags = key.flags
                event.post(tap: .cghidEventTap)
            }
        }
    }

    // MARK: - 다시 보낼 Shift+Enter (Codex)

    /// 기다리고 있는 Shift+Enter 다시 보내기와, 그동안 친 키.
    ///
    /// 그 키들은 처리하지 않고 잡아 둔다. 다시 보낸 Shift+Enter는 보낸 이벤트라서, 입력기가 그사이에 직접 넣는 글자는 모두
    /// 그보다 먼저 들어간다: Shift+Enter 바로 뒤에 이어 친 낱말이 줄바꿈 위로 갔다(NRIME). 다시 보낼 때 잡아 둔 키를
    /// 그 뒤에 차례대로 보내면, 입력기를 평범한 키로 다시 지난다.
    private struct PendingReplay {
        let keyCode: UInt16
        let clientID: ObjectIdentifier
        let bundleID: String?
        let scheduledAt: TimeInterval
        var held: [(keyCode: UInt16, flags: CGEventFlags)] = []
        var work: DispatchWorkItem?
    }

    /// 메인 스레드에서만: handle()에서 걸고 메인 큐에서 보낸다.
    private static var pendingReplay: PendingReplay?

    private static func scheduleReplay(keyCode: UInt16, client: any IMKTextInput, after wait: TimeInterval) {
        // 앞서 기다리던 것이 있으면 이것보다 먼저다.
        firePendingReplay(reason: "superseded")
        var replay = PendingReplay(keyCode: keyCode, clientID: ObjectIdentifier(client as AnyObject),
                                   bundleID: NSWorkspace.shared.frontmostApplication?.bundleIdentifier,
                                   scheduledAt: ProcessInfo.processInfo.systemUptime)
        let work = DispatchWorkItem { firePendingReplay(reason: "timer") }
        replay.work = work
        pendingReplay = replay
        DispatchQueue.main.asyncAfter(deadline: .now() + wait, execute: work)
    }

    /// 다시 보낼 Shift+Enter가 기다리는 동안 친 키를 잡아 둔다(줄바꿈 다음에 보낸다). 기다리는 것이 없으면 false:
    /// 평소처럼 처리한다. 다른 입력칸의 키면 기다리던 것을 지금 보낸다(그 키는 그 줄바꿈 뒤에 친 것이 아니다).
    /// 잡은 키는 엔진이 보지 못한다. 부르는 쪽이 엔진에 탭 무효를 알린다(Shift를 누른 채 친 키면 Shift는 탭이 아니다).
    static func holdForPendingReplay(_ event: NSEvent, client: AnyObject) -> Bool {
        guard event.type == .keyDown, var replay = pendingReplay else { return false }
        guard replay.clientID == ObjectIdentifier(client) else {
            firePendingReplay(reason: "otherClient")
            return false
        }
        let flags = event.cgEvent?.flags
            ?? CGEventFlags(rawValue: UInt64(event.modifierFlags.intersection(.deviceIndependentFlagsMask).rawValue))
        replay.held.append((event.keyCode, flags))
        pendingReplay = replay
        return true
    }

    /// 기다리던 Shift+Enter를 지금 보내고 잡아 둔 키를 그 뒤에 보낸다. 그 앱이 더는 앞에 없으면 줄바꿈은 빼고
    /// (다른 앱에 들어간다) 잡아 둔 키만 보낸다. 친 것은 잃지 않는다. 기다리는 것이 없으면 아무것도 안 한다.
    static func firePendingReplay(reason: String) {
        guard let replay = pendingReplay else { return }
        pendingReplay = nil
        replay.work?.cancel()
        let sameApp = NSWorkspace.shared.frontmostApplication?.bundleIdentifier == replay.bundleID
        var keys: [(keyCode: UInt16, flags: CGEventFlags)] = []
        if sameApp {
            // Shift를 붙여 보낸다. 그냥 Enter면 전송된다.
            keys.append((replay.keyCode, .maskShift))
        }
        keys += replay.held
        postKeySequence(keys)
        DeveloperLogger.shared.log("Reposter", "newline replayed as a key press", metadata: [
            "waitedMs": String(format: "%.0f", (ProcessInfo.processInfo.systemUptime - replay.scheduledAt) * 1000),
            "reason": reason,
            "held": "\(replay.held.count)",
            "sent": sameApp ? "Y" : "N",
        ])
    }

    // MARK: - 넣을 줄바꿈 (그 밖의 Chromium 앱)

    /// 기다리고 있는 줄바꿈 넣기.
    ///
    /// NSNotFound 범위로 넣어서, 넣을 때 조합 중인 글자가 있으면 그것을 바꿔 버린다. 기다리는 동안 다음 낱말을 치기
    /// 시작하면 줄바꿈이 그 조합을 먹는다. 그래서 다음 keyDown을 처리하기 전에 먼저 넣는다(flushPendingNewline).
    private struct PendingNewline {
        let client: any IMKTextInput
        let work: DispatchWorkItem
        let scheduledAt: TimeInterval
    }

    /// 메인 스레드에서만: handle()에서 걸고 메인 큐에서 넣는다.
    private static var pendingNewline: PendingNewline?

    private static func scheduleNewlineInsert(into client: any IMKTextInput, after wait: TimeInterval) {
        // 기다리는 것은 하나뿐이다. 앞서 기다리던 것은 이 키보다 먼저다.
        flushPendingNewline()
        let work = DispatchWorkItem {
            guard let pending = pendingNewline else { return }
            pendingNewline = nil
            insertNewline(pending, reason: "timer")
        }
        pendingNewline = PendingNewline(client: client, work: work, scheduledAt: ProcessInfo.processInfo.systemUptime)
        DispatchQueue.main.asyncAfter(deadline: .now() + wait, execute: work)
    }

    /// 기다리는 줄바꿈이 있으면 지금 넣는다(다음 키가 새 조합을 시작하기 전에). 없으면 아무것도 안 한다.
    static func flushPendingNewline() {
        guard let pending = pendingNewline else { return }
        pendingNewline = nil
        pending.work.cancel()
        insertNewline(pending, reason: "nextKey")
    }

    private static func insertNewline(_ pending: PendingNewline, reason: String) {
        pending.client.insertText("\n" as NSString, replacementRange: NSRange(location: NSNotFound, length: 0))
        DeveloperLogger.shared.log("Reposter", "newline inserted", metadata: [
            "waitedMs": String(format: "%.0f", (ProcessInfo.processInfo.systemUptime - pending.scheduledAt) * 1000),
            "reason": reason,
        ])
    }
}
