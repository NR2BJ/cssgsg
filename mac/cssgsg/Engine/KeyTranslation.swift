import Cocoa

/// NSEvent → 엔진 키 이벤트.
///
/// 수식키 좌우는 이벤트의 기기별 비트(IOLLEvent.h NX_DEVICE*KEYMASK)로 가린다. 합친 플래그로는
/// 양쪽 Shift를 구분할 수 없어서, 왼쪽 Shift+1이 오른쪽 Shift+1로 잘못 잡힌 적이 있다(NRIME 39b98bd).
enum KeyTranslation {

    private static let modShiftL = UInt32(CSSGSG_MOD_SHIFT_L)
    private static let modShiftR = UInt32(CSSGSG_MOD_SHIFT_R)
    private static let modCtrlL = UInt32(CSSGSG_MOD_CTRL_L)
    private static let modCtrlR = UInt32(CSSGSG_MOD_CTRL_R)
    private static let modAltL = UInt32(CSSGSG_MOD_ALT_L)
    private static let modAltR = UInt32(CSSGSG_MOD_ALT_R)
    private static let modMetaL = UInt32(CSSGSG_MOD_META_L)
    private static let modMetaR = UInt32(CSSGSG_MOD_META_R)
    private static let modCaps = UInt32(CSSGSG_MOD_CAPS)

    /// 수식키 keyCode → (그 키의 기기 비트, 같은 종류 양쪽 기기 비트, 합친 플래그)
    private static let modifierKeys: [UInt16: (device: UInt, pair: UInt, flag: NSEvent.ModifierFlags)] = [
        0x38: (0x0002, 0x0006, .shift),       // 왼쪽 Shift
        0x3C: (0x0004, 0x0006, .shift),       // 오른쪽 Shift
        0x3B: (0x0001, 0x2001, .control),     // 왼쪽 Control
        0x3E: (0x2000, 0x2001, .control),     // 오른쪽 Control
        0x3A: (0x0020, 0x0060, .option),      // 왼쪽 Option
        0x3D: (0x0040, 0x0060, .option),      // 오른쪽 Option
        0x37: (0x0008, 0x0018, .command),     // 왼쪽 Command
        0x36: (0x0010, 0x0018, .command),     // 오른쪽 Command
    ]

    /// 수식키 상태 → CSSGSG_MOD_* 비트. 기기 비트가 없는 합성 이벤트는 왼쪽으로 친다.
    static func mods(_ flags: NSEvent.ModifierFlags) -> UInt32 {
        let raw = flags.rawValue
        var m: UInt32 = 0
        func side(_ left: UInt, _ right: UInt, _ l: UInt32, _ r: UInt32, _ flag: NSEvent.ModifierFlags) {
            if raw & left != 0 { m |= l }
            if raw & right != 0 { m |= r }
            if flags.contains(flag) && raw & (left | right) == 0 { m |= l }
        }
        side(0x0002, 0x0004, modShiftL, modShiftR, .shift)
        side(0x0001, 0x2000, modCtrlL, modCtrlR, .control)
        side(0x0020, 0x0040, modAltL, modAltR, .option)
        side(0x0008, 0x0010, modMetaL, modMetaR, .command)
        if flags.contains(.capsLock) { m |= modCaps }
        return m
    }

    static func keyDown(_ event: NSEvent) -> CssgsgKeyEvent {
        CssgsgKeyEvent(
            key: cssgsg_key_from_mac_keycode(event.keyCode),
            down: 1,
            is_repeat: event.isARepeat ? 1 : 0,
            mods: mods(event.modifierFlags),
            time: event.timestamp
        )
    }

    /// flagsChanged → 수식키 하나의 눌림/뗌. Caps Lock은 눌림으로 보낸다(상태는 mods의 CAPS 비트).
    /// 알 수 없는 키(Fn 등)는 nil.
    static func flagsChanged(_ event: NSEvent, previous: NSEvent.ModifierFlags) -> CssgsgKeyEvent? {
        let key = cssgsg_key_from_mac_keycode(event.keyCode)
        if event.keyCode == 0x39 {
            return CssgsgKeyEvent(key: key, down: 1, is_repeat: 0, mods: mods(event.modifierFlags), time: event.timestamp)
        }
        guard let info = modifierKeys[event.keyCode] else { return nil }
        let now = event.modifierFlags.rawValue
        let sideKnown = ((now | previous.rawValue) & info.pair) != 0
        let isDown = sideKnown ? (now & info.device) != 0 : event.modifierFlags.contains(info.flag)
        return CssgsgKeyEvent(
            key: key,
            down: isDown ? 1 : 0,
            is_repeat: 0,
            mods: mods(event.modifierFlags),
            time: event.timestamp
        )
    }
}
