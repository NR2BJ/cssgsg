import AppKit
import Foundation

/// 입력기(cssgsg.app)와 설정 앱(cssgsgSettings.app)이 같이 쓰는 이름·경로·알림. 두 타깃에 같이 들어간다.
enum Cssgsg {
    static let imeBundleID = "com.cssgsg.inputmethod.app"
    static let settingsBundleID = "com.cssgsg.settings"
    /// 입력 소스 모드 ID(Info.plist의 tsInputModeListKey).
    static let inputModeID = imeBundleID + ".en"
    /// 설정 앱 번들 이름. 입력기 옆(/Library/Input Methods)에 같이 설치한다(NRIME와 같다).
    static let settingsAppName = "cssgsgSettings.app"

    // MARK: - 경로 (~/Library/Application Support/cssgsg)

    static var supportDirectory: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("cssgsg", isDirectory: true)
    }
    static var configURL: URL { supportDirectory.appendingPathComponent("config.toml") }
    static var hanjaLearningURL: URL { supportDirectory.appendingPathComponent("hanja-learning.tsv") }
    /// Mozc 학습·사용자 사전 폴더(NRIME의 ~/Library/Application Support/Mozc와 섞이지 않게 따로).
    static var mozcProfileURL: URL { supportDirectory.appendingPathComponent("mozc", isDirectory: true) }
    static var developerLogURL: URL { supportDirectory.appendingPathComponent("developer.log") }

    // MARK: - 설정 앱 → 입력기 (Darwin 알림, 내용 없음)

    enum Notice: String {
        /// 설정 파일을 고쳤다: 다시 읽어 바로 적용한다.
        case configChanged = "com.cssgsg.config-changed"
        /// 한자 학습 파일을 비웠다: 들고 있는 기억도 비운다.
        case hanjaLearningCleared = "com.cssgsg.hanja-learning-cleared"
        /// 끝낸다(Mozc 학습 파일을 지운 뒤). 다음 키 입력 때 macOS가 다시 띄운다.
        case restart = "com.cssgsg.restart"

        func post() {
            CFNotificationCenterPostNotification(
                CFNotificationCenterGetDarwinNotifyCenter(), CFNotificationName(rawValue as CFString), nil, nil, true)
        }
    }

    // MARK: - 입력기 → 설정 앱 (분산 알림, object = 탭 이름)

    /// 이미 떠 있는 설정 앱에 이 탭을 보이라고 한다. 처음 띄울 때는 `--tab 이름` 인자로 준다.
    static let showSettingsTab = Notification.Name("com.cssgsg.settings.show-tab")

    // MARK: - 입력 소스

    /// 사용자가 입력 소스에 cssgsg를 추가했는지. TIS에 묻지 않고 시스템 설정이 쓰는 저장 파일을 읽는다
    /// (macOS 27에서 TIS의 켜짐 속성과 켜진 목록은 틀린다, InputSourceSetup 참고). 못 읽으면 false.
    static var isInputSourceAdded: Bool {
        let domain = "com.apple.inputsources" as CFString
        CFPreferencesAppSynchronize(domain)
        let key = "AppleEnabledThirdPartyInputSources" as CFString
        guard let list = CFPreferencesCopyAppValue(key, domain) as? [[String: Any]] else { return false }
        return list.contains { ($0["Input Mode"] as? String) == inputModeID }
    }

    /// 시스템 설정 → 키보드. 입력 소스 "편집…" → + → 영어 → cssgsg 로 추가한다.
    static func openKeyboardSettings() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.Keyboard-Settings.extension") {
            NSWorkspace.shared.open(url)
        }
    }
}
