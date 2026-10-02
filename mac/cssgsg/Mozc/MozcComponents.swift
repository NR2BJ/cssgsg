// NRIME(github.com/NR2BJ/NRIME) 1.0.12의 MozcComponents.swift를 가져왔다. 다른 점:
// - 연달아 시작하는 것을 셀 때 깨끗이 끝난 시작은 빼고 센다(endRun). 메뉴의 다시 시작이나 "지금 적용"을 몇 번
//   눌러도 멀쩡한 엔진을 버리지 않는다.
// - 이 입력기가 읽을 수 있는 C API 판은 코어가 알려 준다(cssgsg_mozc_abi).
import Foundation

/// 입력기가 읽을 수 있는 Mozc 엔진 하나: libcssgsg_mozc.dylib와 같이 빌드한 mozc.data.
/// 하나는 앱에 들어 있고, 더 새것은 MozcUpdater가 받는다(엔진 워크플로가 upstream Mozc로 빌드한 것).
/// 그래서 입력기를 새로 내지 않아도 Mozc가 올라간다.
struct MozcComponent: Equatable {
    enum Source: String {
        case bundled
        case downloaded
    }

    let source: Source
    let libraryURL: URL
    let dataURL: URL
    let commit: String
    /// Mozc 커밋 날짜(yyyy-MM-dd). "더 새것"의 기준.
    let date: String
    let version: String
    /// cssgsg 래퍼(mozc/cssgsg, 우리 C API) 판. 래퍼만 고쳐 같은 Mozc 커밋으로 다시 빌드한 엔진은 이것만 크다
    /// (0.7.2, mozc/cssgsg/WRAPPER_REVISION). 판을 적지 않던 엔진은 0.
    var wrapper: Int = 0
    /// 받은 엔진의 파일과 상태가 있는 폴더. 앱에 든 것은 nil.
    let directory: URL?

    /// 상태에 적는 판. 래퍼 판 0은 적지 않는다(0.7.1까지 적은 것과 같은 모양).
    var build: MozcStatus.Build {
        MozcStatus.Build(version: version, date: date, commit: commit, wrapper: wrapper > 0 ? wrapper : nil)
    }
}

/// 엔진 찾기, 고르기, 지키기.
///
/// 받은 엔진이 잘못되면 잘해야 일본어 변환이 없고, 나쁘면 입력기가 뜰 때마다 죽는다. 그래서 읽지 못한 엔진,
/// 읽다가 프로세스가 죽은 엔진(읽기가 끝나지 않았다), 10분 안에 깨끗이 끝나지 않은 시작이 세 번인 엔진(죽고 다시 뜨기를
/// 되풀이한다)은 나쁜 엔진으로 적고 다시 쓰지 않는다. 그러면 앱에 든 엔진이 대신한다.
enum MozcComponents {
    /// 이 입력기가 읽을 수 있는 C API 판(코어의 mozc::SUPPORTED_ABI = CSSGSG_MOZC_ABI_VERSION).
    static var supportedABI: Int { Int(cssgsg_mozc_abi()) }

    static let libraryName = "libcssgsg_mozc.dylib"
    static let dataName = "mozc.data"
    static let manifestName = "manifest.json"

    /// 받은 엔진: ~/Library/Application Support/cssgsg/mozc-engines/<커밋>/. 시험에서는 바꾼다.
    static var downloadsDirectory = Cssgsg.mozcEnginesURL

    /// 앱에 든 엔진. 시험에서는 바꾼다.
    static var bundled: MozcComponent? = bundled(in: Bundle.main)

    // MARK: - 찾기

    /// 앱 안의 Frameworks/libcssgsg_mozc.dylib, Resources/mozc.data, Resources/MOZC_VERSION("<커밋> <날짜> <버전> <래퍼 판>",
    /// 래퍼 판은 0.7.2부터).
    static func bundled(in bundle: Bundle) -> MozcComponent? {
        guard let library = bundle.privateFrameworksURL?.appendingPathComponent(libraryName),
              FileManager.default.fileExists(atPath: library.path),
              let data = bundle.url(forResource: "mozc", withExtension: "data"),
              let versionFile = bundle.url(forResource: "MOZC_VERSION", withExtension: nil),
              let line = try? String(contentsOf: versionFile, encoding: .utf8) else { return nil }
        let parts = line.split(whereSeparator: \.isWhitespace).map(String.init)
        guard parts.count >= 3 else { return nil }
        return MozcComponent(source: .bundled, libraryURL: library, dataURL: data,
                             commit: parts[0], date: parts[1], version: parts[2],
                             wrapper: parts.count >= 4 ? Int(parts[3]) ?? 0 : 0, directory: nil)
    }

