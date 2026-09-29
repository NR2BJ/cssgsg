import Foundation

/// 입력기가 뜰 때 Mozc 엔진을 고르고 켠다: 받은 더 새것부터 읽어 보고, 안 되면 앱에 든 것.
/// 엔진은 입력기 프로세스 안에서 돈다(코어가 libcssgsg_mozc.dylib를 읽는다, core/src/mozc.rs).
/// 준비는 10~20ms라 뜰 때 바로 한다. 메인 스레드에서만 쓴다.
enum MozcLoader {
    /// 지금 쓰는 엔진. 없으면 일본어는 가나만 입력한다(히라가나·가타카나 두 후보).
    private(set) static var active: MozcComponent?
    /// 이번 시작의 기록(MozcComponents.beginLoad). 깨끗이 끝날 때 뺀다.
    private static var startedAt: Date?

    /// 차례대로 읽어 보고 처음 되는 것을 쓴다. 받은 엔진이 안 되면 나쁜 엔진으로 적고 다음 것으로 간다.
    @discardableResult
    static func start(_ engine: CoreEngine, candidates: [MozcComponent] = MozcComponents.candidates(),
                      profile: URL = Cssgsg.mozcProfileURL) -> MozcComponent? {
        try? FileManager.default.createDirectory(at: profile, withIntermediateDirectories: true)
        for component in candidates {
            let now = Date()
            guard MozcComponents.beginLoad(component, now: now) else { continue }
            let error = engine.useMozc(libraryPath: component.libraryURL.path, dataPath: component.dataURL.path,
                                       profileDir: profile.path)
            if let error {
                if component.source == .downloaded {
                    MozcComponents.markBad(component, reason: error)
                } else {
                    DeveloperLogger.shared.log("Mozc", "bundled engine failed", metadata: ["error": error])
                }
                continue
            }
            MozcComponents.endLoad(component)
            active = component
            startedAt = now
            DeveloperLogger.shared.log("Mozc", "ready", metadata: [
                "ms": String(format: "%.0f", Date().timeIntervalSince(now) * 1000),
                "version": component.version, "date": component.date,
                "commit": String(component.commit.prefix(7)), "source": component.source.rawValue,
            ])
            MozcComponents.prune(keeping: component)
            return component
        }
        DeveloperLogger.shared.log("Mozc", "no engine could be loaded", metadata: ["tried": "\(candidates.count)"])
        return nil
    }

    /// 입력기가 깨끗이 끝날 때. 이 시작은 죽고 다시 뜨기로 세지 않는다.
    static func finish() {
        if let active, let startedAt {
            MozcComponents.endRun(active, startedAt: startedAt)
        }
    }
}

extension MozcStatus {
    /// 입력기에서: 상태를 고쳐 적고 설정 앱에 알린다.
    static func update(_ change: (inout MozcStatus) -> Void) {
        var status = Cssgsg.imeStatus(MozcStatus.self, key: defaultsKey) ?? MozcStatus()
        change(&status)
        Cssgsg.publishIMEStatus(status, key: defaultsKey, notification: changedNotification)
    }
}
