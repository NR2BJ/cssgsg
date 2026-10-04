import AppKit
import Carbon
import InputMethodKit

/// 인증 창(관리자 암호, 시스템 암호 시트)의 비밀번호를 Graphite로 치게 한다(0.7.5, 사용자 결정 "비밀번호도 Graphite").
///
/// 이 창들은 입력기가 넣은 글자를 버려서(NRIME) 입력기는 키를 넘긴다(PasswordFields). 넘긴 키를 글자로 바꾸는 자판을
/// 그 앱에서만 Graphite 자판(/Library/Keyboard Layouts/cssgsg-Graphite.bundle, tools/mac/keylayout이 엔진 데이터로 만든다.
/// ⌘·Control·Option 층은 ABC라 단축키는 그대로)으로 바꿔 끼운다.
///
/// - IMKTextInput.overrideKeyboard(withKeyboardNamed:)는 앱(클라이언트) 프로세스 안에서 설치된 자판을 입력 소스 ID(없으면 자판 이름)로
///   찾아 TISSetInputMethodKeyboardLayoutOverride를 부른다(HIToolbox -[IMKInputSession_Modern overrideKeyboardWithKeyboardNamed:]를
///   lldb로 봤다). 입력기 프로세스에서 그 함수를 부르는 길은 macOS 26부터 무시되지만(다른 입력기 두 곳의 측정) 이 길은 앱 쪽에서 부른다.
///   Mozc·Keyman·macSKK가 같은 메서드로 기반 자판을 정한다. 사용자가 이 자판을 입력 소스로 추가할 필요는 없다(설치만 되면 찾는다).
/// - macSKK는 활성화마다 부르다가 Apple 앱이 죽는다는 신고를 받고 그만뒀다(#292). 그래서 인증 창이 활성화될 때와, 그 뒤 처음
///   다른 앱이 활성화될 때(되돌리기)만 부른다.
/// - 끼운 자판이 실제로 먹었는지는 인증 창에서 처음 친, Graphite와 쿼티가 다른 키의 글자로 한 번 본다(글자는 기록하지 않는다).
enum PasswordLayout {
    static let bundleURL = URL(fileURLWithPath: "/Library/Keyboard Layouts/cssgsg-Graphite.bundle")
    static let sourceID = "com.cssgsg.keylayout.graphite"
    /// 자판을 끼울 인증 창(키를 넘기는 곳): 관리자 암호 창, 시스템 설정 등의 암호 시트.
    /// 잠금 화면(com.apple.loginwindow)은 아직 넣지 않는다: 재부팅 직후 로그인 창은 입력기가 없어 쿼티라서, 잠금 화면만 Graphite면
    /// 같은 암호를 두 가지로 쳐야 한다. 로그인 창 자판과 같이 바꾼다(0.7.5-beta.1, CONCEPT §13).
    static let clients: Set<String> = ["com.apple.SecurityAgent", "com.apple.LocalAuthenticationRemoteService"]

    /// Graphite 자판을 끼운 앱(되돌리기 전까지).
    private static var overriddenApp: String?
    /// 이번에 끼운 뒤 먹었는지 봤다.
    private static var checked = false

    /// 입력기가 뜰 때 자판 번들을 입력 소스 서버에 알린다(새로 설치했을 때 로그아웃 없이 찾게). 켜지는 않는다.
    static func register() {
        guard FileManager.default.fileExists(atPath: bundleURL.path) else {
            DeveloperLogger.shared.log("PasswordLayout", "bundle missing", metadata: ["path": bundleURL.path])
            return
        }
        let status = TISRegisterInputSource(bundleURL as CFURL)
        DeveloperLogger.shared.log("PasswordLayout", "register", metadata: [
            "status": "\(status)", "found": "\(source(sourceID) != nil)",
        ])
    }

