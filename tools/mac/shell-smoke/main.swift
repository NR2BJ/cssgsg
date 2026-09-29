// 맥 셸 스모크 테스트. 사용법은 run.sh 참고.
//   shell-smoke --self-test                 수식키 좌우·눌림/뗌·중복 판정, 탭 시각, 반복 키, ABC 배열 표 확인
//   shell-smoke [--mode en|ko|ja] [--no-device-bits] [--duplicate-flags] < 키열
//                                           줄마다 화면(확정 글자 + 조합 중 글자)을 JSON 문자열 한 줄로
import Carbon
import Cocoa
import IOKit.hidsystem

func selfTest() -> Bool {
    var ok = true
    func check(_ cond: Bool, _ what: String) {
        print(cond ? "ok   \(what)" : "FAIL \(what)")
        if !cond { ok = false }
    }
    let layout = AbcLayout()
    let physical = KBGetLayoutType(Int16(layout.keyboardType))
    let physicalName = physical == UInt32(kKeyboardANSI) ? "ANSI" : physical == UInt32(kKeyboardISO) ? "ISO" : "JIS"
    print(String(format: "CGEvent 시각 1단위 = %.4fns, 키보드 종류 %u(%@)", secondsPerTick * 1e9, layout.keyboardType, physicalName))

    // ABC 배열 표
    let printable = (0x20...0x7E).compactMap { Unicode.Scalar($0).map(Character.init) }
    check(printable.allSatisfy { layout.keyFor[$0] != nil }, "ABC 배열에서 인쇄 가능한 ASCII 95자 모두 키를 찾음")
    check(layout.keyFor["`"]?.code == UInt16(kVK_ANSI_Grave), "` 는 kVK_ANSI_Grave (ANSI 기준)")

    // 수식키 좌우(기기 비트가 있을 때): NSEvent가 기기 비트를 지키는지 + ModifierState.mods
    let (ls, rs) = (UInt64(NX_DEVICELSHIFTKEYMASK), UInt64(NX_DEVICERSHIFTKEYMASK))
    func mods(_ f: UInt64) -> UInt32 {
        ModifierState().mods(makeEvent(.keyDown, code: 0, flags: f | Flag.nonCoalesced, at: 1).modifierFlags)
    }
    check(mods(Flag.shift | rs) == UInt32(CSSGSG_MOD_SHIFT_R), "오른쪽 Shift 기기 비트 → SHIFT_R")
    check(mods(Flag.shift | ls) == UInt32(CSSGSG_MOD_SHIFT_L), "왼쪽 Shift 기기 비트 → SHIFT_L")
    check(mods(Flag.shift | ls | rs) == UInt32(CSSGSG_MOD_SHIFT_L | CSSGSG_MOD_SHIFT_R), "양쪽 Shift → 둘 다")
    check(mods(Flag.shift) == UInt32(CSSGSG_MOD_SHIFT_L), "기기 비트도 기억도 없는 Shift → 왼쪽으로")
    check(mods(Flag.command | UInt64(NX_DEVICERCMDKEYMASK)) == UInt32(CSSGSG_MOD_META_R), "오른쪽 ⌘ → META_R")
    check(mods(Flag.control | UInt64(NX_DEVICERCTLKEYMASK)) == UInt32(CSSGSG_MOD_CTRL_R), "오른쪽 Control → CTRL_R")
    check(mods(Flag.option | UInt64(NX_DEVICERALTKEYMASK)) == UInt32(CSSGSG_MOD_ALT_R), "오른쪽 Option → ALT_R")
    check(mods(Flag.option | UInt64(NX_DEVICELALTKEYMASK)) == UInt32(CSSGSG_MOD_ALT_L), "왼쪽 Option → ALT_L")
    check(mods(Flag.alphaShift) == UInt32(CSSGSG_MOD_CAPS), "Caps Lock → CAPS")

    // flagsChanged: 상태를 기억하는 판정. 두 번째 인자는 이벤트의 플래그.
    func fc(_ state: inout ModifierState, _ code: Int, _ f: UInt64) -> CssgsgKeyEvent? {
        state.flagsChanged(makeEvent(.flagsChanged, code: UInt16(code), flags: f | Flag.nonCoalesced, at: 1))
    }
    let (sR, sL) = (UInt32(CSSGSG_MOD_SHIFT_R), UInt32(CSSGSG_MOD_SHIFT_L))
    do {  // 기기 비트가 있을 때
        var st = ModifierState()
        let d = fc(&st, kVK_RightShift, Flag.shift | rs)
        check(d?.down == 1 && d?.key == 0xE5 && d?.mods == sR, "기기 비트: 오른쪽 Shift 눌림 → 0xE5, SHIFT_R")
        let l = fc(&st, kVK_Shift, Flag.shift | ls | rs)
        check(l?.down == 1 && l?.key == 0xE1 && l?.mods == sL | sR, "기기 비트: 오른쪽을 쥔 채 왼쪽 눌림")
        check(fc(&st, kVK_RightShift, Flag.shift | ls)?.down == 0, "기기 비트: 왼쪽을 쥔 채 오른쪽 뗌도 안다")
        check(fc(&st, kVK_Shift, 0)?.down == 0, "기기 비트: 왼쪽 뗌")
        let c = fc(&st, kVK_RightCommand, Flag.command | UInt64(NX_DEVICERCMDKEYMASK))
        check(c?.down == 1 && c?.key == 0xE7, "기기 비트: 오른쪽 ⌘ 눌림 → 0xE7")
    }
    do {  // macOS 27 IMKit: 기기 비트 없음
        var st = ModifierState()
        let d = fc(&st, kVK_RightShift, Flag.shift)
        check(d?.down == 1 && d?.key == 0xE5 && d?.mods == sR, "기기 비트 없음: 오른쪽 Shift 눌림 → SHIFT_R(keyCode로 기억)")
        check(fc(&st, kVK_RightShift, Flag.shift) == nil, "기기 비트 없음: 같은 눌림이 또 오면(중복) 버린다")
        let key = st.keyDown(makeEvent(.keyDown, code: UInt16(kVK_ANSI_K), flags: Flag.shift | Flag.nonCoalesced, at: 1))
        check(key.mods == sR, "기기 비트 없음: 오른쪽 Shift를 쥔 글자 → SHIFT_R")
        let u = fc(&st, kVK_RightShift, 0)
        check(u?.down == 0 && u?.mods == 0, "기기 비트 없음: 오른쪽 Shift 뗌")
        check(fc(&st, kVK_RightShift, 0) == nil, "기기 비트 없음: 같은 뗌이 또 오면(중복) 버린다")
        // 양쪽 Shift: 먼저 뗀 쪽은 알 수 없지만, 다 떼면 기억이 비워진다
        _ = fc(&st, kVK_Shift, Flag.shift)
        let both = fc(&st, kVK_RightShift, Flag.shift)
        check(both?.down == 1 && both?.mods == sL | sR, "기기 비트 없음: 왼쪽을 쥔 채 오른쪽 눌림 → 둘 다")
        check(fc(&st, kVK_RightShift, Flag.shift) == nil, "기기 비트 없음: 왼쪽을 쥔 채 오른쪽 뗌은 알 수 없다(알려진 한계)")
        check(fc(&st, kVK_Shift, 0)?.down == 0 && st.held.isEmpty, "기기 비트 없음: 다 떼면 좌우 기억이 비워진다")
    }
    do {  // Caps Lock: 켜짐/꺼짐이 바뀔 때만
        var st = ModifierState()
        let on = fc(&st, kVK_CapsLock, Flag.alphaShift)
        check(on?.key == 0x39 && on?.down == 1 && (on?.mods ?? 0) & UInt32(CSSGSG_MOD_CAPS) != 0, "Caps Lock 켬 → 0x39 + CAPS")
        check(fc(&st, kVK_CapsLock, Flag.alphaShift) == nil, "Caps Lock 켬 중복은 버린다")
        check(fc(&st, kVK_CapsLock, 0) != nil, "Caps Lock 끔은 넘긴다")
        var fresh = ModifierState()
        check(fc(&fresh, kVK_Function, UInt64(NSEvent.ModifierFlags.function.rawValue)) == nil, "fn 키는 넘기지 않음")
    }

    // 탭 판정: 네 가지 입력 조건(기기 비트 있음/없음 × flagsChanged 한 번/두 번) 모두에서
    for (bits, dup) in [(true, false), (false, false), (true, true), (false, true)] {
        let tag = "[\(bits ? "비트" : "비트없음")\(dup ? "·두번" : "")]"
        func tap(_ m: Modifier, hold: Double, from mode: InputMode) -> InputMode {
            let t = Typist(layout: layout, mode: mode, deviceBits: bits, duplicateFlags: dup)
            t.press(m)
            let downTime = t.lastEvent!.timestamp
            t.release(m, after: hold)
            let measured = t.lastEvent!.timestamp - downTime
            if abs(measured - hold) > 1e-6 { print(String(format: "     (NSEvent 시각 차 %.6f초, 기대 %.3f초)", measured, hold)) }
            return t.mode
        }
        check(tap(.rightShift, hold: 0.05, from: .ko) == .en, "\(tag) 오른쪽 Shift 50ms 탭: 한 → 영")
        check(tap(.rightShift, hold: 0.05, from: .en) == .ko, "\(tag) 오른쪽 Shift 50ms 탭: 영 → 한(앞 언어)")
        check(tap(.leftShift, hold: 0.05, from: .ko) == .ja, "\(tag) 왼쪽 Shift 50ms 탭: 한 → 일")
        check(tap(.rightShift, hold: 0.19, from: .ko) == .en, "\(tag) 190ms 탭은 전환")
        check(tap(.rightShift, hold: 0.21, from: .ko) == .ko, "\(tag) 210ms 누름은 전환 안 함")
        check(tap(.rightShift, hold: 0.5, from: .ja) == .ja, "\(tag) 500ms 누름은 전환 안 함")
        do {
            let t = Typist(layout: layout, mode: .ko, deviceBits: bits, duplicateFlags: dup)
            t.press(.rightShift)
            t.key(UInt16(kVK_ANSI_K))
            t.release(.rightShift)
            check(t.mode == .ko, "\(tag) 오른쪽 Shift+글자는 전환 안 함")
        }
        do {
            let t = Typist(layout: layout, mode: .ko, deviceBits: bits, duplicateFlags: dup)
            t.press(.rightShift)
            t.click()
            t.release(.rightShift)
            check(t.mode == .ko, "\(tag) 누르는 동안 마우스 클릭하면 전환 안 함")
        }
        do {
            let t = Typist(layout: layout, mode: .ko, deviceBits: bits, duplicateFlags: dup)
            t.press(.leftShift)
            t.press(.rightShift)
            t.release(.rightShift)
            t.release(.leftShift)
            check(t.mode == .ko, "\(tag) 양쪽 Shift를 같이 누르면 전환 안 함")
        }
    }

    // 반복 키
    do {
        let t = Typist(layout: layout, mode: .ko)
        t.key(UInt16(kVK_ANSI_K), autorepeat: true)
        check(t.lastKey?.is_repeat == 1, "자동 반복 → is_repeat")
        t.key(UInt16(kVK_ANSI_K))
        check(t.lastKey?.is_repeat == 0, "보통 누름 → is_repeat 0")
    }

    // 한자 변환(참신 v18: 대 is, 한 hfs, 민 uds, 국 kre, 전 nvs, 기 kd, ㅁ u): 조합 중인 글자 하나만 바꾼다.
    // 입력기와 같은 TextApplier가 가짜 문서에 넣는다. 앞 글자는 건드리지 않는다.
    func typed(_ keys: String) -> Typist {
        let t = Typist(layout: layout, mode: .ko)
        do { try t.type(keys) } catch { print("     키열 오류: \(error)") }
        return t
    }
    let none = NSRange(location: NSNotFound, length: 0)
    do {
        let t = typed("ishfsudskre{A-ent}")
        check(t.screen == "대한민國" && t.doc.markedRange() == NSRange(location: 3, length: 1),
              "한자: 조합 중인 국만 國(조합 3~4), 앞 글자는 그대로")
        try? t.type("{sp}")
        check(t.screen == "대한민局" && t.doc.markedRange() == NSRange(location: 3, length: 1), "한자: 다음 후보")
        try? t.type("{esc}")
        check(t.screen == "대한민국" && t.doc.markedRange() == NSRange(location: 3, length: 1), "한자: Esc로 국 조합이 돌아옴")
        try? t.type("{A-ent}{ent}")
        check(t.screen == "대한민國" && t.doc.markedRange() == none, "한자: Enter로 확정")
    }
    do {
        let t = typed("u{A-ent}6")
        check(t.screen == "※" && t.doc.markedRange() == none, "한자: 자음 하나 + 한자 키는 기호(ㅁ 6번 ※)")
        let u = typed("nvskd{right}{A-ent}")
        check(u.screen == "전기" && u.doc.markedRange() == none, "한자: 조합이 없으면 한자 키는 앱으로(글자 그대로)")
    }
    return ok
}

let args = Array(CommandLine.arguments.dropFirst())
if args.contains("--self-test") {
    exit(selfTest() ? 0 : 1)
}
var mode = InputMode.ko
if let i = args.firstIndex(of: "--mode"), i + 1 < args.count {
    switch args[i + 1] {
    case "en": mode = .en
    case "ko": mode = .ko
    case "ja": mode = .ja
    default:
        FileHandle.standardError.write("모드는 en|ko|ja\n".data(using: .utf8)!)
        exit(2)
    }
}
let deviceBits = !args.contains("--no-device-bits")
let duplicateFlags = args.contains("--duplicate-flags")
let layout = AbcLayout()
var output = ""
while let line = readLine(strippingNewline: true) {
    let typist = Typist(layout: layout, mode: mode, deviceBits: deviceBits, duplicateFlags: duplicateFlags)
    do {
        try typist.type(line)
        output += json(typist.screen) + "\n"
    } catch {
        output += json("ERROR: \(error)") + "\n"
    }
}
FileHandle.standardOutput.write(output.data(using: .utf8)!)
