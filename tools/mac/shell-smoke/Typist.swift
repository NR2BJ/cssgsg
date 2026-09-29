// 맥 셸 스모크 테스트의 타자기. 셸의 KeyTranslation.swift(ModifierState), CoreEngine.swift,
// TextApplier.swift와 같이 빌드한다(run.sh).
//
// CGEvent로 만든 진짜 NSEvent를 IMKit 컨트롤러가 받는 순서대로 셸 코드에 넣고, 앱 화면을 흉내 낸다.
// 글자는 입력기와 같은 TextApplier가 가짜 문서(FakeDocument, NSTextInputClient 규칙)에 넣는다.
// 키열 문법과 화면 규칙은 러스트 시뮬레이터(core/src/sim.rs)와 같아서 cssgsg-cli batch와 줄마다 비교할 수 있다.
//
// 러스트 시뮬레이터와 일부러 다르게 한 것(실제 입력 흐름에 맞춘다)
// - 글자 → 맥 키코드, 그리고 앱이 넣는 글자는 macOS ABC 배열 데이터(UCKeyTranslate)에서 얻는다. 러스트 쪽 표와 독립.
// - Shift·⌘·Ctrl·Option은 수식키 눌림/뗌(flagsChanged)을 따로 보내고, 좌우 기기 비트(IOLLEvent.h)도 붙인다.
// - macOS 27의 IMKit처럼 좌우 기기 비트를 빼거나(deviceBits: false), flagsChanged를 두 번씩 보낼 수 있다
//   (duplicateFlags: true). 2026-09-29 개발자 기록에서 둘 다 실제로 봤다.
// - 글자 키의 뗌은 보내지 않는다(컨트롤러는 keyDown과 flagsChanged만 받는다).
// - 엔진이 Caps Lock을 끄라고 하면, OS가 보낼 Caps Lock flagsChanged를 다음 입력 전에 보낸다.
import Carbon
import Cocoa
import IOKit.hidsystem

struct SmokeError: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}

// MARK: - macOS ABC 배열

/// ABC 배열 데이터로 키코드 → 글자, 글자 → (키코드, Shift)를 만든다.
final class AbcLayout {
    private let data: Data
    let keyboardType: UInt32
    private(set) var keyFor: [Character: (code: UInt16, shift: Bool)] = [:]

    init() {
        let cond = [kTISPropertyInputSourceID as String: "com.apple.keylayout.ABC"] as CFDictionary
        guard let list = TISCreateInputSourceList(cond, true)?.takeRetainedValue() as? [TISInputSource],
              let source = list.first,
              let raw = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData)
        else { fatalError("ABC 배열(com.apple.keylayout.ABC)을 찾을 수 없다") }
        data = Unmanaged<CFData>.fromOpaque(raw).takeUnretainedValue() as Data
        keyboardType = UInt32(LMGetKbdType())
        // 본 자판(0x00~0x32)만 본다. 숫자판 키는 같은 글자를 내도 쓰지 않는다.
        for code in UInt16(0)...0x32 {
            for shift in [false, true] {
                if let c = char(code, shift: shift, caps: false), keyFor[c] == nil {
                    keyFor[c] = (code, shift)
                }
            }
        }
    }

    /// 키코드가 내는 글자(인쇄 가능한 ASCII 한 글자만). 제어 문자·기능 키는 nil.
    func char(_ code: UInt16, shift: Bool, caps: Bool) -> Character? {
        var mods: UInt32 = 0
        if shift { mods |= UInt32(shiftKey >> 8) & 0xFF }
        if caps { mods |= UInt32(alphaLock >> 8) & 0xFF }
        var dead: UInt32 = 0
        var length = 0
        var buffer = [UniChar](repeating: 0, count: 4)
        let status = data.withUnsafeBytes { bytes -> OSStatus in
            UCKeyTranslate(
                bytes.bindMemory(to: UCKeyboardLayout.self).baseAddress!, code, UInt16(kUCKeyActionDown), mods,
                keyboardType, OptionBits(kUCKeyTranslateNoDeadKeysMask), &dead, buffer.count, &length, &buffer)
        }
        guard status == noErr, length == 1, (0x20...0x7E).contains(buffer[0]),
              let scalar = Unicode.Scalar(buffer[0]) else { return nil }
        return Character(scalar)
    }
}

