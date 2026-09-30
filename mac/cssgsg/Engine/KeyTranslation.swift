import Cocoa

/// NSEvent → 엔진 키 이벤트.
///
/// IMKit이 넘기는 이벤트에서 실제로 본 것(macOS 27, 2026-09-29 개발자 기록):
/// - flagsChanged에 좌우 기기 비트(IOLLEvent.h NX_DEVICE*KEYMASK)가 없다. 오른쪽 Shift도 합친 .shift만 온다.
/// - 같은 flagsChanged가 3~5ms 간격으로 두 번 온다(keyDown은 한 번). 114개 중 43쌍.
/// 합친 플래그만 보고 "모르면 왼쪽"으로 치면 오른쪽 Shift 탭이 "왼쪽 Shift를 쥔 채 오른쪽 Shift"가 되고,
/// 중복 눌림은 "다른 키가 끼어들었다"가 되어 탭이 모두 사라졌다(0.1.2까지).
///
/// 그래서 수식키 상태를 기억한다(NRIME ShortcutHandler와 같은 방식).
/// - 좌우는 flagsChanged의 keyCode로 기억한다. 기기 비트가 있으면 그것을 그대로 믿는다.
/// - 눌림/뗌은 기억한 상태와 비교해서 바뀐 것만 넘긴다. 중복은 버린다.
/// - 합친 플래그가 꺼진 계열은 좌우 기억을 지운다(다른 곳에서 뗀 것을 놓쳐도 되돌아온다).
/// - 기기 비트 없이 양쪽을 같이 누르면, 먼저 뗀 쪽의 뗌은 알 수 없다(합친 플래그가 그대로라서). 탭 판정에는 지장이 없다.
///
/// 프로세스에 하나만 두고 모든 컨트롤러가 같이 쓴다(IMKit은 클라이언트마다 컨트롤러를 따로 만든다).
struct ModifierState {
    private struct Family {
        let left: UInt16
        let right: UInt16
        let leftDevice: UInt
        let rightDevice: UInt
        let flag: NSEvent.ModifierFlags
        let leftBit: UInt32
        let rightBit: UInt32
    }

    private static let families: [Family] = [
        Family(left: 0x38, right: 0x3C, leftDevice: 0x0002, rightDevice: 0x0004, flag: .shift,
               leftBit: UInt32(CSSGSG_MOD_SHIFT_L), rightBit: UInt32(CSSGSG_MOD_SHIFT_R)),
        Family(left: 0x3B, right: 0x3E, leftDevice: 0x0001, rightDevice: 0x2000, flag: .control,
               leftBit: UInt32(CSSGSG_MOD_CTRL_L), rightBit: UInt32(CSSGSG_MOD_CTRL_R)),
        Family(left: 0x3A, right: 0x3D, leftDevice: 0x0020, rightDevice: 0x0040, flag: .option,
               leftBit: UInt32(CSSGSG_MOD_ALT_L), rightBit: UInt32(CSSGSG_MOD_ALT_R)),
        Family(left: 0x37, right: 0x36, leftDevice: 0x0008, rightDevice: 0x0010, flag: .command,
               leftBit: UInt32(CSSGSG_MOD_META_L), rightBit: UInt32(CSSGSG_MOD_META_R)),
    ]
    private static let capsLockKeyCode: UInt16 = 0x39

    /// 눌려 있다고 기억하는 수식키(keyCode).
    private(set) var held: Set<UInt16> = []
    private var capsOn: Bool?

    /// 수식키 플래그 → 엔진 비트. 좌우를 알 수 없어서 무리의 양쪽 비트를 켠다.
    static func familyMods(_ flags: NSEvent.ModifierFlags) -> UInt32 {
        families.reduce(0) { bits, f in flags.contains(f.flag) ? bits | f.leftBit | f.rightBit : bits }
    }