    /// 입력기가 그 앱에서 활성화됐다(activateServer). 인증 창이면 Graphite 자판을 끼우고, 끼운 뒤 다른 앱이 오면 되돌린다.
    static func activated(client: any IMKTextInput) {
        let app = client.bundleIdentifier()
        if let app, clients.contains(app) {
            guard source(sourceID) != nil else {
                DeveloperLogger.shared.log("PasswordLayout", "layout not installed", metadata: ["app": app])
                return
            }
            client.overrideKeyboard(withKeyboardNamed: sourceID)
            overriddenApp = app
            checked = false
            DeveloperLogger.shared.log("PasswordLayout", "override", metadata: ["app": app, "layout": sourceID])
        } else if let previous = overriddenApp {
            let ascii = currentASCIILayoutID() ?? "com.apple.keylayout.ABC"
            client.overrideKeyboard(withKeyboardNamed: ascii)
            overriddenApp = nil
            DeveloperLogger.shared.log("PasswordLayout", "restore", metadata: [
                "after": previous, "app": app ?? "unknown", "layout": ascii,
            ])
        }
    }

    /// 인증 창에서 처음 친, Graphite와 쿼티가 다른 글자 키로 끼운 자판이 먹었는지 한 번 기록한다. 글자는 남기지 않는다.
    static func check(_ event: NSEvent, app: String?) {
        guard !checked, let app, app == overriddenApp, event.type == .keyDown,
              event.modifierFlags.intersection([.command, .control, .option]).isEmpty,
              let typed = event.characters,
              let graphite = character(layout: sourceID, code: event.keyCode, shift: event.modifierFlags.contains(.shift)),
              let ascii = character(layout: currentASCIILayoutID() ?? "com.apple.keylayout.ABC", code: event.keyCode,
                                    shift: event.modifierFlags.contains(.shift)),
              graphite != ascii
        else { return }
        checked = true
        DeveloperLogger.shared.log("PasswordLayout", "check", metadata: [
            "app": app, "graphite": "\(typed == graphite)", "qwerty": "\(typed == ascii)",
        ])
    }

    // MARK: - TIS

    private static func source(_ id: String) -> TISInputSource? {
        let filter = [kTISPropertyInputSourceID as String: id] as CFDictionary
        return (TISCreateInputSourceList(filter, true)?.takeRetainedValue() as? [TISInputSource])?.first
    }

    /// 입력기가 키를 넘길 때 macOS가 쓰는 자판(가장 최근에 쓴 ASCII 자판, 보통 ABC).
    private static func currentASCIILayoutID() -> String? {
        guard let layout = TISCopyCurrentASCIICapableKeyboardLayoutInputSource()?.takeRetainedValue(),
              let raw = TISGetInputSourceProperty(layout, kTISPropertyInputSourceID) else { return nil }
        return Unmanaged<CFString>.fromOpaque(raw).takeUnretainedValue() as String
    }

    /// 자판이 그 키(수식키 없음 또는 Shift)로 내는 글자.
    private static func character(layout id: String, code: UInt16, shift: Bool) -> String? {
        guard let source = source(id), let raw = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) else {
            return nil
        }
        let data = Unmanaged<CFData>.fromOpaque(raw).takeUnretainedValue() as Data
        var dead: UInt32 = 0
        var length = 0
        var buffer = [UniChar](repeating: 0, count: 4)
        let status = data.withUnsafeBytes { bytes -> OSStatus in
            UCKeyTranslate(
                bytes.bindMemory(to: UCKeyboardLayout.self).baseAddress!, code, UInt16(kUCKeyActionDown),
                shift ? UInt32(shiftKey >> 8) & 0xFF : 0, UInt32(LMGetKbdType()), OptionBits(kUCKeyTranslateNoDeadKeysMask),
                &dead, buffer.count, &length, &buffer)
        }
        guard status == noErr, length > 0 else { return nil }
        return String(utf16CodeUnits: buffer, count: length)
    }
}
