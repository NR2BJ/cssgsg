import Foundation

/// 개발자 모드 기록. 켜져 있을 때만 파일에 쓴다.
///
/// 글자 내용은 쓰지 않는다. 키 코드, 수식키, 길이, 모드만 남긴다. 문제가 생기면 이 기록을
/// 코어에 그대로 재생해서 재현한다.
///
/// 켜기: `defaults write com.cssgsg.inputmethod.app developerMode -bool true`
/// 파일: ~/Library/Application Support/cssgsg/developer.log
final class DeveloperLogger {
    static let shared = DeveloperLogger()

    private let queue = DispatchQueue(label: "com.cssgsg.devlog")
    private let fileURL: URL
    private let formatter: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()
    private static let maxBytes = 5 * 1024 * 1024

    var isEnabled: Bool { UserDefaults.standard.bool(forKey: "developerMode") }

    private init() {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let dir = support.appendingPathComponent("cssgsg", isDirectory: true)
        fileURL = dir.appendingPathComponent("developer.log")
    }

    func log(_ category: String, _ message: String, metadata: [String: String] = [:]) {
        guard isEnabled else { return }
        let meta = metadata.sorted { $0.key < $1.key }.map { "\($0.key)=\($0.value)" }.joined(separator: " ")
        let line = "\(formatter.string(from: Date())) [\(category)] \(message)\(meta.isEmpty ? "" : " " + meta)\n"
        let url = fileURL
        queue.async {
            let fm = FileManager.default
            try? fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            if let attrs = try? fm.attributesOfItem(atPath: url.path),
               let size = attrs[.size] as? Int, size > Self.maxBytes {
                let old = url.deletingPathExtension().appendingPathExtension("old.log")
                try? fm.removeItem(at: old)
                try? fm.moveItem(at: url, to: old)
            }
            guard let data = line.data(using: .utf8) else { return }
            if let handle = try? FileHandle(forWritingTo: url) {
                handle.seekToEndOfFile()
                handle.write(data)
                try? handle.close()
            } else {
                try? data.write(to: url)
            }
        }
    }
}