// MARK: - 이벤트

enum Flag {
    static let alphaShift = CGEventFlags.maskAlphaShift.rawValue
    static let shift = CGEventFlags.maskShift.rawValue
    static let control = CGEventFlags.maskControl.rawValue
    static let option = CGEventFlags.maskAlternate.rawValue
    static let command = CGEventFlags.maskCommand.rawValue
    /// 진짜 키 이벤트에 늘 붙어 있는 비트(NX_NONCOALSESCEDMASK).
    static let nonCoalesced = UInt64(NX_NONCOALSESCEDMASK)
    /// 좌우 기기 비트 전부.
    static let deviceMask = UInt64(NX_DEVICELSHIFTKEYMASK | NX_DEVICERSHIFTKEYMASK | NX_DEVICELCTLKEYMASK | NX_DEVICERCTLKEYMASK
        | NX_DEVICELALTKEYMASK | NX_DEVICERALTKEYMASK | NX_DEVICELCMDKEYMASK | NX_DEVICERCMDKEYMASK)
}

/// 수식키 하나: 맥 키코드(Events.h), 기기 비트와 같은 종류 양쪽 기기 비트(IOLLEvent.h), 합친 플래그.
struct Modifier {
    let code: UInt16
    let device: UInt64
    let pair: UInt64
    let flag: UInt64

    private static let shifts = UInt64(NX_DEVICELSHIFTKEYMASK | NX_DEVICERSHIFTKEYMASK)
    private static let controls = UInt64(NX_DEVICELCTLKEYMASK | NX_DEVICERCTLKEYMASK)
    private static let options = UInt64(NX_DEVICELALTKEYMASK | NX_DEVICERALTKEYMASK)
    private static let commands = UInt64(NX_DEVICELCMDKEYMASK | NX_DEVICERCMDKEYMASK)

    static let leftShift = Modifier(code: UInt16(kVK_Shift), device: UInt64(NX_DEVICELSHIFTKEYMASK), pair: shifts, flag: Flag.shift)
    static let rightShift = Modifier(code: UInt16(kVK_RightShift), device: UInt64(NX_DEVICERSHIFTKEYMASK), pair: shifts, flag: Flag.shift)
    static let leftControl = Modifier(code: UInt16(kVK_Control), device: UInt64(NX_DEVICELCTLKEYMASK), pair: controls, flag: Flag.control)
    static let rightControl = Modifier(code: UInt16(kVK_RightControl), device: UInt64(NX_DEVICERCTLKEYMASK), pair: controls, flag: Flag.control)
    static let leftOption = Modifier(code: UInt16(kVK_Option), device: UInt64(NX_DEVICELALTKEYMASK), pair: options, flag: Flag.option)
    static let rightOption = Modifier(code: UInt16(kVK_RightOption), device: UInt64(NX_DEVICERALTKEYMASK), pair: options, flag: Flag.option)
    static let leftCommand = Modifier(code: UInt16(kVK_Command), device: UInt64(NX_DEVICELCMDKEYMASK), pair: commands, flag: Flag.command)
    static let rightCommand = Modifier(code: UInt16(kVK_RightCommand), device: UInt64(NX_DEVICERCMDKEYMASK), pair: commands, flag: Flag.command)
}

/// CGEvent 시각 1단위가 몇 초인지. 문서는 나노초라 하지만 애플 실리콘은 mach 틱일 수 있어서 NSEvent에 넣어 보고 잰다.
let secondsPerTick: Double = {
    func stamp(_ ticks: UInt64) -> Double {
        let cg = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: true)!
        cg.timestamp = ticks
        return NSEvent(cgEvent: cg)!.timestamp
    }
    return (stamp(3_000_000_000) - stamp(1_000_000_000)) / 2_000_000_000
}()

