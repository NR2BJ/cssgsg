// NRIME(github.com/NR2BJ/NRIME)의 Shared/UpdateManager.swift를 줄여서 가져왔다.
// - 채널은 stable 하나다(GitHub /releases/latest). 0.x 동안은 모든 릴리스를 정식 릴리스로 낸다.
// - 같은 버전 재업로드 감지는 뺐다. 릴리스 스크립트가 이미 있는 태그를 거부하므로 버전은 늘 올라간다.
// - 화면은 메뉴 막대 메뉴의 항목뿐이다. 입력기 프로세스에서 모달 창을 띄우면 메인 스레드가 막혀
//   모든 앱의 입력이 멈출 수 있다.
import AppKit
import CryptoKit
import Foundation

struct GitHubRelease: Codable {
    let tagName: String
    let body: String?
    let htmlURL: String?
    let draft: Bool?
    let prerelease: Bool?
    let assets: [GitHubAsset]

    enum CodingKeys: String, CodingKey {
        case tagName = "tag_name"
        case body
        case htmlURL = "html_url"
        case draft
        case prerelease
        case assets
    }

    /// 태그 앞의 v를 뗀 버전(예: "0.1.1").
    var version: String { tagName.hasPrefix("v") ? String(tagName.dropFirst()) : tagName }

    var pkgAsset: GitHubAsset? { assets.first { $0.name.hasSuffix(".pkg") } }
}

struct GitHubAsset: Codable {
    let name: String
    let size: Int
    let browserDownloadURL: String
    /// GitHub가 계산한 내용 해시("sha256:<hex>"). pkg는 관리자 권한으로 설치하므로 받은 파일을 이것과 대조한다.
    let digest: String?

    enum CodingKeys: String, CodingKey {
        case name
        case size
        case browserDownloadURL = "browser_download_url"
        case digest
    }
}

/// GitHub 릴리스로 새 버전을 확인하고, 받아서 설치한다. 메인 스레드에서만 쓴다.
final class Updater: NSObject, URLSessionDownloadDelegate {
    static let shared = Updater()

    enum State {
        case idle
        case checking
        case upToDate
        case available(GitHubRelease)
        case downloading(GitHubRelease, progress: Double)
        case installing(GitHubRelease)
        case failed(String)
    }

    static let latestReleaseURL = URL(string: "https://api.github.com/repos/NR2BJ/cssgsg/releases/latest")!
    private static let checkInterval: TimeInterval = 24 * 60 * 60
    private static let lastCheckKey = "updateLastCheck"
    private static let etagKey = "updateETag"
    private static let cachedReleaseKey = "updateCachedRelease"

