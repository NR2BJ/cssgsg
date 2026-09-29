// NRIME(github.com/NR2BJ/NRIME) 1.0.12의 MozcUpdater.swift를 가져왔다.
import Foundation

/// GitHub에서 더 새 Mozc 엔진을 찾아 받는다. 그래서 입력기를 새로 내지 않아도 이 맥의 Mozc가 올라간다.
///
/// 엔진 워크플로(.github/workflows/mozc-component.yml)가 upstream Mozc의 버전이나 사전 데이터가 바뀔 때마다
/// GitHub 컴퓨터에서 빌드하고 코어의 Mozc 시험을 돌린 뒤, mozc-<판>-<yyyymmdd>-<커밋 7자리> 태그의 prerelease로
/// cssgsg-mozc.zip(libcssgsg_mozc.dylib, mozc.data, manifest.json)을 낸다. 앱 업데이트는 이 릴리스를 보지 않는다
/// (pkg가 없다, 태그가 버전이 아니다).
///
/// 입력기가 뜨고 5분 뒤, 그다음은 하루에 한 번 확인한다(설정 앱의 "지금 확인"은 바로). 보내는 것은 릴리스 목록 요청뿐이다.
/// 받은 파일은 GitHub가 적은 SHA-256과 manifest의 파일별 해시가 모두 맞아야 쓰고, 입력기가 다음에 뜰 때부터 쓴다
/// (설정 앱의 "지금 적용"은 바로 다시 띄운다). 확인과 받기는 메인 스레드 밖에서 해서 입력을 막지 않는다.
final class MozcUpdater {
    static let shared = MozcUpdater()

    static let releasesURL = URL(string: "https://api.github.com/repos/NR2BJ/cssgsg/releases?per_page=50")!
    static let assetName = "cssgsg-mozc.zip"
    static let tagPrefix = "mozc-"
    private static let firstCheckDelay: TimeInterval = 5 * 60
    private static let checkInterval: TimeInterval = 24 * 60 * 60

    private var timer: Timer?
    private var checking = false

    private init() {}

    /// 릴리스가 내놓은 엔진.
    struct Offer: Equatable {
        let abi: Int
        /// yyyy-MM-dd
        let date: String
        let commitPrefix: String
        let downloadURL: URL
        /// GitHub가 적은 "sha256:<hex>".
        let digest: String
    }

    // MARK: - 일정 (메인 스레드)

    func start() {
        timer = Timer.scheduledTimer(withTimeInterval: Self.firstCheckDelay, repeats: false) { _ in
            MozcUpdater.shared.checkNow()
            MozcUpdater.shared.timer = Timer.scheduledTimer(withTimeInterval: Self.checkInterval, repeats: true) { _ in
                MozcUpdater.shared.checkNow()
            }
        }
    }

    /// `active`: 지금 쓰는 엔진. 그것과 받아 둔 엔진보다 새것만 받는다.
    func checkNow(active: MozcComponent? = MozcLoader.active) {
        guard !checking else { return }
        checking = true
        let installed = (active.map { [$0] } ?? []) + MozcComponents.downloaded()
        let bad = MozcComponents.badCommits()
        let abi = MozcComponents.supportedABI
        let downloads = MozcComponents.downloadsDirectory
        Task.detached(priority: .utility) {
            let outcome = await Self.check(installed: installed, bad: bad, abi: abi, downloads: downloads)
            await MainActor.run {
                MozcUpdater.shared.checking = false
                MozcStatus.update { status in
                    // 실패한 확인(오프라인, GitHub 장애)은 확인으로 치지 않는다. 설정 앱은 실패만 따로 보인다.
                    if case .failed = outcome {
                        status.checkFailedAt = Date()
                        return
                    }
                    status.checkedAt = Date()
                    status.checkFailedAt = nil
                    if case .downloaded(let component) = outcome {
                        status.pending = component.build
                    }
                }
            }
        }
    }

    // MARK: - 확인 (메인 스레드 밖)

    enum Outcome {
        case upToDate
        case downloaded(MozcComponent)
        case failed(String)
    }

