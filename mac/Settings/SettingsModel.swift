import AppKit
import Foundation
import SwiftUI

/// config.toml의 설정. 러스트 코어 `Config`의 JSON 모양이고 키 이름은 파일과 같다(snake_case ↔ camelCase는
/// JSON 인코더·디코더가 바꾼다). 모양과 값 검사는 코어에 있고(config-ffi), 여기서는 그대로 받아 고칠 뿐이다.
struct CssgsgConfig: Codable, Equatable {
    var koLayout: String
    var tapThresholdMs: Int
    /// 빠른 탭 전환 보정(실험적): 탭 수식키를 떼기 직전에 친 글자를 잠깐 잡아 두었다가 전환 뒤에 친다.
    var tapBuffering: Bool
    var tapOverlapMs: Int
    var capsShiftInverts: Bool
    var shortcuts: Shortcuts
    var ja: Ja
    var mac: Mac

    /// 단축키 글자열: "tap:shift_right"(수식키 탭), "alt+enter"(조합), ""(없음). ShortcutText가 화면 글자로 바꾼다.
    struct Shortcuts: Codable, Equatable {
        var toggleEnglish: String
        var toggleNonEnglish: String
        var hanja: String
    }

    struct Ja: Codable, Equatable {
        var punctuation: String
        var slashNakaguro: Bool
        var fullWidthSpace: Bool
        var yenSign: Bool
        var convertWithSpace: Bool
        var convertWithTab: Bool
        var capsKatakanaAutoOff: Bool
        var katakanaDirect: Bool
    }

    struct Mac: Codable, Equatable {
        var hud: Bool
        var hudPosition: String
        var candidateFontSize: Int
        /// 줄바꿈 대기 조정(-50~50). 기본값은 Electron 15ms, Codex류 120ms(코어 MacConfig).
        var newlineDelayOffsetMs: Int

        /// 코어 MacConfig::shift_enter_delay_ms/newline_replay_ms와 같은 계산(화면에 보이기만 한다).
        var electronDelayMs: Int { max(5, 15 + newlineDelayOffsetMs) }
        var codexDelayMs: Int { max(5, 120 + newlineDelayOffsetMs) }
    }
}

/// 러스트 설정 라이브러리(libcssgsg_config.a, config-ffi) 부르기.
enum ConfigBridge {
    /// 파일 내용(nil이면 기본 설정) → 설정. 틀렸으면 까닭.
    static func parse(_ toml: String?) -> Result<CssgsgConfig, ConfigProblem> {
        let json: String? = {
            let pointer = toml.map { $0.withCString { cssgsg_config_json($0) } } ?? cssgsg_config_json(nil)
            return pointer.map { String(cString: $0) }
        }()
        guard let json, let data = json.data(using: .utf8) else { return .failure(.lastError) }
        do {
            let decoder = JSONDecoder()
            decoder.keyDecodingStrategy = .convertFromSnakeCase
            return .success(try decoder.decode(CssgsgConfig.self, from: data))
        } catch {
            return .failure(ConfigProblem(message: "\(error)"))
        }
    }

    /// 설정 → 파일 내용(설명이 달리고 기본값인 설정은 주석).
    static func render(_ config: CssgsgConfig) -> Result<String, ConfigProblem> {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        guard let data = try? encoder.encode(config), let json = String(data: data, encoding: .utf8) else {
            return .failure(ConfigProblem(message: tr("설정을 JSON으로 바꾸지 못했다", "Could not encode the settings",
                                                      "設定を JSON に変換できません")))
        }
        guard let pointer = json.withCString({ cssgsg_config_toml($0) }) else { return .failure(.lastError) }
        return .success(String(cString: pointer))
    }
}

struct ConfigProblem: Error {
    let message: String
    static var lastError: ConfigProblem { ConfigProblem(message: String(cString: cssgsg_config_error())) }
}

