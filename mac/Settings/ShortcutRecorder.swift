import AppKit
import SwiftUI

/// 단축키 글자열(설정 파일, core/src/shortcut.rs) ↔ 화면 글자.
/// - 탭: "tap:shift_right" → "오른쪽 Shift 탭"
/// - 조합: "control+alt+shift+meta+키" → "Control + Option + Return". 수식키는 좌우를 가리지 않는다.
/// - "": 없음
enum ShortcutText {
    /// 수식키 keyCode → 탭 이름(좌우를 가린다).
    static let modifierKeys: [UInt16: String] = [
        0x38: "shift_left", 0x3C: "shift_right", 0x3B: "control_left", 0x3E: "control_right",
        0x3A: "alt_left", 0x3D: "alt_right", 0x37: "meta_left", 0x36: "meta_right",
    ]

    /// 조합의 수식키 종류. 파일에 적는 순서와 같다.
    static let families: [(name: String, flag: NSEvent.ModifierFlags, label: String)] = [
        ("control", .control, "Control"), ("alt", .option, "Option"), ("shift", .shift, "Shift"), ("meta", .command, "Command"),
    ]

    /// 수식키 없이 단축키로 두면 그 키를 칠 수 없게 되는 키(코어 Shortcut::validate와 같다).
    static let needsModifier: Set<String> = [
        "space", "enter", "tab", "backspace", "escape", "minus", "equal", "bracket_left", "bracket_right", "backslash",
        "semicolon", "quote", "grave", "comma", "period", "slash",
    ]

    static func label(_ value: String) -> String {
        if value.isEmpty || value == "none" { return tr("없음", "None", "なし") }
        if value.hasPrefix("tap:") { return tapLabel(String(value.dropFirst(4))) }
        let parts = value.split(separator: "+").map(String.init)
        guard let key = parts.last else { return value }
        let mods = parts.dropLast().map { name in families.first { $0.name == name }?.label ?? name }
        return (mods + [keyLabel(key)]).joined(separator: " + ")
    }

    static func tapLabel(_ name: String) -> String {
        let parts = name.split(separator: "_").map(String.init)
        guard parts.count == 2 else { return name }
        let key = families.first { $0.name == parts[0] }?.label ?? parts[0]
        let right = parts[1] == "right"
        return right
            ? tr("오른쪽 \(key) 탭", "Right \(key) tap", "右\(key) タップ")
            : tr("왼쪽 \(key) 탭", "Left \(key) tap", "左\(key) タップ")
    }

    static func keyLabel(_ name: String) -> String {
        let named: [String: String] = [
            "enter": "Return", "keypad_enter": "Enter", "escape": "Esc", "backspace": "Delete", "delete": "⌦ Delete",
            "tab": "Tab", "space": "Space", "minus": "-", "equal": "=", "bracket_left": "[", "bracket_right": "]",
            "backslash": "\\", "semicolon": ";", "quote": "'", "grave": "`", "comma": ",", "period": ".", "slash": "/",
            "home": "Home", "end": "End", "page_up": "Page Up", "page_down": "Page Down",
            "left": "←", "right": "→", "up": "↑", "down": "↓",
        ]
        return named[name] ?? name.uppercased()
    }

    static func actionName(_ path: WritableKeyPath<CssgsgConfig.Shortcuts, String>) -> String {
        if path == \CssgsgConfig.Shortcuts.toggleEnglish {
            return tr("영어 ↔ 비영어", "English ↔ non-English", "英語 ↔ 英語以外")
        }
        if path == \CssgsgConfig.Shortcuts.toggleNonEnglish {
            return tr("한국어 ↔ 일본어", "Korean ↔ Japanese", "韓国語 ↔ 日本語")
        }
        return tr("한자 변환", "Hanja conversion", "ハンジャ変換（韓国語の漢字）")
    }
}

/// 단축키 한 줄: 이름, 지금 단축키(누르면 녹화), 지우기, 기본값.
struct ShortcutRow: View {
    let title: String
    let value: String
    let defaultValue: String
    /// 새 단축키를 준다. 받아 주지 않으면 까닭을 돌려준다.
    let set: (String) -> String?

    @StateObject private var recorder = ShortcutRecording()
    @State private var problem: String?

    var body: some View {
        LabeledContent(title) {
            VStack(alignment: .trailing, spacing: 4) {
                HStack(spacing: 6) {
                    Button {
                        problem = nil
                        if recorder.active {
                            recorder.stop()
                        } else {
                            recorder.start { recorded in
                                if let recorded { problem = set(recorded) }
                            } rejected: { reason in
                                problem = reason
                            }
                        }
                    } label: {
                        Text(recorder.active ? tr("키를 누른다… (Esc 취소)", "Press keys… (Esc to cancel)", "キーを押す…（Esc で取消）")
                             : ShortcutText.label(value))
                            .frame(minWidth: 180)
                            .foregroundStyle(recorder.active ? Color.accentColor : (value.isEmpty ? Color.secondary : Color.primary))
                    }
                    Button {
                        recorder.stop()
                        problem = set("")
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                    }
                    .buttonStyle(.borderless)
                    .foregroundStyle(.secondary)
                    .help(tr("단축키 없음", "No shortcut", "ショートカットなし"))
                    .disabled(value.isEmpty)
                    Button {
                        recorder.stop()
                        problem = set(defaultValue)
                    } label: {
                        Image(systemName: "arrow.uturn.backward.circle")
                    }
                    .buttonStyle(.borderless)
                    .foregroundStyle(.secondary)
                    .help(tr("기본값: \(ShortcutText.label(defaultValue))", "Default: \(ShortcutText.label(defaultValue))",
                             "既定: \(ShortcutText.label(defaultValue))"))
                    .disabled(value == defaultValue)
                }
                if let problem {
                    Text(problem).font(.caption).foregroundStyle(.orange)
                }
            }
        }
        .onDisappear { recorder.stop() }
    }
}