    /// 받은 엔진 폴더의 manifest.json(엔진 워크플로의 tools/mozc/package-component.sh가 쓴다).
    struct Manifest: Codable, Equatable {
        let abi: Int
        let commit: String
        let date: String
        let version: String
        /// 래퍼 판(0.7.2부터 적는다). 없으면 0.
        var wrapper: Int? = nil
        /// 파일 이름 → SHA-256(16진).
        let files: [String: String]
    }

    /// 받은 엔진 폴더 이름: 래퍼 판이 0이면 커밋(0.7.1까지와 같다), 아니면 "<커밋>-w<래퍼 판>".
    /// 같은 Mozc 커밋이라도 래퍼 판이 다르면 다른 엔진이다(나쁜 엔진 표시도 따로).
    static func directoryName(commit: String, wrapper: Int) -> String {
        wrapper > 0 ? "\(commit)-w\(wrapper)" : commit
    }

    /// 폴더 이름 → (커밋, 래퍼 판).
    static func identity(ofDirectory name: String) -> (commit: String, wrapper: Int) {
        if let range = name.range(of: "-w", options: .backwards), let wrapper = Int(name[range.upperBound...]) {
            return (String(name[..<range.lowerBound]), wrapper)
        }
        return (name, 0)
    }

    /// 읽어도 되는 받은 엔진: 파일이 다 있고, 판이 맞고, 나쁜 엔진이 아닌 것.
    static func downloaded() -> [MozcComponent] {
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(at: downloadsDirectory,
                                                        includingPropertiesForKeys: nil) else { return [] }
        return entries.compactMap { directory -> MozcComponent? in
            guard !directory.lastPathComponent.hasPrefix("."),
                  !fm.fileExists(atPath: directory.appendingPathComponent(State.bad.rawValue).path),
                  let data = try? Data(contentsOf: directory.appendingPathComponent(manifestName)),
                  let manifest = try? JSONDecoder().decode(Manifest.self, from: data),
                  manifest.abi == supportedABI else { return nil }
            let library = directory.appendingPathComponent(libraryName)
            let mozcData = directory.appendingPathComponent(dataName)
            guard fm.fileExists(atPath: library.path), fm.fileExists(atPath: mozcData.path) else { return nil }
            return MozcComponent(source: .downloaded, libraryURL: library, dataURL: mozcData,
                                 commit: manifest.commit, date: manifest.date, version: manifest.version,
                                 wrapper: manifest.wrapper ?? 0, directory: directory)
        }
    }

    /// 읽어 볼 차례: 앱에 든 것보다 새로 받은 것(새것부터), 그다음 앱에 든 것.
    static func candidates() -> [MozcComponent] {
        let base = bundled
        let newer = downloaded()
            .filter { component in
                guard let base else { return true }
                return isNewer(component, than: base)
            }
            .sorted { isNewer($0, than: $1) }
        return newer + (base.map { [$0] } ?? [])
    }

    /// Mozc 커밋 날짜로, 같으면 Mozc 버전으로 비교한다. 같은 커밋이면 래퍼 판이 큰 것이 새것이다(래퍼만 고친 엔진, 0.7.2).
    static func isNewer(_ lhs: MozcComponent, than rhs: MozcComponent) -> Bool {
        guard lhs.commit != rhs.commit else { return lhs.wrapper > rhs.wrapper }
        if lhs.date != rhs.date { return lhs.date > rhs.date }
        guard let left = SemanticVersion(lhs.version), let right = SemanticVersion(rhs.version) else { return false }
        return right < left
    }

    /// 나쁜 엔진의 폴더 이름(directoryName: 커밋, 래퍼 판이 있으면 "<커밋>-w<판>"). 다시 받지 않는다.
    static func badEngines() -> Set<String> {
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(at: downloadsDirectory,
                                                        includingPropertiesForKeys: nil) else { return [] }
        return Set(entries.compactMap { directory -> String? in
            guard fm.fileExists(atPath: directory.appendingPathComponent(State.bad.rawValue).path) else { return nil }
            return directory.lastPathComponent
        })
    }

    // MARK: - 지키기

    enum State: String {
        /// 읽기 전에 쓰고 다 읽으면 지운다. 다음 시작 때 남아 있으면 읽다가 프로세스가 죽은 것이다.
        case loading
        /// 이 엔진을 다시 읽지 않는다. 내용은 까닭.
        case bad
        /// 아직 깨끗이 끝나지 않은 시작 시각(한 줄에 하나). 죽고 다시 뜨기를 되풀이하는지 본다.
        case starts
    }

