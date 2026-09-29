import Foundation

/// 한자 학습(고른 후보 기억) 파일. 엔진이 기억을 들고 있고, 여기서는 읽고 쓰기만 한다.
/// ~/Library/Application Support/cssgsg/hanja-learning.tsv, 줄마다 `읽기\t글자\t고른 횟수\t순번`.
/// 확정할 때마다 바뀌므로 잠시 모았다가 저장한다. 끝낼 때(다시 시작 포함)는 바로 저장한다.
final class HanjaLearningStore {
    static let shared = HanjaLearningStore()

    private let url = CoreEngine.hanjaLearningURL
    private var savePending = false
    private let queue = DispatchQueue(label: "cssgsg.hanja-learning", qos: .utility)

    /// 파일을 엔진에 불러온다(없으면 빈 기억).
    func load(into engine: CoreEngine = .shared) {
        guard let tsv = try? String(contentsOf: url, encoding: .utf8) else { return }
        let count = engine.loadHanjaLearning(tsv)
        DeveloperLogger.shared.log("Hanja", "learning loaded", metadata: ["entries": "\(count)"])
    }

    /// 2초 안의 변경을 모아 한 번 저장한다. 메인 스레드에서 부른다.
    func scheduleSave(from engine: CoreEngine = .shared) {
        guard !savePending else { return }
        savePending = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
            guard let self else { return }
            self.savePending = false
            let tsv = engine.hanjaLearningTSV()
            self.queue.async { self.write(tsv) }
        }
    }

    /// 바로 저장한다(끝낼 때).
    func saveNow(from engine: CoreEngine = .shared) {
        let tsv = engine.hanjaLearningTSV()
        queue.sync { write(tsv) }
    }

    private func write(_ tsv: String) {
        do {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try tsv.write(to: url, atomically: true, encoding: .utf8)
        } catch {
            DeveloperLogger.shared.log("Hanja", "learning save failed", metadata: ["error": "\(error)"])
        }
    }
}