    private static let session: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 30
        configuration.timeoutIntervalForResource = 10 * 60
        return URLSession(configuration: configuration)
    }()

    private static func check(installed: [MozcComponent], bad: Set<String>, abi: Int, downloads: URL) async -> Outcome {
        do {
            var request = URLRequest(url: releasesURL)
            request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
            let (data, response) = try await session.data(for: request)
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            guard status == 200 else { return log(.failed("HTTP \(status)")) }
            let releases = try JSONDecoder().decode([GitHubRelease].self, from: data)
            guard let offer = newest(in: releases, abi: abi, bad: bad), isWanted(offer, installed: installed) else {
                return log(.upToDate)
            }
            let (file, _) = try await session.download(from: offer.downloadURL)
            defer { try? FileManager.default.removeItem(at: file) }
            return log(.downloaded(try install(zip: file, offer: offer, abi: abi, into: downloads)))
        } catch {
            return log(.failed("\(error)"))
        }
    }

    @discardableResult
    private static func log(_ outcome: Outcome) -> Outcome {
        switch outcome {
        case .upToDate:
            DeveloperLogger.shared.log("Mozc", "update check: up to date")
        case .downloaded(let component):
            DeveloperLogger.shared.log("Mozc", "update downloaded", metadata: [
                "version": component.version, "date": component.date, "commit": String(component.commit.prefix(7)),
            ])
        case .failed(let reason):
            DeveloperLogger.shared.log("Mozc", "update check failed", metadata: ["reason": reason])
        }
        return outcome
    }

    /// 릴리스가 내놓은 엔진: 태그 mozc-<판>-<yyyymmdd>-<커밋 7자리>, cssgsg-mozc.zip, GitHub가 적은 해시.
    static func offer(from release: GitHubRelease) -> Offer? {
        guard release.draft != true, release.tagName.hasPrefix(tagPrefix) else { return nil }
        let parts = release.tagName.dropFirst(tagPrefix.count).split(separator: "-").map(String.init)
        guard parts.count == 3,
              let abi = Int(parts[0]),
              parts[1].count == 8, parts[1].allSatisfy(\.isASCII), Int(parts[1]) != nil,
              parts[2].count >= 7, parts[2].allSatisfy(\.isHexDigit),
              let asset = release.assets.first(where: { $0.name == assetName }),
              let url = URL(string: asset.browserDownloadURL),
              let digest = asset.digest, digest.lowercased().hasPrefix("sha256:") else { return nil }
        let d = parts[1]
        let date = "\(d.prefix(4))-\(d.dropFirst(4).prefix(2))-\(d.suffix(2))"
        return Offer(abi: abi, date: date, commitPrefix: parts[2].lowercased(), downloadURL: url, digest: digest)
    }

    /// 이 판의 엔진 중 이 맥에서 나쁜 엔진이 되지 않은 가장 새것.
    static func newest(in releases: [GitHubRelease], abi: Int, bad: Set<String>) -> Offer? {
        releases
            .compactMap(offer(from:))
            .filter { offer in
                offer.abi == abi && !bad.contains(where: { $0.lowercased().hasPrefix(offer.commitPrefix) })
            }
            .max { $0.date < $1.date }
    }

    /// 받을 만한가: 여기 있는 어느 것보다 새것이고, 이미 있는 것이 아니다.
    static func isWanted(_ offer: Offer, installed: [MozcComponent]) -> Bool {
        !installed.contains { component in
            component.commit.lowercased().hasPrefix(offer.commitPrefix) || component.date >= offer.date
        }
    }

    enum InstallError: Error {
        case digestMismatch
        case unzipFailed
        case badManifest(String)
    }

    /// 받은 cssgsg-mozc.zip을 확인하고 <downloads>/<커밋>/에 푼다. MozcComponents가 거기서 찾는다.
    static func install(zip: URL, offer: Offer, abi: Int, into downloads: URL) throws -> MozcComponent {
        guard GitHub.fileMatchesDigest(at: zip, expected: offer.digest) == true else {
            throw InstallError.digestMismatch
        }
        let fm = FileManager.default
        try fm.createDirectory(at: downloads, withIntermediateDirectories: true)
        let staging = downloads.appendingPathComponent(".staging-\(UUID().uuidString)")
        defer { try? fm.removeItem(at: staging) }
        try fm.createDirectory(at: staging, withIntermediateDirectories: true)

        let ditto = Process()
        ditto.executableURL = URL(fileURLWithPath: "/usr/bin/ditto")
        ditto.arguments = ["-x", "-k", zip.path, staging.path]
        try ditto.run()
        ditto.waitUntilExit()
        guard ditto.terminationStatus == 0 else { throw InstallError.unzipFailed }

        let manifestData = try Data(contentsOf: staging.appendingPathComponent(MozcComponents.manifestName))
        let manifest = try JSONDecoder().decode(MozcComponents.Manifest.self, from: manifestData)
        guard manifest.abi == offer.abi, manifest.abi == abi else {
            throw InstallError.badManifest("C API 판 \(manifest.abi)")
        }
        guard manifest.commit.lowercased().hasPrefix(offer.commitPrefix), manifest.date == offer.date,
              !manifest.commit.contains("/"), !manifest.commit.hasPrefix(".") else {
            throw InstallError.badManifest("커밋이나 날짜가 릴리스와 다르다")
        }
        for name in [MozcComponents.libraryName, MozcComponents.dataName] {
            guard let hex = manifest.files[name],
                  GitHub.fileMatchesDigest(at: staging.appendingPathComponent(name), expected: "sha256:\(hex)") == true
            else {
                throw InstallError.badManifest("\(name)의 해시가 맞지 않는다")
            }
        }
        // 받은 파일에 격리 표시가 붙어 있으면 라이브러리를 읽지 못할 수 있다.
        for name in [MozcComponents.libraryName, MozcComponents.dataName, MozcComponents.manifestName] {
            removexattr(staging.appendingPathComponent(name).path, "com.apple.quarantine", 0)
        }

        let destination = downloads.appendingPathComponent(manifest.commit)
        if fm.fileExists(atPath: destination.path) {
            try fm.removeItem(at: destination)
        }
        try fm.moveItem(at: staging, to: destination)
        return MozcComponent(source: .downloaded,
                             libraryURL: destination.appendingPathComponent(MozcComponents.libraryName),
                             dataURL: destination.appendingPathComponent(MozcComponents.dataName),
                             commit: manifest.commit, date: manifest.date, version: manifest.version,
                             directory: destination)
    }
}
