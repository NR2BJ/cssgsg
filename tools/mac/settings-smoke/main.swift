// 설정 앱 스모크 테스트. 설정 앱 소스(SettingsApp.swift의 @main만 빼고)를 그대로 붙여 빌드한다(run.sh).
//
//   settings-smoke                 단축키 녹화(가짜 키 이벤트를 앱에 보낸다)와 화면 글자, 사전 읽기 정리
//   settings-smoke --shots <폴더>   위에 더해 탭 × 화면 언어를 창 없이 그려 PNG로 남긴다(눈으로 확인)
//
// 설정 파일·사전은 읽기만 한다. 업데이트 확인은 하지 않는다(마지막 확인을 방금으로 둔다, 메모리에만).
import AppKit
import SwiftUI

var failures = 0
func check(_ ok: Bool, _ what: String) {
    print(ok ? "ok   \(what)" : "FAIL \(what)")
    if !ok { failures += 1 }
}

let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
let now = Date().timeIntervalSince1970
UserDefaults.standard.register(defaults: ["update.stable.lastCheck": now, "update.beta.lastCheck": now])

// MARK: - 단축키 녹화

/// 앱에 온 것처럼 이벤트를 보낸다. 녹화기는 로컬 모니터(sendEvent가 부른다)로 받는다.
func send(_ type: NSEvent.EventType, _ code: UInt16, _ flags: UInt, repeat isRepeat: Bool = false) {
    guard let event = NSEvent.keyEvent(
        with: type, location: .zero, modifierFlags: NSEvent.ModifierFlags(rawValue: flags),
        timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: 0, context: nil,
        characters: "", charactersIgnoringModifiers: "", isARepeat: isRepeat, keyCode: code)
    else { return }
    NSApp.sendEvent(event)
}

func spin(_ seconds: Double) {
    RunLoop.current.run(until: Date().addingTimeInterval(seconds))
}

// 기기 비트(NX_DEVICE*KEYMASK) + 합친 플래그
let shiftL: UInt = 0x02 | NSEvent.ModifierFlags.shift.rawValue
let shiftR: UInt = 0x04 | NSEvent.ModifierFlags.shift.rawValue
let controlL: UInt = 0x01 | NSEvent.ModifierFlags.control.rawValue
let optionL: UInt = 0x20 | NSEvent.ModifierFlags.option.rawValue
let optionR: UInt = 0x40 | NSEvent.ModifierFlags.option.rawValue
let commandL: UInt = 0x08 | NSEvent.ModifierFlags.command.rawValue

/// 녹화를 시작하고 `steps`를 보낸 뒤: 기록한 글자열("(취소)", 아직 녹화 중이면 "(녹화 중)"), 거부 까닭, 녹화 중인지.
func record(wait: Double = 0.3, _ steps: () -> Void) -> (result: String, rejected: String?, active: Bool) {
    let recorder = ShortcutRecording()
    var result: String?? = .none
    var rejected: String?
    recorder.start({ result = .some($0) }, rejected: { rejected = $0 })
    steps()
    spin(wait)
    let active = recorder.active
    recorder.stop()
    return (result.map { $0 ?? "(취소)" } ?? "(녹화 중)", rejected, active)
}

