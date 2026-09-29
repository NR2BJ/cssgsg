import AppKit
import Foundation
import SwiftUI

/// config.toml의 설정. 러스트 코어 `Config`의 JSON 모양이고 키 이름은 파일과 같다.
/// 모양과 값 검사는 코어에 있고(config-ffi), 여기서는 그대로 받아 고칠 뿐이다.
struct CssgsgConfig: Codable, Equatable {
    var koLayout: String
    var tapThresholdMs: Int
    /// 탭 키 이름 → "toggle_english" / "toggle_non_english". 적지 않은 키는 아무 일도 하지 않는다.
    var taps: [String: String]
    var capsShiftInverts: Bool
    var ja: Ja
    var mac: Mac

    struct Ja: Codable, Equatable {
        var punctuation: String
        var slashNakaguro: Bool
        var fullWidthSpace: Bool
        var capsKatakanaAutoOff: Bool
        var katakanaDirect: Bool

        enum CodingKeys: String, CodingKey {
            case punctuation
            case slashNakaguro = "slash_nakaguro"
            case fullWidthSpace = "full_width_space"
            case capsKatakanaAutoOff = "caps_katakana_auto_off"
            case katakanaDirect = "katakana_direct"
        }
    }

    struct Mac: Codable, Equatable {
        var hud: Bool
        var hudPosition: String
        var newlineReplayMs: Int

        enum CodingKeys: String, CodingKey {
            case hud
            case hudPosition = "hud_position"
            case newlineReplayMs = "newline_replay_ms"
        }
    }

    // 키 이름 바꾸기 전략(convertFromSnakeCase)은 taps 사전의 키(shift_right)까지 바꿔서 쓰지 않는다.
    enum CodingKeys: String, CodingKey {
        case koLayout = "ko_layout"
        case tapThresholdMs = "tap_threshold_ms"
        case taps
        case capsShiftInverts = "caps_shift_inverts"
        case ja
        case mac
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
            return .success(try JSONDecoder().decode(CssgsgConfig.self, from: data))
        } catch {
            return .failure(ConfigProblem(message: "\(error)"))
        }
    }

    /// 설정 → 파일 내용(설명이 달리고 기본값인 설정은 주석).
    static func render(_ config: CssgsgConfig) -> Result<String, ConfigProblem> {
        guard let data = try? JSONEncoder().encode(config), let json = String(data: data, encoding: .utf8) else {
            return .failure(ConfigProblem(message: "설정을 JSON으로 바꾸지 못했다"))
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

    static let defaults: CssgsgConfig = {
        guard case let .success(config) = ConfigBridge.parse(nil) else {
            fatalError("설정 라이브러리가 기본 설정을 주지 못했다: \(String(cString: cssgsg_config_error()))")
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

    /// 탭 키 하나의 동작("none"은 표에서 뺀다).
    func tapBinding(_ key: String) -> Binding<String> {
        Binding(
            get: { self.config.taps[key] ?? "none" },
            set: { value in
                self.update { config in
                    if value == "none" { config.taps.removeValue(forKey: key) } else { config.taps[key] = value }
                }
            })
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