    static var currentVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.0.0"
    }

    static var cacheDirectory: URL {
        FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("com.cssgsg.inputmethod.app/updates", isDirectory: true)
    }

    /// 상태가 바뀔 때 부른다(메뉴 갱신).
    var onChange: (() -> Void)?
    private(set) var state: State = .idle {
        didSet { onChange?() }
    }

    private let defaults = UserDefaults.standard
    private var timer: Timer?
    private var downloadTask: URLSessionDownloadTask?
    private var downloading: GitHubRelease?
    private lazy var downloadSession = URLSession(configuration: .default, delegate: self, delegateQueue: .main)

    /// 실행하고 1분 뒤 한 번, 그 뒤로 한 시간마다 하루가 지났는지 보고 확인한다.
    func start() {
        DispatchQueue.main.asyncAfter(deadline: .now() + 60) { [weak self] in self?.checkIfDue() }
        timer = Timer.scheduledTimer(withTimeInterval: 60 * 60, repeats: true) { [weak self] _ in
            self?.checkIfDue()
        }
    }

    private func checkIfDue() {
        let last = defaults.double(forKey: Self.lastCheckKey)
        guard Date().timeIntervalSince1970 - last >= Self.checkInterval else { return }
        check(userInitiated: false)
    }

    // MARK: - 확인

    /// 지금 확인한다. 사람이 누른 확인이면 실패도 메뉴에 보인다. 자동 확인의 네트워크 실패는 조용히 넘긴다.
    func check(userInitiated: Bool) {
        switch state {
        case .checking, .downloading, .installing: return
        default: break
        }
        state = .checking
        var request = URLRequest(url: Self.latestReleaseURL, timeoutInterval: 20)
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        request.setValue("cssgsg/\(Self.currentVersion)", forHTTPHeaderField: "User-Agent")
        // 조건부 요청: 바뀐 게 없으면 304라서 GitHub의 비인증 요청 한도를 아낀다.
        if let etag = defaults.string(forKey: Self.etagKey), cachedRelease() != nil {
            request.setValue(etag, forHTTPHeaderField: "If-None-Match")
        }
        URLSession.shared.dataTask(with: request) { [weak self] data, response, error in
            DispatchQueue.main.async {
                self?.finishCheck(data, response as? HTTPURLResponse, error, userInitiated: userInitiated)
            }
        }.resume()
    }

    private func finishCheck(_ data: Data?, _ response: HTTPURLResponse?, _ error: Error?, userInitiated: Bool) {
        guard let response, error == nil else {
            state = userInitiated ? .failed("서버 연결 안 됨") : .idle
            return
        }
        defaults.set(Date().timeIntervalSince1970, forKey: Self.lastCheckKey)
        switch response.statusCode {
        case 304:
            evaluate(cachedRelease())
        case 200:
            guard let data, let release = try? JSONDecoder().decode(GitHubRelease.self, from: data) else {
                state = .failed("릴리스 정보 읽기 실패")
                return
            }
            // 본문을 읽은 뒤에만 ETag를 남긴다. 그래야 304가 늘 읽어 둔 릴리스를 가리킨다.
            defaults.set(data, forKey: Self.cachedReleaseKey)
            if let etag = response.value(forHTTPHeaderField: "ETag") {
                defaults.set(etag, forKey: Self.etagKey)
            }
            evaluate(release)
        case 404:
            state = .upToDate  // 아직 릴리스가 없다
        default:
            state = userInitiated ? .failed("GitHub 응답 \(response.statusCode)") : .idle
        }
    }

    private func evaluate(_ release: GitHubRelease?) {
        if let release, Self.offers(release, over: Self.currentVersion) {
            state = .available(release)
        } else {
            state = .upToDate
        }
    }

    /// 이 릴리스를 설치하라고 권할지: 정식 릴리스이고, pkg가 붙어 있고, 지금보다 높은 버전.
    static func offers(_ release: GitHubRelease, over current: String) -> Bool {
        release.draft != true && release.prerelease != true && release.pkgAsset != nil
            && SemanticVersion.isNewer(remote: release.version, than: current)
    }

    private func cachedRelease() -> GitHubRelease? {
        guard let data = defaults.data(forKey: Self.cachedReleaseKey) else { return nil }
        return try? JSONDecoder().decode(GitHubRelease.self, from: data)
    }

    // MARK: - 설치

    /// 받아서 해시를 확인하고 설치한다. 설치가 끝나면 postinstall이 이 프로세스를 끝내고 새 버전을 띄운다.
    func install() {
        guard case let .available(release) = state, let asset = release.pkgAsset,
              let url = URL(string: asset.browserDownloadURL) else { return }
        downloading = release
        state = .downloading(release, progress: 0)
        let task = downloadSession.downloadTask(with: url)
        downloadTask = task
        task.resume()
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64,
                    totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        guard downloadTask === self.downloadTask, let release = downloading, totalBytesExpectedToWrite > 0 else { return }
        state = .downloading(release, progress: Double(totalBytesWritten) / Double(totalBytesExpectedToWrite))
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        guard downloadTask === self.downloadTask, let release = downloading else { return }
        // 파일 이름은 서버가 준 이름이 아니라 버전으로 짓는다.
        let directory = Self.cacheDirectory
        let destination = directory.appendingPathComponent("cssgsg-\(Self.safe(release.version)).pkg")
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try? FileManager.default.removeItem(at: destination)
            try FileManager.default.moveItem(at: location, to: destination)
        } catch {
            state = .failed("파일 저장 실패")
            return
        }
        // 관리자 권한으로 설치할 파일이다. 해시가 없거나 다르면 설치하지 않는다.
        guard Self.fileMatchesDigest(at: destination, expected: release.pkgAsset?.digest) == true else {
            try? FileManager.default.removeItem(at: destination)
            state = .failed("해시 불일치")
            return
        }
        runInstaller(pkg: destination, release: release)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard task === downloadTask, let error, (error as NSError).code != NSURLErrorCancelled else { return }
        state = .failed("내려받기 실패")
    }

    private func runInstaller(pkg: URL, release: GitHubRelease) {
        state = .installing(release)
        let command = Self.installCommand(pkgPath: pkg.path)
        let script = Self.installerScript(command: command, version: release.version)
        // 입력기는 LSUIElement라서, 뒤에서 띄운 암호 창이 키보드 포커스를 못 받을 수 있다(NRIME 8ef120f).
        NSApp.activate(ignoringOtherApps: true)
        DispatchQueue.global(qos: .userInitiated).async {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
            process.arguments = ["-e", script]
            var status: Int32 = -1
            do {
                try process.run()
                Self.spawnRelauncher(waitingFor: process.processIdentifier)
                process.waitUntilExit()
                status = process.terminationStatus
            } catch {}
            DispatchQueue.main.async {
                // 설치에 성공하면 보통 여기 오기 전에 postinstall이 이 프로세스를 끝낸다.
                if status == 0 {
                    try? FileManager.default.removeItem(at: pkg)
                    self.state = .idle
                } else {
                    self.state = .failed("설치 취소 또는 실패")
                }
            }
        }
    }

    /// 설치된 앱의 자리(pkg 설치 위치).
    static let installedAppPath = "/Library/Input Methods/cssgsg.app"

    /// 관리자 권한으로 돌릴 셸 명령: pkg 설치만 한다.
    static func installCommand(pkgPath: String) -> String {
        "/usr/sbin/installer -pkg \(shellQuote(pkgPath)) -target /"
    }

    /// 설치가 끝나면 새 앱을 띄우는 셸 스크립트. osascript(pid)가 끝나기를 기다렸다가 사용자 세션에서 연다.
    ///
    /// root에서 띄우면 안 된다. postinstall 안의 open도, installer 뒤 `launchctl asuser … sudo -u … open`도
    /// "LAUNCH: Asking CSUI to launch 0 items" / procNotFound(-600)로 실패했다(0.1.1, 0.1.3 업데이트 기록).
    /// 사용자 세션의 open은 앱을 끈 직후에도 뜬다. 이 셸은 이름이 cssgsg가 아니라 postinstall의 killall에 같이 죽지 않는다.
    /// 설치를 취소해서 옛 앱이 살아 있으면 open은 아무것도 하지 않는다.
    static func relaunchScript(waitingFor pid: Int32, opener: String = "/usr/bin/open") -> String {
        "while /bin/kill -0 \(pid) 2>/dev/null; do /bin/sleep 0.3; done; /bin/sleep 1; \(opener) -g \(shellQuote(installedAppPath))"
    }

    private static func spawnRelauncher(waitingFor pid: Int32) {
        let shell = Process()
        shell.executableURL = URL(fileURLWithPath: "/bin/sh")
        shell.arguments = ["-c", relaunchScript(waitingFor: pid)]
        try? shell.run()
    }

    /// osascript에 넘길 AppleScript. 셸 명령은 AppleScript 문자열 하나로 감싼다.
    static func installerScript(command: String, version: String) -> String {
        let prompt = "cssgsg \(safe(version)) 업데이트를 설치합니다."
        return "do shell script \(appleScriptLiteral(command)) with administrator privileges with prompt \(appleScriptLiteral(prompt))"
    }

    /// 셸 작은따옴표 인용. 안의 '는 '\''로 바꾼다.
    static func shellQuote(_ text: String) -> String {
        "'" + text.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    /// AppleScript 문자열 리터럴. \ 와 " 를 escape한다.
    static func appleScriptLiteral(_ text: String) -> String {
        "\"" + text.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\""
    }

    /// 버전 문자열에서 파일 이름·안내 문구에 넣어도 되는 글자(ASCII 영숫자, ".", "-")만 남긴다.
    static func safe(_ version: String) -> String {
        String(String.UnicodeScalarView(version.unicodeScalars.filter {
            $0.isASCII && (CharacterSet.alphanumerics.contains($0) || $0 == "." || $0 == "-")
        }))
    }

    /// 파일의 SHA-256을 GitHub 해시("sha256:<hex>")와 비교한다. 해시가 없으면 nil, 읽지 못하면 false.
    static func fileMatchesDigest(at url: URL, expected: String?) -> Bool? {
        guard let expected, expected.lowercased().hasPrefix("sha256:") else { return nil }
        let expectedHex = String(expected.dropFirst("sha256:".count)).lowercased()
        guard let handle = try? FileHandle(forReadingFrom: url) else { return false }
        defer { try? handle.close() }
        var hasher = SHA256()
        while let chunk = try? handle.read(upToCount: 1 << 20), !chunk.isEmpty {
            hasher.update(data: chunk)
        }
        let actualHex = hasher.finalize().map { String(format: "%02x", $0) }.joined()
        return actualHex == expectedHex
    }
}