func makeEvent(_ type: CGEventType, code: UInt16, flags: UInt64, at seconds: Double, autorepeat: Bool = false) -> NSEvent {
    let cg = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: type != .keyUp)!
    cg.type = type
    cg.flags = CGEventFlags(rawValue: flags)
    cg.timestamp = CGEventTimestamp((seconds / secondsPerTick).rounded())
    if autorepeat {
        cg.setIntegerValueField(.keyboardEventAutorepeat, value: 1)
    }
    return NSEvent(cgEvent: cg)!
}

// MARK: - 가짜 문서

/// NSTextInputClient 규칙대로 움직이는 문서. 커서는 늘 조합 글자 끝(=문서 끝)이다(러스트 시뮬레이터와 같다).
/// 앱마다 다른 점을 흉내 내는 스위치(입력기에 보이는 것만 바뀌고 문서 자체는 규칙대로 움직인다).
final class FakeDocument: TextClient {
    let storage = NSMutableString()
    private(set) var marked: NSRange?
    var selection = NSRange(location: 0, length: 0)
    /// replacementRange를 무시하는 앱.
    var ignoresReplacementRange = false
    /// 글자를 읽어 주지 않는 앱(Chromium은 조합 중에 조합 밖 글자를 주지 않는다).
    var readable = true
    /// 조합 글자 자리를 알려 주지 않는 앱.
    var reportsMarkedRange = true
    /// 조합 글자 자리를 틀리게 알려 주는 앱(이 위치라고 한다).
    var reportedMarkedLocation: Int?

    var string: String { storage as String }

    /// 바꿀 구간: replacementRange가 있으면 그것, 없으면 조합 글자, 그것도 없으면 선택.
    private func target(_ replacement: NSRange) -> NSRange {
        if replacement.location != NSNotFound && !ignoresReplacementRange { return replacement }
        return marked ?? selection
    }

    func insertText(_ text: String, replacementRange: NSRange) {
        let r = target(replacementRange)
        storage.replaceCharacters(in: r, with: text)
        marked = nil
        selection = NSRange(location: r.location + (text as NSString).length, length: 0)
    }

    func setMarkedText(_ text: Any, selectionRange: NSRange, replacementRange: NSRange) {
        let s = (text as? NSAttributedString)?.string ?? (text as? String) ?? ""
        let r = target(replacementRange)
        storage.replaceCharacters(in: r, with: s)
        let length = (s as NSString).length
        marked = length > 0 ? NSRange(location: r.location, length: length) : nil
        selection = NSRange(location: r.location + min(selectionRange.location, length), length: 0)
    }

    func markedRange() -> NSRange {
        guard reportsMarkedRange, let m = marked else { return NSRange(location: NSNotFound, length: 0) }
        return NSRange(location: reportedMarkedLocation ?? m.location, length: m.length)
    }
    func selectedRange() -> NSRange { selection }

    func substring(_ range: NSRange) -> String? {
        guard readable, range.location != NSNotFound, NSMaxRange(range) <= storage.length else { return nil }
        return storage.substring(with: range)
    }

    /// 앱이 받은 글자 키. 시뮬레이터처럼 확정 글자 끝(조합 글자 앞)에 넣는다.
    func typed(_ text: String) {
        let at = marked?.location ?? selection.location
        storage.insert(text, at: at)
        let n = (text as NSString).length
        if let m = marked { marked = NSRange(location: m.location + n, length: m.length) }
        selection.location += n
    }

    /// 앱이 받은 Backspace: 확정 글자 끝의 한 글자(유니코드 스칼라)를 지운다.
    func deleteBackward() {
        let end = marked?.location ?? selection.location
        guard end > 0 else { return }
        let low = (0xDC00...0xDFFF).contains(Int(storage.character(at: end - 1)))
        let n = low && end >= 2 ? 2 : 1
        storage.deleteCharacters(in: NSRange(location: end - n, length: n))
        if let m = marked { marked = NSRange(location: m.location - n, length: m.length) }
        selection.location -= n
    }
}

// MARK: - 타자기

