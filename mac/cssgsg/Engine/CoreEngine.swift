import Foundation

/// 입력 모드. 값은 C ABI(CSSGSG_MODE_*)와 같다.
enum InputMode: Int32 {
    case en = 0
    case ko = 1
    case ja = 2

    /// 메뉴 막대와 HUD에 보일 글자: 배열 이름의 머리글자(참신세벌식 ㅊ, Graphite G, 新月 月).
    var label: String {
        switch self {
        case .en: return "G"
        case .ko: return "ㅊ"
        case .ja: return "月"
        }
    }
}

/// 조합 중 글자. 위치와 길이는 UTF-16 단위(NSString과 같다).
struct PreeditUpdate {
    struct Segment {
        let range: NSRange
        let focused: Bool
    }
    let text: String
    let caret: Int
    let segments: [Segment]
}

enum CandidateUpdate {
    case unchanged
    case hide
    /// items: 포커스된 문절의 후보 전체, selected: 전체 목록 기준, grid: 격자(펼친) 모드.
    case show(items: [String], selected: Int?, grid: Bool)
}

/// 엔진이 키 하나를 처리한 결과(Swift 쪽 사본). C 쪽 포인터는 다음 호출 때 무효가 되므로 바로 복사한다.
struct EngineOutput {
    var consumed = false
    var commit = ""
    var preedit: PreeditUpdate?
    var candidates: CandidateUpdate = .unchanged
    var mode: InputMode?
    var capsLockOff = false
}

/// 러스트 코어 엔진. 입력기 프로세스에 하나만 있다(모드는 앱 전체 공통, NRIME와 같다).
/// IMKit은 메인 스레드에서 부르므로 메인 스레드에서만 쓴다.
final class CoreEngine {
    static let shared = CoreEngine(configTOML: try? String(contentsOf: configURL, encoding: .utf8))

    private let engine: OpaquePointer
    /// 설정 파일을 못 읽었으면 그 이유(기본 설정으로 돌았다).
    private(set) var configError: String?
    /// 설정 파일의 [mac] 표(엔진은 쓰지 않고 셸이 쓴다).
    let macSettings: CssgsgMacSettings

    static var configURL: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("cssgsg", isDirectory: true)
            .appendingPathComponent("config.toml")
    }

    /// config.toml 내용으로 만든다. nil이면 기본 설정, 내용에 오류가 있으면 기본 설정으로 돌고 configError에 남긴다.
    /// 입력기는 shared만 쓴다. 따로 만드는 것은 셸 스모크 테스트(tools/mac/shell-smoke)용이다.
    init(configTOML: String?) {
        var made: OpaquePointer?
        if let src = configTOML {
            made = src.withCString { cssgsg_engine_new($0) }
            if made == nil {
                configError = String(cString: cssgsg_last_error())
                NSLog("cssgsg: config.toml 오류, 기본 설정으로 시작: \(configError ?? "")")
            }
        }
        guard let engine = made ?? cssgsg_engine_new(nil) else {
            fatalError("cssgsg: 엔진을 만들 수 없다: \(String(cString: cssgsg_last_error()))")
        }
        self.engine = engine
        macSettings = cssgsg_engine_mac_settings(engine)
    }

    deinit {
        cssgsg_engine_free(engine)
    }

    var mode: InputMode { InputMode(rawValue: cssgsg_engine_mode(engine)) ?? .en }

    /// 일본어 한자 변환(Mozc)을 켠다. 실패하면 false(변환기는 히라가나·가타카나만 내는 임시 변환기 그대로).
    func useMozc(dataPath: String, profileDir: String) -> Bool {
        cssgsg_engine_use_mozc(engine, dataPath, profileDir) != 0
    }

    static var mozcProfileURL: URL {
        configURL.deletingLastPathComponent().appendingPathComponent("mozc", isDirectory: true)
    }

    func handle(_ event: CssgsgKeyEvent, secureField: Bool = false, gameMode: Bool = false, tapsDisabled: Bool = false) -> EngineOutput {
        var ev = event
        var ctx = CssgsgContext(
            game_mode: gameMode ? 1 : 0,
            taps_disabled: tapsDisabled ? 1 : 0,
            secure_field: secureField ? 1 : 0
        )
        return Self.copy(cssgsg_engine_handle_key(engine, &ev, &ctx))
    }

    /// 조합 중인 것을 확정한다(포커스 해제 등).
    func commitAll() -> EngineOutput { Self.copy(cssgsg_engine_commit(engine)) }

    /// 마우스 클릭: 확정하고 진행 중인 수식키 탭을 무효로 한다.
    func mouseDown() -> EngineOutput { Self.copy(cssgsg_engine_mouse_down(engine)) }

    /// 확정하지 않고 비운다(입력기 활성화 때. 여기서 확정하면 Electron에서 글자가 겹친다).
    func reset() -> EngineOutput { Self.copy(cssgsg_engine_reset(engine)) }

    func setMode(_ mode: InputMode) -> EngineOutput { Self.copy(cssgsg_engine_set_mode(engine, mode.rawValue)) }

    private static func copy(_ pointer: UnsafePointer<CssgsgOutput>?) -> EngineOutput {
        guard let o = pointer?.pointee else { return EngineOutput() }
        var out = EngineOutput()
        out.consumed = o.consumed != 0
        out.commit = o.commit.map { String(cString: $0) } ?? ""
        out.capsLockOff = o.caps_lock_off != 0
        out.mode = o.mode >= 0 ? InputMode(rawValue: o.mode) : nil
        if o.preedit_changed != 0 {
            let text = o.preedit.map { String(cString: $0) } ?? ""
            var segments: [PreeditUpdate.Segment] = []
            for i in 0..<Int(o.segment_count) {
                guard let s = o.segments?[i] else { continue }
                segments.append(.init(range: NSRange(location: Int(s.start), length: Int(s.len)), focused: s.focused != 0))
            }
            out.preedit = PreeditUpdate(text: text, caret: Int(o.preedit_caret), segments: segments)
        }
        if o.candidates_changed != 0 {
            if o.candidate_count == 0 {
                out.candidates = .hide
            } else {
                let items = (0..<Int(o.candidate_count)).compactMap { i in
                    o.candidates?[i].map { String(cString: $0) }
                }
                out.candidates = .show(
                    items: items,
                    selected: o.candidate_selected >= 0 ? Int(o.candidate_selected) : nil,
                    grid: o.candidate_grid != 0
                )
            }
        }
        return out
    }
}