MainActor.assumeIsolated {
    UILanguage.active = .ko
    var r = record {
        send(.flagsChanged, 0x3C, shiftR)
        send(.flagsChanged, 0x3C, 0)
    }
    check(r.result == "tap:shift_right" && !r.active, "녹화: 오른쪽 Shift만 눌렀다 떼면 탭 (\(r.result))")
    r = record {
        send(.flagsChanged, 0x3B, controlL)
        send(.flagsChanged, 0x3B, 0)
    }
    check(r.result == "tap:control_left", "녹화: 왼쪽 Control 탭 (\(r.result))")
    r = record {
        send(.flagsChanged, 0x3D, optionR)
        send(.keyDown, 0x24, optionR)
    }
    check(r.result == "alt_right+enter", "녹화: 오른쪽 Option+Return → alt_right+enter(좌우를 가린다) (\(r.result))")
    r = record {
        // 기기 비트가 없는 이벤트: flagsChanged로 기억한 쪽을 쓴다
        send(.flagsChanged, 0x3D, NSEvent.ModifierFlags.option.rawValue)
        send(.keyDown, 0x24, NSEvent.ModifierFlags.option.rawValue)
    }
    check(r.result == "alt_right+enter", "녹화: 기기 비트가 없어도 누른 쪽을 기억한다 (\(r.result))")
    r = record {
        send(.flagsChanged, 0x3B, controlL)
        send(.flagsChanged, 0x38, controlL | shiftL)
        send(.keyDown, 0x31, controlL | shiftL)
    }
    check(r.result == "control_left+shift_left+space", "녹화: 왼쪽 Control+왼쪽 Shift+Space, 파일에 적는 순서 (\(r.result))")
    r = record {
        send(.flagsChanged, 0x37, commandL)
        send(.flagsChanged, 0x3A, commandL | optionL)
        send(.keyDown, 0x28, commandL | optionL)
    }
    check(r.result == "(녹화 중)" && r.rejected?.contains("⌘") == true, "녹화: ⌘ 조합은 거부하고 계속 녹화 (\(r.rejected ?? "-"))")
    r = record { send(.keyDown, 0x69, 0) }
    check(r.result == "f13", "녹화: F13은 수식키 없이도 된다 (\(r.result))")
    r = record { send(.keyDown, 0x35, 0) }
    check(r.result == "(취소)" && !r.active, "녹화: Esc는 취소")
    r = record { send(.keyDown, 0x00, 0) }
    check(r.result == "(녹화 중)" && r.active && r.rejected?.contains("A") == true,
          "녹화: 수식키 없는 A는 거부하고 계속 녹화 (\(r.rejected ?? "-"))")
    r = record { send(.keyDown, 0x31, 0) }
    check(r.result == "(녹화 중)" && r.rejected != nil, "녹화: 수식키 없는 Space도 거부")
    r = record {
        send(.flagsChanged, 0x38, shiftL)
        send(.flagsChanged, 0x3C, shiftL | shiftR)
        send(.flagsChanged, 0x3C, shiftL)
        send(.flagsChanged, 0x38, 0)
    }
    check(r.result == "(녹화 중)", "녹화: 수식키 둘을 겹쳐 누르면 탭이 아니다 (\(r.result))")
    r = record {
        send(.flagsChanged, 0x38, shiftL)
        send(.keyDown, 0x00, shiftL)
        send(.flagsChanged, 0x38, 0)
    }
    check(r.result == "shift_left+a", "녹화: Shift+A는 조합이고 그 뒤 뗌은 탭이 아니다 (\(r.result))")
    r = record {
        send(.flagsChanged, 0x3C, shiftR)
        send(.flagsChanged, 0x3C, 0)
        send(.keyDown, 0x60, 0)  // F5: 0.15초 안에 누른 다른 키
    }
    check(r.result == "f5", "녹화: 탭 뒤 0.15초 안에 다른 키를 누르면 그 키다 (\(r.result))")
    r = record {
        send(.flagsChanged, 0x3C, shiftR)
        send(.keyDown, 0x00, shiftR, repeat: true)
    }
    check(r.result == "(녹화 중)", "녹화: 자동 반복 키는 무시 (\(r.result))")

    // 화면 글자
    let labels: [(UILanguage, String, String)] = [
        (.ko, "tap:shift_right", "오른쪽 Shift 탭"), (.en, "tap:shift_right", "Right Shift tap"),
        (.ja, "tap:alt_left", "左Option タップ"), (.ko, "alt_left+enter", "왼쪽 Option + Return"),
        (.ko, "alt+enter", "Option + Return"), (.ja, "alt_right+enter", "右Option + Return"),
        (.en, "control_left+shift_right+space", "Left Control + Right Shift + Space"), (.en, "meta+f5", "Command + F5"),
        (.ko, "", "없음"), (.en, "", "None"), (.ja, "", "なし"), (.en, "alt+bracket_left", "Option + ["),
    ]
    for (lang, value, text) in labels {
        UILanguage.active = lang
        let shown = ShortcutText.label(value)
        check(shown == text, "화면 글자(\(lang.rawValue)): \"\(value)\" → \(shown)")
    }
    UILanguage.active = .ko

    // 사전: 읽기 정리와 품사 이름
    check(UserDictionaryModel.normalizedReading(" クモツ ") == "くもつ", "사전: 가타카나 읽기는 히라가나로, 앞뒤 공백 제거")
    check(UserDictionaryModel.normalizedReading("ヴぁー") == "ゔぁー", "사전: ヴ는 ゔ, 장음 부호 ー는 그대로")
    check(UserDictionaryModel.normalizedReading("ｶﾞｯｺｰ") == "がっこー", "사전: 반각 가타카나(탁점 합치기)")
    check(UserDictionaryModel.normalizedReading("ＡＢＣ１２ヵヶ") == "ABC12ゕゖ", "사전: 전각 영숫자는 반각, ヵヶ는 ゕゖ")
    UILanguage.active = .ja
    check(MozcPOS.name(1) == "名詞" && MozcPOS.name(35) == "動詞一段", "사전: 품사 이름(일본어)")
    UILanguage.active = .ko
    check(MozcPOS.name(1) == "명사 名詞" && MozcPOS.name(99) == "99", "사전: 품사 이름(한국어, 모르는 번호)")
    check(MozcPOS.choices.count == 18 && Set(MozcPOS.choices).count == 18, "사전: 고르는 품사 18개(NRIME와 같다)")
}

