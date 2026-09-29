import AppKit
import Carbon
import Foundation

/// 입력 소스 등록과 "추가했는지" 확인.
///
/// macOS 27에서 서드파티 입력기는 프로그램이 켤 수 없다. TISEnableInputSource는 입력기 안에서도, 바깥 프로세스에서도
/// noErr를 돌려주지만 아무것도 저장하지 않는다(켜진 목록은 ~/Library/Preferences/com.apple.inputsources.plist의
/// AppleEnabledThirdPartyInputSources이고, 시스템 설정에서 사람이 추가해야만 바뀐다). 그래서 켜지 않고 안내만 한다.
///
/// "추가했는지"는 TIS로 묻지 않고 시스템 설정이 쓰는 저장 파일을 읽는다(2026-09-29 macOS 27에서 확인).
/// - kTISPropertyInputSourceIsEnabled: 추가하지 않은 입력기의 모드도 tsInputModeDefaultStateKey가 true면 "켜짐"이다
///   (아이누어 입력기도 그렇다).
/// - TISCreateInputSourceList(…, false): 같은 프로세스에서 먼저 전체 목록(true)을 받아 두면 그런 "기본 켜짐" 모드가
///   켜진 목록에 섞여 나온다. 호출 순서에 따라 답이 달라서 쓸 수 없다.
enum InputSourceSetup {
    private static var modeID: String { (Bundle.main.bundleIdentifier ?? "com.cssgsg.inputmethod.app") + ".en" }
    private static let promptedKey = "didPromptAddInputSource"

    /// 사용자가 입력 소스에 추가했는지.
    /// 저장 형식을 읽지 못하면 "추가 안 됨"으로 친다(안내 메뉴가 떠 있을 뿐이라 해가 없다).
    static var isAdded: Bool {
        let domain = "com.apple.inputsources" as CFString
        CFPreferencesAppSynchronize(domain)
        let key = "AppleEnabledThirdPartyInputSources" as CFString
        guard let list = CFPreferencesCopyAppValue(key, domain) as? [[String: Any]] else { return false }
        let id = modeID
        return list.contains { ($0["Input Mode"] as? String) == id }
    }

    /// 입력 소스 서버에 이 번들을 (다시) 알린다. 켜지는 않는다. 업데이트로 바뀐 이름(InfoPlist.strings)도 이때 다시 읽힌다.
    static func register() {
        let status = TISRegisterInputSource(Bundle.main.bundleURL as CFURL)
        DeveloperLogger.shared.log("Setup", "register", metadata: ["status": "\(status)", "added": "\(isAdded)"])
    }

    /// 처음 한 번, 아직 추가하지 않았으면 키보드 설정을 열어 준다(pkg postinstall이 설치 직후 앱을 띄운다).
    static func promptOnceIfNotAdded() {
        let defaults = UserDefaults.standard
        guard !defaults.bool(forKey: promptedKey), !isAdded else { return }
        defaults.set(true, forKey: promptedKey)
        openKeyboardSettings()
    }

    /// 시스템 설정 → 키보드. 입력 소스 "편집…" → + → 영어 → cssgsg 로 추가한다.
    static func openKeyboardSettings() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.Keyboard-Settings.extension") {
            NSWorkspace.shared.open(url)
        }
    }
}
