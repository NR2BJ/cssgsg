// 맥 셸 스모크 테스트. 사용법은 run.sh 참고.
//   shell-smoke --self-test                 수식키 좌우·눌림/뗌 판정, 시각 단위, 반복 키, ABC 배열 표 확인
//   shell-smoke [--mode en|ko|ja] < 키열    줄마다 화면(확정 글자 + 조합 중 글자)을 JSON 문자열 한 줄로
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

    // 수식키 좌우: NSEvent가 기기 비트를 지키는지 + KeyTranslation.mods
    let (ls, rs) = (UInt64(NX_DEVICELSHIFTKEYMASK), UInt64(NX_DEVICERSHIFTKEYMASK))
    func mods(_ f: UInt64) -> UInt32 {
        KeyTranslation.mods(makeEvent(.keyDown, code: 0, flags: f | Flag.nonCoalesced, at: 1).modifierFlags)
    }
    check(mods(Flag.shift | rs) == UInt32(CSSGSG_MOD_SHIFT_R), "오른쪽 Shift → SHIFT_R")
    check(mods(Flag.shift | ls) == UInt32(CSSGSG_MOD_SHIFT_L), "왼쪽 Shift → SHIFT_L")
    check(mods(Flag.shift | ls | rs) == UInt32(CSSGSG_MOD_SHIFT_L | CSSGSG_MOD_SHIFT_R), "양쪽 Shift → 둘 다")
    check(mods(Flag.shift) == UInt32(CSSGSG_MOD_SHIFT_L), "기기 비트 없는 Shift(합성 이벤트) → 왼쪽으로")
    check(mods(Flag.command | UInt64(NX_DEVICERCMDKEYMASK)) == UInt32(CSSGSG_MOD_META_R), "오른쪽 ⌘ → META_R")
    check(mods(Flag.control | UInt64(NX_DEVICERCTLKEYMASK)) == UInt32(CSSGSG_MOD_CTRL_R), "오른쪽 Control → CTRL_R")
    check(mods(Flag.option | UInt64(NX_DEVICERALTKEYMASK)) == UInt32(CSSGSG_MOD_ALT_R), "오른쪽 Option → ALT_R")
    check(mods(Flag.option | UInt64(NX_DEVICELALTKEYMASK)) == UInt32(CSSGSG_MOD_ALT_L), "왼쪽 Option → ALT_L")
    check(mods(Flag.alphaShift) == UInt32(CSSGSG_MOD_CAPS), "Caps Lock → CAPS")

    // flagsChanged 눌림/뗌 판정
    func fc(_ code: Int, _ f: UInt64, previous: UInt64) -> CssgsgKeyEvent? {
        KeyTranslation.flagsChanged(
            makeEvent(.flagsChanged, code: UInt16(code), flags: f | Flag.nonCoalesced, at: 1),
            previous: NSEvent.ModifierFlags(rawValue: UInt(previous)))
    }
    let rsDown = fc(kVK_RightShift, Flag.shift | rs, previous: 0)
    check(rsDown?.down == 1 && rsDown?.key == 0xE5, "오른쪽 Shift 눌림 → HID 0xE5 눌림")
    check(fc(kVK_RightShift, Flag.shift | ls, previous: Flag.shift | ls | rs)?.down == 0, "왼쪽 Shift를 쥔 채 오른쪽 Shift 뗌 → 뗌")
    let lsDown = fc(kVK_Shift, Flag.shift | ls | rs, previous: Flag.shift | rs)
    check(lsDown?.down == 1 && lsDown?.key == 0xE1, "오른쪽 Shift를 쥔 채 왼쪽 Shift 눌림 → HID 0xE1 눌림")
    check(fc(kVK_Shift, 0, previous: Flag.shift | ls)?.down == 0, "왼쪽 Shift 뗌")
    check(fc(kVK_Shift, Flag.shift, previous: 0)?.down == 1, "기기 비트 없는 Shift 눌림")
    check(fc(kVK_Shift, 0, previous: Flag.shift)?.down == 0, "기기 비트 없는 Shift 뗌")
    let rcDown = fc(kVK_RightCommand, Flag.command | UInt64(NX_DEVICERCMDKEYMASK), previous: 0)
    check(rcDown?.down == 1 && rcDown?.key == 0xE7, "오른쪽 ⌘ 눌림 → HID 0xE7")
    let capsEvent = fc(kVK_CapsLock, Flag.alphaShift, previous: 0)
    check(capsEvent?.key == 0x39 && capsEvent?.down == 1 && (capsEvent?.mods ?? 0) & UInt32(CSSGSG_MOD_CAPS) != 0,
          "Caps Lock 켬 → HID 0x39 눌림 + CAPS")
    check(fc(kVK_Function, UInt64(NSEvent.ModifierFlags.function.rawValue), previous: 0) == nil, "fn 키는 넘기지 않음")

    // 시각: 탭 판정(기본 200ms)이 NSEvent 시각 단위로 맞게 돈다
    func tap(_ m: Modifier, hold: Double, from mode: InputMode) -> InputMode {
        let t = Typist(layout: layout, mode: mode)
        t.press(m)
        let downTime = t.lastEvent!.timestamp
        t.release(m, after: hold)
        let measured = t.lastEvent!.timestamp - downTime
        if abs(measured - hold) > 1e-6 { print(String(format: "     (NSEvent 시각 차 %.6f초, 기대 %.3f초)", measured, hold)) }
        return t.mode
    }
    check(tap(.rightShift, hold: 0.05, from: .ko) == .en, "오른쪽 Shift 50ms 탭: 한 → 영")
    check(tap(.rightShift, hold: 0.05, from: .en) == .ko, "오른쪽 Shift 50ms 탭: 영 → 한(앞 언어)")
    check(tap(.leftShift, hold: 0.05, from: .ko) == .ja, "왼쪽 Shift 50ms 탭: 한 → 일")
    check(tap(.rightShift, hold: 0.19, from: .ko) == .en, "190ms 탭은 전환")
    check(tap(.rightShift, hold: 0.21, from: .ko) == .ko, "210ms 누름은 전환 안 함")
    check(tap(.rightShift, hold: 0.5, from: .ja) == .ja, "500ms 누름은 전환 안 함")
    do {
        let t = Typist(layout: layout, mode: .ko)
        t.press(.rightShift)
        t.key(UInt16(kVK_ANSI_K))
        t.release(.rightShift)
        check(t.mode == .ko, "오른쪽 Shift+글자는 전환 안 함")
    }
    do {
        let t = Typist(layout: layout, mode: .ko)
        t.press(.rightShift)
        t.click()
        t.release(.rightShift)
        check(t.mode == .ko, "누르는 동안 마우스 클릭하면 전환 안 함")
    }

    // 반복 키
    do {
        let t = Typist(layout: layout, mode: .ko)
        t.key(UInt16(kVK_ANSI_K), autorepeat: true)
        check(t.lastKey?.is_repeat == 1, "자동 반복 → is_repeat")
        t.key(UInt16(kVK_ANSI_K))
        check(t.lastKey?.is_repeat == 0, "보통 누름 → is_repeat 0")
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
let layout = AbcLayout()
var output = ""
while let line = readLine(strippingNewline: true) {
    let typist = Typist(layout: layout, mode: mode)
    do {
        try typist.type(line)
        output += json(typist.screen) + "\n"
    } catch {
        output += json("ERROR: \(error)") + "\n"
    }
}
FileHandle.standardOutput.write(output.data(using: .utf8)!)