/// 키열을 IMKit 이벤트 순서대로 셸 코드(ModifierState → CoreEngine)에 넣고 앱 화면을 흉내 낸다.
final class Typist {
    let engine = CoreEngine(configTOML: nil)
    let layout: AbcLayout
    let doc = FakeDocument()
    private(set) var mode: InputMode
    /// 마지막으로 엔진에 넣은 키 이벤트와 그 NSEvent(자체 점검용).
    private(set) var lastKey: CssgsgKeyEvent?
    private(set) var lastEvent: NSEvent?

    private var caps = false
    private var held: UInt64 = 0
    /// 컨트롤러의 modifiers와 같다(프로세스 전체에서 하나).
    private var modifiers = ModifierState()
    private var capsOffPending = false
    private var clock = 1.0
    /// 좌우 기기 비트를 붙일지(false면 macOS 27 IMKit처럼 합친 플래그만).
    let deviceBits: Bool
    /// flagsChanged를 4ms 간격으로 두 번씩 보낼지(macOS 27 IMKit).
    let duplicateFlags: Bool

    init(layout: AbcLayout, mode: InputMode, deviceBits: Bool = true, duplicateFlags: Bool = false) {
        self.layout = layout
        self.mode = mode
        self.deviceBits = deviceBits
        self.duplicateFlags = duplicateFlags
        apply(engine.setMode(mode))
    }

    var screen: String { doc.string }

    private var flags: UInt64 {
        let all = held | (caps ? Flag.alphaShift : 0) | Flag.nonCoalesced
        return deviceBits ? all : all & ~Flag.deviceMask
    }

    // MARK: 수식키

    func press(_ m: Modifier) {
        deliverPending()
        clock += 0.03
        held |= m.device | m.flag
        flagsChanged(m.code)
    }

    func release(_ m: Modifier, after: Double = 0.03) {
        clock += after
        held &= ~m.device
        if held & m.pair == 0 {
            held &= ~m.flag
        }
        flagsChanged(m.code)
    }

    func toggleCaps() {
        deliverPending()
        clock += 0.03
        caps.toggle()
        flagsChanged(UInt16(kVK_CapsLock))
    }

    private func flagsChanged(_ code: UInt16) {
        deliverFlags(makeEvent(.flagsChanged, code: code, flags: flags, at: clock))
        if duplicateFlags {
            deliverFlags(makeEvent(.flagsChanged, code: code, flags: flags, at: clock + 0.004))
        }
    }

    private func deliverFlags(_ event: NSEvent) {
        lastEvent = event
        guard let key = modifiers.flagsChanged(event) else { return }
        lastKey = key
        apply(engine.handle(key))
    }

    // MARK: 글자 키

    /// 키 하나를 누른다. 뗌은 보내지 않는다.
    func key(_ code: UInt16, autorepeat: Bool = false) {
        deliverPending()
        clock += 0.03
        let event = makeEvent(.keyDown, code: code, flags: flags, at: clock, autorepeat: autorepeat)
        let key = modifiers.keyDown(event)
        lastEvent = event
        lastKey = key
        var out = engine.handle(key)
        if let context = out.hanjaContext {
            // 입력기 컨트롤러와 같다: 앱 글자를 읽어 변환을 시작하고, 글자는 TextApplier가 이미 넣었다.
            let (result, trace) = TextApplier.beginHanja(context, engine: engine, doc: doc)
            lastHanja = trace
            applyNonText(result)
            out = result
        } else {
            apply(out)
        }
        clock += 0.03
        guard !out.consumed, event.modifierFlags.intersection([.command, .control, .option]).isEmpty else { return }
        // 엔진이 넘긴 키는 앱이 ABC 배열대로 처리한다.
        switch Int(code) {
        case kVK_Delete:
            doc.deleteBackward()
        case kVK_Return, kVK_ANSI_KeypadEnter:
            doc.typed("\n")
        case kVK_Tab:
            doc.typed("\t")
        default:
            if let c = layout.char(code, shift: event.modifierFlags.contains(.shift), caps: caps) {
                doc.typed(String(c))
            }
        }
    }