// MARK: - 화면 스냅숏

@MainActor
func render<V: View>(_ view: V, width: CGFloat, height: CGFloat, name: String, to out: URL) {
    let host = NSHostingView(rootView: view.frame(width: width, height: height))
    let window = NSWindow(contentRect: NSRect(x: -20000, y: -20000, width: width, height: height), styleMask: [.titled],
                          backing: .buffered, defer: false)
    window.contentView = host
    host.frame = NSRect(x: 0, y: 0, width: width, height: height)
    RunLoop.current.run(until: Date().addingTimeInterval(0.6))
    host.layoutSubtreeIfNeeded()
    guard let rep = host.bitmapImageRepForCachingDisplay(in: host.bounds) else { return }
    host.cacheDisplay(in: host.bounds, to: rep)
    try? rep.representation(using: .png, properties: [:])?.write(to: out.appendingPathComponent("\(name).png"))
}

let args = Array(CommandLine.arguments.dropFirst())
if let i = args.firstIndex(of: "--shots"), i + 1 < args.count {
    let out = URL(fileURLWithPath: args[i + 1], isDirectory: true)
    try? FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
    MainActor.assumeIsolated {
        let model = SettingsModel()
        let dictionary = UserDictionaryModel()
        let updater = Updater()
        for lang in UILanguage.allCases {
            UILanguage.active = lang
            let l = lang.rawValue
            render(GeneralTab(model: model), width: 740, height: 1100, name: "general-\(l)", to: out)
            render(KoreanTab(model: model), width: 740, height: 420, name: "korean-\(l)", to: out)
            render(JapaneseTab(model: model, dictionary: dictionary), width: 740, height: 1960, name: "japanese-\(l)", to: out)
            render(AboutTab(model: model, updater: updater, language: .constant(lang)), width: 740, height: 1000,
                   name: "about-\(l)", to: out)
        }
        print("스냅숏: \(out.path) (탭 4 × 언어 3)")
    }
}
print(failures == 0 ? "모두 통과" : "실패 \(failures)개")
exit(failures == 0 ? 0 : 1)
