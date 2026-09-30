// NRIME(github.com/NR2BJ/NRIME)의 PermissionMonitor를 가져왔다.
import ApplicationServices
import Cocoa

/// 입력기 자신의 권한을 확인해 설정 앱에 알린다(PermissionStatus). 없는 권한은 설정 앱에서 단추를 눌렀을 때만
/// macOS에 청한다. 스스로는 묻지 않는다(0.5.x는 처음 실행 때 한 번 물었다).
enum PermissionMonitor {
    private static var lastCheck: Date = .distantPast

    /// 입력기가 뜰 때. 그다음은 시스템 설정에서 손쉬운 사용(기기 제어 및 데이터 접근) 목록이 바뀔 때마다 다시 본다:
    /// macOS가 그때 com.apple.accessibility.api 분산 알림을 보낸다. 권한 기록에 반영되기까지 조금 걸려서 잠깐 뒤에 본다.
    /// 그래서 시스템 설정에서 켜고 끄면 설정 앱의 상태가 따라 바뀐다(설정 앱으로 돌아올 때도 다시 묻는다).
    static func start() {
        refresh(force: true)
        DistributedNotificationCenter.default().addObserver(
            forName: Notification.Name("com.apple.accessibility.api"), object: nil, queue: .main
        ) { _ in
            DeveloperLogger.shared.log("Permissions", "system list changed")
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { refresh(force: true) }
        }
    }

    /// 입력기가 활성화될 때마다 부른다. 싸지만 1분에 한 번으로 줄인다. 뜬 뒤에 준 권한도 이렇게 보인다.
    static func refreshIfStale() {
        refresh(force: false)
    }

    static func refresh(force: Bool) {
        let now = Date()
        guard force || now.timeIntervalSince(lastCheck) > 60 else { return }
        lastCheck = now
        // CGPreflightPostEventAccess를 그대로 쓰지 않는다: 그 답은 프로세스에서 처음 물었을 때로 굳는다
        // (KeyEventReposter.canPostEvents). 기록에는 무엇이 굳어 있는지 보이게 따로 남긴다.
        let status = PermissionStatus(postEvents: KeyEventReposter.canPostEvents, accessibility: AXIsProcessTrusted(),
                                      checkedAt: now)
        let previous: PermissionStatus? = Cssgsg.imeStatus(PermissionStatus.self, key: PermissionStatus.defaultsKey)
        Cssgsg.publishIMEStatus(status, key: PermissionStatus.defaultsKey, notification: PermissionStatus.changedNotification)
        if previous?.postEvents != status.postEvents || previous?.accessibility != status.accessibility {
            DeveloperLogger.shared.log("Permissions", "status", metadata: [
                "postEvents": "\(status.postEvents)", "accessibility": "\(status.accessibility)",
                "preflight": "\(CGPreflightPostEventAccess())",
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