    /// 마지막 한자 변환 시작 과정(자체 점검용).
    private(set) var lastHanja: TextApplier.HanjaTrace?

    func click() {
        deliverPending()
        apply(engine.mouseDown())
    }

    private func apply(_ out: EngineOutput) {
        TextApplier.apply(out, to: doc)
        applyNonText(out)
    }

    private func applyNonText(_ out: EngineOutput) {
        if let m = out.mode {
            mode = m
        }
        if out.capsLockOff && caps {
            // 셸은 CapsLock.set(false)를 부르고, 그러면 OS가 Caps Lock flagsChanged를 보낸다.
            capsOffPending = true
        }
    }

    private func deliverPending() {
        guard capsOffPending else { return }
        capsOffPending = false
        clock += 0.01
        caps = false
        flagsChanged(UInt16(kVK_CapsLock))
    }

    // MARK: 키열

    /// 러스트 시뮬레이터와 같은 키열 문법(core/src/sim.rs 맨 위 설명).
    func type(_ keys: String) throws {
        var it = keys.makeIterator()
        while let c = it.next() {
            if c == "{" {
                var name = ""
                while true {
                    guard let ch = it.next() else { throw SmokeError("닫는 }가 없다: {\(name)") }
                    if ch == "}" { break }
                    name.append(ch)
                }
                try special(name)
            } else {
                guard let (code, shift) = layout.keyFor[c] else { throw SmokeError("키로 칠 수 없는 글자 \(c)") }
                if shift {
                    press(.leftShift)
                    key(code)
                    release(.leftShift)
                } else {
                    key(code)
                }
            }
        }
    }

    private func special(_ name: String) throws {
        if let dash = name.firstIndex(of: "-"), dash != name.startIndex {
            let rest = String(name[name.index(after: dash)...])
            let m: Modifier
            switch name[..<dash] {
            case "S": m = .leftShift
            case "M": m = .leftCommand
            case "C": m = .leftControl
            case "A": m = .leftOption
            default: throw SmokeError("알 수 없는 수식키 \(name)")
            }
            let code: UInt16
            if let named = Self.named[rest] {
                code = named
            } else if rest.count == 1, let k = layout.keyFor[rest.first!] {
                code = k.code
            } else {
                throw SmokeError("알 수 없는 키 \(rest)")
            }
            press(m)
            key(code)
            release(m)
            return
        }
        switch name {
        case "rs":
            press(.rightShift)
            release(.rightShift, after: 0.05)
        case "ls":
            press(.leftShift)
            release(.leftShift, after: 0.05)
        case "caps":
            toggleCaps()
        case "click":
            click()
        default:
            guard let code = Self.named[name] else { throw SmokeError("알 수 없는 특수 키 {\(name)}") }
            key(code)
        }
    }

    static let named: [String: UInt16] = [
        "sp": UInt16(kVK_Space), "space": UInt16(kVK_Space), "bs": UInt16(kVK_Delete),
        "ent": UInt16(kVK_Return), "enter": UInt16(kVK_Return), "esc": UInt16(kVK_Escape), "tab": UInt16(kVK_Tab),
        "left": UInt16(kVK_LeftArrow), "right": UInt16(kVK_RightArrow), "up": UInt16(kVK_UpArrow),
        "down": UInt16(kVK_DownArrow), "pgup": UInt16(kVK_PageUp), "pgdn": UInt16(kVK_PageDown),
        "del": UInt16(kVK_ForwardDelete),
    ]
}

/// cssgsg-cli의 json()과 같은 모양으로 적는다.
func json(_ s: String) -> String {
    var out = "\""
    for u in s.unicodeScalars {
        switch u {
        case "\"": out += "\\\""
        case "\\": out += "\\\\"
        case "\n": out += "\\n"
        case "\t": out += "\\t"
        case _ where u.value < 0x20: out += String(format: "\\u%04x", u.value)
        default: out.unicodeScalars.append(u)
        }
    }
    return out + "\""
}