    /// 지금 실제로 누르고 있는 수식키(빠른 탭 전환 보정의 타이머가 엔진에 준다). 이벤트가 아니라 지금 상태라서,
    /// 앱을 거쳐 늦게 오는 뗌 이벤트보다 먼저 안다(NRIME physicalModifierFlags).
    static func physicalMods() -> UInt32 {
        familyMods(NSEvent.modifierFlags)
    }

    /// keyDown → 엔진 이벤트. 수식키 비트는 기억한 상태로 채운다.
    mutating func keyDown(_ event: NSEvent) -> CssgsgKeyEvent {
        resync(event.modifierFlags)
        return CssgsgKeyEvent(
            key: cssgsg_key_from_mac_keycode(event.keyCode),
            down: 1,
            is_repeat: event.isARepeat ? 1 : 0,
            mods: mods(event.modifierFlags),
            time: event.timestamp
        )
    }

    /// flagsChanged → 수식키 하나의 눌림/뗌. 바뀐 것이 없거나(중복) 모르는 키(fn 등)면 nil.
    /// Caps Lock은 켜짐/꺼짐이 바뀔 때 눌림으로 보낸다(상태는 mods의 CAPS 비트).
    mutating func flagsChanged(_ event: NSEvent) -> CssgsgKeyEvent? {
        let flags = event.modifierFlags
        let code = event.keyCode
        if code == Self.capsLockKeyCode {
            let on = flags.contains(.capsLock)
            defer { capsOn = on }
            guard on != capsOn else { return nil }
            return make(code, down: true, flags: flags, time: event.timestamp)
        }
        guard let family = Self.families.first(where: { $0.left == code || $0.right == code }) else { return nil }
        let raw = flags.rawValue
        let device = code == family.left ? family.leftDevice : family.rightDevice
        let sideKnown = raw & (family.leftDevice | family.rightDevice) != 0
        let isDown = sideKnown ? raw & device != 0 : flags.contains(family.flag)
        let wasDown = held.contains(code)
        if isDown {
            held.insert(code)
        } else {
            held.remove(code)
        }
        resync(flags)
        guard isDown != wasDown else { return nil }
        return make(code, down: isDown, flags: flags, time: event.timestamp)
    }

    /// 수식키 상태 → CSSGSG_MOD_* 비트. 기기 비트가 있으면 그것, 없으면 기억한 좌우, 그것도 없으면 왼쪽.
    func mods(_ flags: NSEvent.ModifierFlags) -> UInt32 {
        let raw = flags.rawValue
        var m: UInt32 = 0
        for family in Self.families where flags.contains(family.flag) {
            if raw & (family.leftDevice | family.rightDevice) != 0 {
                if raw & family.leftDevice != 0 { m |= family.leftBit }
                if raw & family.rightDevice != 0 { m |= family.rightBit }
            } else {
                let left = held.contains(family.left)
                let right = held.contains(family.right)
                if left || !right { m |= family.leftBit }
                if right { m |= family.rightBit }
            }
        }
        if flags.contains(.capsLock) {
            m |= UInt32(CSSGSG_MOD_CAPS)
        }
        return m
    }

    /// 기억을 이벤트의 플래그에 맞춘다: 합친 플래그가 꺼진 계열은 지우고, 기기 비트가 있으면 그대로 따른다.
    private mutating func resync(_ flags: NSEvent.ModifierFlags) {
        let raw = flags.rawValue
        for family in Self.families {
            if !flags.contains(family.flag) {
                held.remove(family.left)
                held.remove(family.right)
            } else if raw & (family.leftDevice | family.rightDevice) != 0 {
                for (code, device) in [(family.left, family.leftDevice), (family.right, family.rightDevice)] {
                    if raw & device != 0 { held.insert(code) } else { held.remove(code) }
                }
            }
        }
    }

    private func make(_ code: UInt16, down: Bool, flags: NSEvent.ModifierFlags, time: TimeInterval) -> CssgsgKeyEvent {
        CssgsgKeyEvent(key: cssgsg_key_from_mac_keycode(code), down: down ? 1 : 0, is_repeat: 0, mods: mods(flags), time: time)
    }
}