/// 설정 화면의 상태. 파일이 원본이다: 바꿀 때마다 파일을 다시 읽고(직접 고친 것을 덮지 않게) 그 위에 바꿔 쓴 뒤
/// 입력기에 알린다(Darwin 알림). 입력기는 곧바로 다시 읽어 적용한다.
@MainActor
final class SettingsModel: ObservableObject {
    @Published private(set) var config: CssgsgConfig
    /// 설정 파일을 읽지 못한 까닭(직접 고치다 틀림). 그동안 입력기는 기본 설정으로 돈다.
    @Published private(set) var fileProblem: String?
    /// 마지막으로 쓰지 못한 까닭.
    @Published private(set) var writeProblem: String?
    @Published private(set) var hanjaLearningCount = 0
    @Published private(set) var inputSourceAdded = false
    @Published private(set) var developerMode = false
    /// 입력기의 키 보내기(손쉬운 사용) 권한. 모르면 nil(입력기가 떠 있지 않거나 아직 적지 않았다).
    @Published private(set) var postEventAllowed: Bool?
    @Published private(set) var imeRunning = false

    static let defaults: CssgsgConfig = {
        guard case let .success(config) = ConfigBridge.parse(nil) else {
            fatalError("설정 라이브러리가 기본 설정을 주지 못했다: \(ConfigProblem.lastError.message)")
        }
        return config
    }()

    init() {
        config = Self.defaults
        reload()
    }

    /// 파일과 시스템 상태를 다시 읽는다(창이 앞으로 올 때, 바꾸기 전).
    func reload() {
        let text = try? String(contentsOf: Cssgsg.configURL, encoding: .utf8)
        switch ConfigBridge.parse(text) {
        case let .success(parsed):
            if parsed != config { config = parsed }
            fileProblem = nil
        case let .failure(problem):
            config = Self.defaults
            fileProblem = problem.message
        }
        hanjaLearningCount = Self.learningCount()
        inputSourceAdded = Cssgsg.isInputSourceAdded
        developerMode = Self.readDeveloperMode()
    }

    func binding<Value: Equatable>(_ path: WritableKeyPath<CssgsgConfig, Value>) -> Binding<Value> {
        Binding(get: { self.config[keyPath: path] }, set: { value in self.update { $0[keyPath: path] = value } })
    }

    /// 단축키 하나를 바꾼다. 받아 줄 수 없으면(다른 곳에 이미 쓰는 단축키 등) 까닭을 돌려주고 바꾸지 않는다.
    func setShortcut(_ path: WritableKeyPath<CssgsgConfig.Shortcuts, String>, to value: String) -> String? {
        reload()
        let others: [WritableKeyPath<CssgsgConfig.Shortcuts, String>] =
            [\.toggleEnglish, \.toggleNonEnglish, \.hanja].filter { $0 != path }
        if !value.isEmpty, let taken = others.first(where: { config.shortcuts[keyPath: $0] == value }) {
            return tr("이미 ‘\(ShortcutText.actionName(taken))’에 쓰고 있다.", "Already used for “\(ShortcutText.actionName(taken))”.",
                      "すでに「\(ShortcutText.actionName(taken))」に使っています。")
        }
        var next = config
        next.shortcuts[keyPath: path] = value
        // 코어가 받아 주는지 먼저 본다(수식키 없는 글자 키 등). 설정 앱이 미리 거르지 못한 경우의 까닭은 코어의 말이다.
        if case let .failure(problem) = ConfigBridge.render(next) { return problem.message }
        update { $0.shortcuts[keyPath: path] = value }
        return nil
    }

    func update(_ change: (inout CssgsgConfig) -> Void) {
        reload()
        var next = config
        change(&next)
        guard next != config || fileProblem != nil else { return }
        switch ConfigBridge.render(next) {
        case let .failure(problem):
            writeProblem = problem.message
        case let .success(text):
            do {
                try write(text)
                config = next
                fileProblem = nil
                writeProblem = nil
                Cssgsg.Notice.configChanged.post()
            } catch {
                writeProblem = error.localizedDescription
            }
        }
    }

