// NRIME(github.com/NR2BJ/NRIME)의 PermissionMonitor를 가져왔다.
import ApplicationServices
import Cocoa

/// 입력기 자신의 권한을 확인해 설정 앱에 알린다(PermissionStatus). 없는 권한은 설정 앱에서 단추를 눌렀을 때만
/// macOS에 청한다. 스스로는 묻지 않는다(0.5.x는 처음 실행 때 한 번 물었다).
enum PermissionMonitor {
    private static var lastCheck: Date = .distantPast

    /// 입력기가 뜰 때.
    static func start() {
        refresh(force: true)
    }

    /// 입력기가 활성화될 때마다 부른다. 싸지만 1분에 한 번으로 줄인다. 뜬 뒤에 준 권한도 이렇게 보인다.
    static func refreshIfStale() {
        refresh(force: false)
    }

    static func refresh(force: Bool) {
        let now = Date()
        guard force || now.timeIntervalSince(lastCheck) > 60 else { return }
        lastCheck = now
        // CGPreflightPostEventAccess 하나만 보지 않는다: 그 답은 프로세스에서 처음 물었을 때로 굳는다
        // (KeyEventReposter.canPostEvents).
        let status = PermissionStatus(postEvents: KeyEventReposter.canPostEvents, accessibility: AXIsProcessTrusted(),
                                      checkedAt: now)
        let previous: PermissionStatus? = Cssgsg.imeStatus(PermissionStatus.self, key: PermissionStatus.defaultsKey)
        Cssgsg.publishIMEStatus(status, key: PermissionStatus.defaultsKey, notification: PermissionStatus.changedNotification)
        if previous?.postEvents != status.postEvents || previous?.accessibility != status.accessibility {
            DeveloperLogger.shared.log("Permissions", "status", metadata: [
                "postEvents": "\(status.postEvents)", "accessibility": "\(status.accessibility)",
            ])
        }
    }

    /// 설정 앱의 "다시 확인 / 권한 요청": 없는 권한을 청하고 다시 확인한다.
    /// 손쉬운 사용 창을 띄우면 macOS 27은 cssgsg를 "기기 제어 및 데이터 접근"에 올린다. 이 권한이 키 보내기도 허락한다.
    /// CGRequestPostEventAccess만으로는 모자란다: 확인과 마찬가지로 프로세스의 첫 답을 되풀이해 macOS까지 안 갈 수 있다.
    static func requestMissing() {
        if !AXIsProcessTrusted() {
            DeveloperLogger.shared.log("Permissions", "requesting access")
            let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
            _ = AXIsProcessTrustedWithOptions(options)
            if !CGPreflightPostEventAccess() {
                _ = CGRequestPostEventAccess()
            }
        }
        refresh(force: true)
    }
}
