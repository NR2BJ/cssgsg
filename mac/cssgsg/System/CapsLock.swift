import IOKit
import IOKit.hidsystem

/// Caps Lock 켜짐 상태를 바꾼다(손쉬운 사용 권한 필요 없음).
/// 일본어 모드에서 켠 Caps Lock(가타카나)을 다른 모드로 나갈 때 끄는 데 쓴다.
enum CapsLock {
    static func set(_ on: Bool) {
        let service = IOServiceGetMatchingService(kIOMainPortDefault, IOServiceMatching(kIOHIDSystemClass))
        guard service != IO_OBJECT_NULL else { return }
        defer { IOObjectRelease(service) }
        var connect: io_connect_t = 0
        // kIOHIDParamConnectType = 1
        guard IOServiceOpen(service, mach_task_self_, 1, &connect) == KERN_SUCCESS else { return }
        defer { IOServiceClose(connect) }
        // kIOHIDCapsLockState = 1
        let result = IOHIDSetModifierLockState(connect, 1, on)
        DeveloperLogger.shared.log("CapsLock", "set", metadata: ["on": "\(on)", "result": "\(result)"])
    }
}