    private func write(_ text: String) throws {
        let fm = FileManager.default
        try fm.createDirectory(at: Cssgsg.supportDirectory, withIntermediateDirectories: true)
        let url = Cssgsg.configURL
        if fileProblem != nil, fm.fileExists(atPath: url.path) {
            // 틀린 파일은 지우지 않고 옆에 남긴다.
            let backup = url.appendingPathExtension("bak")
            try? fm.removeItem(at: backup)
            try fm.moveItem(at: url, to: backup)
        }
        try text.write(to: url, atomically: true, encoding: .utf8)
    }

    /// 설정 파일을 Finder에서 보인다. 없으면 지금 설정(기본값)으로 만든다.
    func revealConfigFile() {
        let url = Cssgsg.configURL
        if !FileManager.default.fileExists(atPath: url.path), case let .success(text) = ConfigBridge.render(config) {
            try? write(text)
        }
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }

    // MARK: - 입력기 상태

    /// 입력기에게 상태(키 보내기 권한)를 다시 적어 달라고 하고, 조금 뒤 읽는다. 입력기가 떠 있지 않으면 nil.
    /// 그동안은 입력기가 지난번에 적어 둔 값을 보인다.
    func refreshIMEStatus() {
        imeRunning = !NSRunningApplication.runningApplications(withBundleIdentifier: Cssgsg.imeBundleID).isEmpty
        guard imeRunning else {
            postEventAllowed = nil
            return
        }
        postEventAllowed = Cssgsg.imePreference(Cssgsg.postEventAllowedKey) as Bool?
        Cssgsg.Notice.statusRequest.post()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { [weak self] in
            self?.postEventAllowed = Cssgsg.imePreference(Cssgsg.postEventAllowedKey) as Bool?
        }
    }

    /// 키 보내기 권한: 입력기가 시스템 창을 청하고(처음 한 번만 뜬다), 손쉬운 사용 설정을 연다.
    /// 권한은 입력기 프로세스가 받아야 해서 설정 앱이 직접 청하지 않는다. 켜는 것은 사용자가 한다.
    func requestPostEventAccess() {
        Cssgsg.Notice.requestPostEventAccess.post()
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
            NSWorkspace.shared.open(url)
        }
    }

    // MARK: - 학습

    private static func learningCount() -> Int {
        guard let text = try? String(contentsOf: Cssgsg.hanjaLearningURL, encoding: .utf8) else { return 0 }
        return text.split(separator: "\n").filter { !$0.hasPrefix("#") && !$0.isEmpty }.count
    }

    /// 고른 한자 기억을 지운다. 입력기가 떠 있으면 들고 있는 기억도 비우고 빈 파일을 쓴다.
    func clearHanjaLearning() {
        try? FileManager.default.removeItem(at: Cssgsg.hanjaLearningURL)
        Cssgsg.Notice.hanjaLearningCleared.post()
        hanjaLearningCount = 0
    }

    /// Mozc가 기억한 변환(문절 나누기, 고른 후보, 전각·반각)을 지우고 입력기를 다시 시작한다.
    /// 파일은 입력기가 mmap으로 들고 있어서 지운 뒤 끝내면 되살아나지 않는다. 다음 키 입력 때 macOS가 다시 띄운다.
    func clearMozcLearning() {
        for name in ["segment.db", "boundary.db", "cform.db", ".history.db"] {
            try? FileManager.default.removeItem(at: Cssgsg.mozcProfileURL.appendingPathComponent(name))
        }
        Cssgsg.Notice.restart.post()
    }

    // MARK: - 개발자 기록 (입력기의 기본값 저장소 developerMode)

    private static func readDeveloperMode() -> Bool {
        let domain = Cssgsg.imeBundleID as CFString
        CFPreferencesAppSynchronize(domain)
        return (CFPreferencesCopyAppValue("developerMode" as CFString, domain) as? Bool) ?? false
    }

    func setDeveloperMode(_ on: Bool) {
        let domain = Cssgsg.imeBundleID as CFString
        CFPreferencesSetAppValue("developerMode" as CFString, NSNumber(value: on), domain)
        CFPreferencesAppSynchronize(domain)
        developerMode = Self.readDeveloperMode()
    }
}
