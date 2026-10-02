import AppKit
import Foundation
import SwiftUI

/// config.toml의 설정. 러스트 코어 `Config`의 JSON 모양이고 키 이름은 파일과 같다(snake_case ↔ camelCase는
/// JSON 인코더·디코더가 바꾼다). 모양과 값 검사는 코어에 있고(config-ffi), 여기서는 그대로 받아 고칠 뿐이다.
struct CssgsgConfig: Codable, Equatable {
    var koLayout: String
    var tapThresholdMs: Int
    /// 빠른 탭 전환 보정(실험적): 탭 수식키를 떼기 직전에 친 글자를 잠깐 잡아 두었다가 전환 뒤에 친다.
    /// 판정 시간은 엔진이 정해 둔 값이다(Shift가 글자를 바꾸는지에 따라 30ms·80ms).
    var tapBuffering: Bool
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
        /// 줄바꿈 넣기(웹 기술로 만든 앱)·⌘ 단축키 다시 보내기(모든 앱) 대기(밀리초, 0~100, 기본 20).
        var newlineInsertWaitMs: Int
        /// Shift+Enter 다시 보내기 대기(밀리초, 0~100, 기본 50): newlineKeyPressApps의 앱.
        var newlineKeyPressWaitMs: Int
        /// Shift+Enter 키를 다시 보낼 앱(번들 ID). 기본 Codex(com.openai.codex).
        var newlineKeyPressApps: [String]
    }
}

/// 러스트 설정 라이브러리(libcssgsg_config.a, config-ffi) 부르기.
///
/// 설정 파일 전체(코어 `Config`의 JSON)를 같이 들고 있다가, 쓸 때 이 앱이 고친 값만 그 위에 얹는다. 그래서 맥 설정 앱이
/// 모르는 값(윈도우 설정 앱이 쓴 `[windows]` 등)도 그대로 남는다. 0.7.1까지는 아는 값만으로 다시 써서 지웠다.
enum ConfigBridge {
    /// 파일 내용(nil이면 기본 설정) → 설정과 설정 전체의 JSON. 틀렸으면 까닭.
    static func parse(_ toml: String?) -> Result<(config: CssgsgConfig, document: String), ConfigProblem> {
        let json: String? = {
            let pointer = toml.map { $0.withCString { cssgsg_config_json($0) } } ?? cssgsg_config_json(nil)
            return pointer.map { String(cString: $0) }
        }()
        guard let json, let data = json.data(using: .utf8) else { return .failure(.lastError) }
        do {
            let decoder = JSONDecoder()
            decoder.keyDecodingStrategy = .convertFromSnakeCase
            return .success((try decoder.decode(CssgsgConfig.self, from: data), json))
        } catch {
            return .failure(ConfigProblem(message: "\(error)"))
        }
    }

    /// 설정 → 파일 내용(설명이 달리고 기본값인 설정은 주석). `document`(parse가 준 설정 전체)에 이 앱이 아는 값을 얹는다.
    static func render(_ config: CssgsgConfig, over document: String) -> Result<String, ConfigProblem> {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        guard let data = try? encoder.encode(config),
              let mine = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let base = (try? JSONSerialization.jsonObject(with: Data(document.utf8))) as? [String: Any],
              let merged = try? JSONSerialization.data(withJSONObject: merge(base, mine), options: [.sortedKeys]),
              let json = String(data: merged, encoding: .utf8)
        else {
            return .failure(ConfigProblem(message: tr("설정을 JSON으로 바꾸지 못했습니다.", "Couldn’t encode the settings.",
                                                      "設定を JSON に変換できませんでした。")))
        }
        guard let pointer = json.withCString({ cssgsg_config_toml($0) }) else { return .failure(.lastError) }
        return .success(String(cString: pointer))
    }