    /// 받은 엔진을 읽기 전에 부른다. 읽으면 안 되면(그리고 나쁜 엔진으로 적었으면) false: 지난번 읽기가 끝나지 않았거나,
    /// 10분 안에 깨끗이 끝나지 않은 시작이 이번까지 세 번이다. `now`는 이번 시작의 기록이다(endRun에 같은 값을 준다).
    static func beginLoad(_ component: MozcComponent, now: Date = Date()) -> Bool {
        guard let directory = component.directory else { return true }
        let loading = directory.appendingPathComponent(State.loading.rawValue)
        if FileManager.default.fileExists(atPath: loading.path) {
            markBad(component, reason: "지난번 읽기가 끝나지 않았다(읽다가 프로세스가 죽었다)")
            return false
        }
        let earlier = starts(in: directory)
        let recent = (earlier + [now]).filter { now.timeIntervalSince($0) < 600 }
        if recent.count >= 3 {
            markBad(component, reason: "10분 안에 깨끗이 끝나지 않은 시작이 \(recent.count)번")
            return false
        }
        writeStarts((earlier + [now]).suffix(5), in: directory)
        try? Data().write(to: loading)
        return true
    }

    /// 다 읽었다(엔진을 만들고 변환해 보았다).
    static func endLoad(_ component: MozcComponent) {
        guard let directory = component.directory else { return }
        try? FileManager.default.removeItem(at: directory.appendingPathComponent(State.loading.rawValue))
    }

    /// 입력기가 깨끗이 끝날 때(applicationWillTerminate): 이번 시작을 기록에서 뺀다. 죽어서 끝난 시작만 남는다.
    static func endRun(_ component: MozcComponent, startedAt: Date) {
        guard let directory = component.directory else { return }
        var remaining = starts(in: directory)
        // 적었다 읽은 시각은 마지막 자리가 다를 수 있다(Date는 2001년 기준으로 든다).
        if let index = remaining.lastIndex(where: { abs($0.timeIntervalSince(startedAt)) < 0.001 }) {
            remaining.remove(at: index)
            writeStarts(remaining, in: directory)
        }
    }

    static func markBad(_ component: MozcComponent, reason: String) {
        guard let directory = component.directory else { return }
        try? reason.write(to: directory.appendingPathComponent(State.bad.rawValue), atomically: true, encoding: .utf8)
        try? FileManager.default.removeItem(at: directory.appendingPathComponent(State.loading.rawValue))
        DeveloperLogger.shared.log("Mozc", "component marked bad", metadata: [
            "version": component.version, "commit": String(component.commit.prefix(7)), "reason": reason,
        ])
    }

    /// 새로 받은 좋은 엔진 둘(쓰는 것과 물러설 것)만 남기고 지운다. 나쁜 엔진은 표시만 남긴다(같은 것을 또 받지 않게).
    static func prune(keeping active: MozcComponent?) {
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(at: downloadsDirectory,
                                                        includingPropertiesForKeys: nil) else { return }
        let good = downloaded().sorted { isNewer($0, than: $1) }
        var keep = Set(good.prefix(2).compactMap { $0.directory?.standardizedFileURL })
        if let directory = active?.directory { keep.insert(directory.standardizedFileURL) }
        for entry in entries where !entry.lastPathComponent.hasPrefix(".") {
            if fm.fileExists(atPath: entry.appendingPathComponent(State.bad.rawValue).path) {
                try? fm.removeItem(at: entry.appendingPathComponent(libraryName))
                try? fm.removeItem(at: entry.appendingPathComponent(dataName))
            } else if !keep.contains(entry.standardizedFileURL) {
                try? fm.removeItem(at: entry)
            }
        }
    }

    private static func starts(in directory: URL) -> [Date] {
        let text = (try? String(contentsOf: directory.appendingPathComponent(State.starts.rawValue), encoding: .utf8)) ?? ""
        return text.split(separator: "\n")
            .compactMap { TimeInterval(String($0)) }
            .map(Date.init(timeIntervalSince1970:))
    }

    private static func writeStarts<S: Sequence>(_ dates: S, in directory: URL) where S.Element == Date {
        let text = dates.map { String($0.timeIntervalSince1970) }.joined(separator: "\n")
        try? text.write(to: directory.appendingPathComponent(State.starts.rawValue), atomically: true, encoding: .utf8)
    }
}

extension MozcStatus {
    /// 입력기가 엔진을 읽은 뒤의 상태: 쓰는 엔진을 적고, 그만큼 새것을 기다리던 표시는 지운다.
    func started(with component: MozcComponent?) -> MozcStatus {
        var status = self
        status.active = component?.build
        status.activeSource = component?.source.rawValue
        // 기다리던 것만큼 새 엔진으로 떴으면 지운다: 같은 커밋이면 래퍼 판이 그만큼 되었는지, 아니면 날짜로.
        if let pending, let component,
           pending.commit == component.commit ? (pending.wrapper ?? 0) <= component.wrapper : pending.date <= component.date {
            status.pending = nil
        }
        return status
    }
}