/// 단축키 녹화(NRIME와 같은 방식). 이 앱 창에 온 키를 가로채서 쓰고, 녹화하는 동안 창에는 넘기지 않는다(⌘W 등).
/// - 수식키 + 다른 키: 다른 키를 누르는 순간 조합으로 기록한다.
/// - 수식키 하나만 눌렀다 떼면(사이에 다른 키·수식키 없이) 0.15초 뒤 탭으로 기록한다. 그새 다른 키를 누르면 탭이 아니다.
/// - 수식키 없이 Esc: 취소.
/// 입력기는 이 창의 단추에 키를 보내지 않는다(글자 입력 칸이 아니다). 그래서 이미 쓰는 단축키도 녹화된다.
final class ShortcutRecording: ObservableObject {
    @Published private(set) var active = false

    private var monitor: Any?
    private var done: ((String?) -> Void)?
    private var rejected: ((String) -> Void)?
    /// 지금 눌려 있는 수식키(keyCode).
    private var held: Set<UInt16> = []
    /// 탭이 될 수 있는 수식키: 아무것도 누르지 않은 때 혼자 눌렀고, 그 뒤로 다른 키·수식키가 없었다.
    private var tapCandidate: UInt16?
    private var pendingTap: DispatchWorkItem?

    func start(_ done: @escaping (String?) -> Void, rejected: @escaping (String) -> Void) {
        stop()
        self.done = done
        self.rejected = rejected
        held = []
        tapCandidate = nil
        active = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .flagsChanged]) { [weak self] event in
            self?.handle(event)
            return nil
        }
    }

    func stop() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        pendingTap?.cancel()
        pendingTap = nil
        done = nil
        rejected = nil
        if active { active = false }
    }

    private func finish(_ value: String?) {
        let done = self.done
        stop()
        done?(value)
    }

    private func handle(_ event: NSEvent) {
        switch event.type {
        case .keyDown:
            pendingTap?.cancel()
            pendingTap = nil
            tapCandidate = nil
            let flags = event.modifierFlags
            let families = ShortcutText.families.filter { flags.contains($0.flag) }
            if event.keyCode == 0x35 && families.isEmpty {
                finish(nil)  // Esc: 취소
                return
            }
            guard !event.isARepeat, let pointer = cssgsg_config_key_name(event.keyCode) else { return }
            let key = String(cString: pointer)
            if families.isEmpty && (ShortcutText.needsModifier.contains(key) || key.count == 1) {
                rejected?(tr("\(ShortcutText.keyLabel(key))만으로는 쓸 수 없다(그 키를 칠 수 없게 된다). 수식키와 같이 누른다.",
                             "\(ShortcutText.keyLabel(key)) alone can’t be a shortcut (you couldn’t type it). Hold a modifier with it.",
                             "\(ShortcutText.keyLabel(key)) だけではショートカットにできません（その文字が打てなくなります）。修飾キーと一緒に押します。"))
                return
            }
            finish((families.map(\.name) + [key]).joined(separator: "+"))
        case .flagsChanged:
            guard let name = ShortcutText.modifierKeys[event.keyCode] else { return }  // Caps Lock, fn
            let code = event.keyCode
            let down = isDown(code, event.modifierFlags)
            if down {
                pendingTap?.cancel()
                pendingTap = nil
                tapCandidate = held.isEmpty ? code : nil
                held.insert(code)
            } else {
                held.remove(code)
                guard held.isEmpty, tapCandidate == code else {
                    tapCandidate = nil
                    return
                }
                tapCandidate = nil
                let work = DispatchWorkItem { [weak self] in self?.finish("tap:" + name) }
                pendingTap = work
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.15, execute: work)
            }
        default:
            break
        }
    }

    /// 수식키가 눌렸는지. 앱에 오는 flagsChanged에는 좌우 기기 비트(NX_DEVICE*KEYMASK)가 있어서 그것을 본다.
    /// 없으면 합친 플래그로 본다(같은 종류 양쪽을 같이 누르면 틀릴 수 있지만 그때는 탭이 아니다).
    private func isDown(_ code: UInt16, _ flags: NSEvent.ModifierFlags) -> Bool {
        let device: [UInt16: UInt] = [
            0x38: 0x02, 0x3C: 0x04, 0x3B: 0x01, 0x3E: 0x2000, 0x3A: 0x20, 0x3D: 0x40, 0x37: 0x08, 0x36: 0x10,
        ]
        let family: [UInt16: NSEvent.ModifierFlags] = [
            0x38: .shift, 0x3C: .shift, 0x3B: .control, 0x3E: .control, 0x3A: .option, 0x3D: .option, 0x37: .command, 0x36: .command,
        ]
        guard let bit = device[code], let flag = family[code] else { return false }
        let pair = device.filter { family[$0.key] == flag }.values.reduce(0, |)
        let raw = flags.rawValue
        return raw & pair != 0 ? raw & bit != 0 : flags.contains(flag)
    }
}