    /// `over`의 값으로 `base`를 덮는다. 표(객체)는 안으로 들어가 덮어서, `over`에 없는 키는 `base` 것이 남는다.
    static func merge(_ base: [String: Any], _ over: [String: Any]) -> [String: Any] {
        var out = base
        for (key, value) in over {
            if let inner = base[key] as? [String: Any], let replacement = value as? [String: Any] {
                out[key] = merge(inner, replacement)
            } else {
                out[key] = value
            }
        }
        return out
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
    /// 마지막으로 읽은 설정 전체(JSON). 쓸 때 이 위에 config를 얹는다(ConfigBridge.render).
    private var document: String
    /// 설정 파일을 읽지 못한 까닭(직접 고치다 틀림). 그동안 입력기는 앞의 올바른 설정(처음이면 기본 설정)으로 돈다.
    @Published private(set) var fileProblem: String?
    /// 마지막으로 쓰지 못한 까닭.
    @Published private(set) var writeProblem: String?
    @Published private(set) var hanjaLearningCount = 0
    @Published private(set) var inputSourceAdded = false
    @Published private(set) var developerMode = false
    /// 입력기가 마지막으로 확인한 권한(PermissionStatus). 아직 적지 않았으면 nil.
    @Published private(set) var permission: PermissionStatus?
    @Published private(set) var imeRunning = false
    /// 입력기가 쓰는 Mozc 엔진과 받아 둔 새 엔진(MozcStatus). 아직 적지 않았으면 nil.
    @Published private(set) var mozc: MozcStatus?
    /// "지금 확인"을 누르고 입력기의 답을 기다리는 중.
    @Published private(set) var mozcChecking = false
    private var observers: [NSObjectProtocol] = []

    static let defaults: CssgsgConfig = defaultDocument.config
    private static let defaultDocument: (config: CssgsgConfig, document: String) = {
        guard case let .success(parsed) = ConfigBridge.parse(nil) else {
            fatalError("설정 라이브러리가 기본 설정을 주지 못했다: \(ConfigProblem.lastError.message)")
        }
        return parsed
    }()

    init() {
        config = Self.defaults
        document = Self.defaultDocument.document
        reload()
        // 입력기가 권한이나 Mozc 상태를 다시 적으면 바로 다시 읽는다.
        let center = DistributedNotificationCenter.default()
        observers.append(center.addObserver(forName: PermissionStatus.changedNotification, object: nil, queue: .main) {
            [weak self] _ in
            MainActor.assumeIsolated { self?.readPermission() }
        })
        observers.append(center.addObserver(forName: MozcStatus.changedNotification, object: nil, queue: .main) {
            [weak self] _ in
            MainActor.assumeIsolated {
                self?.mozcChecking = false
                self?.readMozcStatus()
            }
        })
        readMozcStatus()
    }

    /// 파일과 시스템 상태를 다시 읽는다(창이 앞으로 올 때, 바꾸기 전).
    func reload() {
        let text = try? String(contentsOf: Cssgsg.configURL, encoding: .utf8)
        switch ConfigBridge.parse(text) {
        case let .success(parsed):
            if parsed.config != config { config = parsed.config }
            document = parsed.document
            fileProblem = nil
        case let .failure(problem):
            config = Self.defaults
            document = Self.defaultDocument.document
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
            return tr("이미 ‘\(ShortcutText.actionName(taken))’에 쓰고 있는 단축키입니다.",
                      "Already used for “\(ShortcutText.actionName(taken))”.",
                      "すでに「\(ShortcutText.actionName(taken))」で使っているショートカットです。")
        }
        var next = config
        next.shortcuts[keyPath: path] = value
        // 코어가 받아 주는지 먼저 본다(수식키 없는 글자 키 등). 설정 앱이 미리 거르지 못한 경우의 까닭은 코어의 말이다.
        if case let .failure(problem) = ConfigBridge.render(next, over: document) { return problem.message }
        update { $0.shortcuts[keyPath: path] = value }
        return nil
    }

    func update(_ change: (inout CssgsgConfig) -> Void) {
        reload()
        var next = config
        change(&next)
        guard next != config || fileProblem != nil else { return }
        switch ConfigBridge.render(next, over: document) {
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
        if !FileManager.default.fileExists(atPath: url.path),
           case let .success(text) = ConfigBridge.render(config, over: document) {
            try? write(text)
        }
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }

    // MARK: - 입력기 상태

    /// 입력기에게 권한을 다시 확인해 적어 달라고 한다(적으면 알림이 온다). 그동안은 지난번 값을 보인다.
    func refreshIMEStatus() {
        imeRunning = !NSRunningApplication.runningApplications(withBundleIdentifier: Cssgsg.imeBundleID).isEmpty
        readPermission()
        if imeRunning { Cssgsg.Notice.statusRequest.post() }
    }

    private func readPermission() {
        permission = Cssgsg.imeStatus(PermissionStatus.self, key: PermissionStatus.defaultsKey)
    }

    /// "다시 확인 / 권한 요청": 입력기가 없는 권한을 청하고(시스템 창) 다시 확인한다.
    /// 권한은 입력기 프로세스가 받아야 해서 설정 앱이 직접 청하지 않는다. 켜는 것은 사용자가 한다.
    func recheckPermission() {
        Cssgsg.Notice.requestPostEventAccess.post()
    }

    /// 시스템 설정 → 개인정보 보호 및 보안 → 기기 제어 및 데이터 접근(macOS 26 이하: 손쉬운 사용).
    func openPermissionSettings() {
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
    /// 파일은 입력기가 mmap으로 들고 있어서 지운 뒤 끝내면 되살아나지 않는다.
    func clearMozcLearning() {
        for name in ["segment.db", "boundary.db", "cform.db", ".history.db"] {
            try? FileManager.default.removeItem(at: Cssgsg.mozcProfileURL.appendingPathComponent(name))
        }
        Cssgsg.Notice.restart.post()
    }

    // MARK: - Mozc 엔진 (입력기가 따로 업데이트한다, MozcUpdater)

    func readMozcStatus() {
        mozc = Cssgsg.imeStatus(MozcStatus.self, key: MozcStatus.defaultsKey)
    }

    /// "지금 확인": 입력기가 확인하고 상태를 다시 적는다(알림이 오면 다시 읽는다). 입력기가 없으면 아무 일도 없다.
    func checkMozcUpdate() {
        guard !NSRunningApplication.runningApplications(withBundleIdentifier: Cssgsg.imeBundleID).isEmpty else { return }
        mozcChecking = true
        Cssgsg.Notice.mozcCheckForUpdate.post()
        // 답이 없으면(입력기가 막 끝났다) 기다림 표시만 거둔다.
        DispatchQueue.main.asyncAfter(deadline: .now() + 60) { [weak self] in self?.mozcChecking = false }
    }

    /// "지금 적용": 입력기를 다시 띄운다. 뜰 때 받아 둔 새 엔진을 읽는다.
    func applyMozcUpdate() {
        Cssgsg.Notice.restart.post()
    }

    /// 앱에 든 엔진의 판(입력기 옆 cssgsg.app의 MOZC_VERSION, "<커밋> <날짜> <버전>"). 입력기가 아직 적지 않았을 때 보인다.
    static var bundledMozc: MozcStatus.Build? {
        let file = Bundle.main.bundleURL.deletingLastPathComponent()
            .appendingPathComponent("cssgsg.app/Contents/Resources/MOZC_VERSION")
        guard let line = try? String(contentsOf: file, encoding: .utf8) else { return nil }
        let parts = line.split(whereSeparator: \.isWhitespace).map(String.init)
        guard parts.count >= 3 else { return nil }
        return MozcStatus.Build(version: parts[2], date: parts[1], commit: parts[0],
                                wrapper: parts.count >= 4 ? Int(parts[3]) : nil)
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
