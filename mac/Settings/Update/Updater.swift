// NRIME(github.com/NR2BJ/NRIME)의 Shared/UpdateManager.swift를 줄여서 가져왔다.
// - 채널은 정식(stable)과 베타(beta)다. 정식은 GitHub /releases/latest, 베타는 /releases 목록에서 가장 높은 버전
//   (prerelease 포함). 채널마다 ETag와 받은 응답을 따로 둔다.
// - 확인은 설정 앱만 한다(정보 탭을 열 때마다, "지금 확인", 채널을 바꿀 때). 입력기 프로세스는 네트워크를 쓰지 않는다.
//   0.4.0까지는 입력기 메뉴가 확인했는데, 입력기에서 창을 띄우면 메인 스레드가 막혀 모든 앱의 입력이 멈출 수 있어
//   메뉴 항목뿐이었다. 설정 앱에서는 릴리스 노트와 진행을 제대로 보인다.
// - 같은 버전 재업로드 감지는 뺐다. 릴리스 스크립트가 이미 있는 태그를 거부하므로 버전은 늘 올라간다.
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

enum UpdateChannel: String, CaseIterable, Identifiable {
    case stable, beta

    var id: String { rawValue }

    var name: String {
        switch self {
        case .stable: return tr("정식", "Stable", "正式版")
        case .beta: return tr("베타", "Beta", "ベータ")
        }
    }

    /// 정식은 최신 정식 릴리스 하나(prerelease·초안 제외), 베타는 최근 릴리스 목록.
    var url: URL {
        switch self {
        case .stable: return URL(string: "https://api.github.com/repos/NR2BJ/cssgsg/releases/latest")!
        case .beta: return URL(string: "https://api.github.com/repos/NR2BJ/cssgsg/releases?per_page=20")!
        }
    }
}

/// 업데이트 판단과 설치 명령. 상태가 없어서 스모크 테스트(tools/mac/update-smoke)가 그대로 부른다.
enum UpdateLogic {
    /// 이 릴리스를 설치하라고 권할지: 초안이 아니고, pkg가 붙어 있고, 지금보다 높은 버전.
    /// 정식 채널은 prerelease를 권하지 않는다.
    static func offers(_ release: GitHubRelease, over current: String, channel: UpdateChannel) -> Bool {
        release.draft != true && release.pkgAsset != nil
            && (channel == .beta || release.prerelease != true)
            && SemanticVersion.isNewer(remote: release.version, than: current)
    }

    /// 권할 릴리스 중 가장 높은 버전. 없으면 nil.
    static func pick(_ releases: [GitHubRelease], over current: String, channel: UpdateChannel) -> GitHubRelease? {
        releases.filter { offers($0, over: current, channel: channel) }
            .compactMap { release in SemanticVersion(release.version).map { (release, $0) } }
            .max { $0.1 < $1.1 }?.0
    }

    /// GitHub 응답 본문 → 릴리스들. 정식은 릴리스 하나, 베타는 목록이다.
    static func decode(_ data: Data, channel: UpdateChannel) -> [GitHubRelease]? {
        switch channel {
        case .stable: return (try? JSONDecoder().decode(GitHubRelease.self, from: data)).map { [$0] }
        case .beta: return try? JSONDecoder().decode([GitHubRelease].self, from: data)
        }
    }

    /// 설치 자리(pkg 설치 위치).
    static let installedAppPath = "/Library/Input Methods/cssgsg.app"
    static let installedSettingsPath = "/Library/Input Methods/cssgsgSettings.app"

    /// 관리자 권한으로 돌릴 셸 명령: pkg 설치만 한다.
    static func installCommand(pkgPath: String) -> String {
        "/usr/sbin/installer -pkg \(shellQuote(pkgPath)) -target /"
    }

    /// osascript에 넘길 AppleScript. 셸 명령은 AppleScript 문자열 하나로 감싼다.
    static func installerScript(command: String, version: String) -> String {
        let prompt = tr("cssgsg \(safe(version)) 업데이트를 설치합니다.", "Installing the cssgsg \(safe(version)) update.",
                        "cssgsg \(safe(version)) のアップデートをインストールします。")
        return "do shell script \(appleScriptLiteral(command)) with administrator privileges with prompt \(appleScriptLiteral(prompt))"
    }

    /// 설치하고 다시 띄우는 셸 스크립트. 설치(osascript)가 성공하면 1초 뒤 사용자 세션에서 입력기와 설정 앱을 연다.
    ///
    /// - postinstall이 옛 입력기와 설정 앱(이 셸을 띄운 프로세스)을 끝내도, 이 셸은 이름이 달라 같이 죽지 않는다.
    /// - root에서 띄우면 안 된다. postinstall 안의 open도, installer 뒤 `launchctl asuser … sudo -u … open`도
    ///   "LAUNCH: Asking CSUI to launch 0 items" / procNotFound(-600)로 실패했다(0.1.1, 0.1.3 업데이트 기록).
    ///   사용자 세션의 open은 앱을 끈 직후에도 뜬다.
    /// - 취소하거나 실패하면 아무것도 열지 않고 osascript의 종료 코드로 끝난다.
    static func installScript(appleScript: String, osascript: String = "/usr/bin/osascript",
                              opener: String = "/usr/bin/open") -> String {
        "\(osascript) -e \(shellQuote(appleScript)) || exit $?; /bin/sleep 1; "
            + "\(opener) -g \(shellQuote(installedAppPath)); "
            + "\(opener) -a \(shellQuote(installedSettingsPath)) --args --tab about"
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

/// GitHub 릴리스로 새 버전을 확인하고, 받아서 설치한다(설정 앱 정보 탭). 메인 스레드에서만 쓴다.
final class Updater: NSObject, ObservableObject, URLSessionDownloadDelegate {
    enum State {
        case idle
        case checking
        case upToDate
        case available(GitHubRelease)
        case downloading(GitHubRelease, progress: Double)
        case installing(GitHubRelease)
        case failed(String)

        var isBusy: Bool {
            switch self {
            case .checking, .downloading, .installing: return true
            default: return false
            }
        }
    }

    @Published private(set) var state: State = .idle
    @Published private(set) var lastCheck: Date?
    /// 채널을 바꾸면 그 채널로 바로 확인한다.
    @Published var channel: UpdateChannel {
        didSet {
            guard channel != oldValue else { return }
            defaults.set(channel.rawValue, forKey: Self.channelKey)
            // 설치 중에는 채널을 못 바꾼다(정보 탭이 막는다). 받는 중이면 그만둔다.
            downloadTask?.cancel()
            downloadTask = nil
            downloading = nil
            state = .idle
            lastCheck = lastCheckDate(channel)
            check(userInitiated: true)
        }
    }

    /// 정보 탭을 열 때 이만큼 지났으면 확인한다. 0.5.0은 NRIME처럼 하루였는데, 그날 한 번 본 뒤에는 새 릴리스가 나와도
    /// 다음 날까지 "최신 버전이다"로 보였다(사용자가 0.5.0에 머물러 있었다). 바뀐 게 없으면 GitHub는 304만 준다(ETag).
    private static let checkInterval: TimeInterval = 60
    private static let channelKey = "updateChannel"

    static var currentVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.0.0"
    }

    static var cacheDirectory: URL {
        FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("com.cssgsg.settings/updates", isDirectory: true)
    }

    private let defaults = UserDefaults.standard
    private var downloadTask: URLSessionDownloadTask?
    private var downloading: GitHubRelease?
    private lazy var downloadSession = URLSession(configuration: .default, delegate: self, delegateQueue: .main)

    override init() {
        channel = UserDefaults.standard.string(forKey: Self.channelKey).flatMap(UpdateChannel.init(rawValue:)) ?? .stable
        super.init()
        lastCheck = lastCheckDate(channel)
    }

    // 채널마다 따로 두는 값
    private func key(_ name: String, _ channel: UpdateChannel) -> String { "update.\(channel.rawValue).\(name)" }

    private func lastCheckDate(_ channel: UpdateChannel) -> Date? {
        let t = defaults.double(forKey: key("lastCheck", channel))
        return t > 0 ? Date(timeIntervalSince1970: t) : nil
    }

    // MARK: - 확인

    /// 정보 탭을 열 때: 1분이 지났으면 확인하고, 아니면 지난번 응답으로 상태를 보인다.
    func checkIfDue() {
        if let last = lastCheckDate(channel), Date().timeIntervalSince(last) < Self.checkInterval {
            if case .idle = state { evaluate(cachedReleases(channel)) }
            return
        }
        check(userInitiated: false)
    }

    /// 지금 확인한다. 사람이 누른 확인이면 네트워크 실패도 보인다. 자동 확인의 네트워크 실패는 조용히 넘긴다.
    func check(userInitiated: Bool) {
        guard !state.isBusy else { return }
        let channel = channel
        state = .checking
        var request = URLRequest(url: channel.url, timeoutInterval: 20)
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        request.setValue("cssgsg/\(Self.currentVersion)", forHTTPHeaderField: "User-Agent")
        // 조건부 요청: 바뀐 게 없으면 304라서 GitHub의 비인증 요청 한도(시간당 60)를 아낀다.
        if let etag = defaults.string(forKey: key("etag", channel)), cachedReleases(channel) != nil {
            request.setValue(etag, forHTTPHeaderField: "If-None-Match")
        }
        URLSession.shared.dataTask(with: request) { [weak self] data, response, error in
            DispatchQueue.main.async {
                guard let self, self.channel == channel else { return }  // 그새 채널을 바꿨다
                self.finishCheck(channel, data, response as? HTTPURLResponse, error, userInitiated: userInitiated)
            }
        }.resume()
    }

    private func finishCheck(_ channel: UpdateChannel, _ data: Data?, _ response: HTTPURLResponse?, _ error: Error?,
                             userInitiated: Bool) {
        guard let response, error == nil else {
            state = userInitiated ? .failed(tr("서버에 연결하지 못했습니다", "Couldn’t reach the server", "サーバーに接続できませんでした")) : .idle
            return
        }
        let now = Date()
        defaults.set(now.timeIntervalSince1970, forKey: key("lastCheck", channel))
        lastCheck = now
        switch response.statusCode {
        case 304:
            evaluate(cachedReleases(channel))
        case 200:
            guard let data, let releases = UpdateLogic.decode(data, channel: channel) else {
                state = .failed(tr("릴리스 정보를 읽지 못했습니다", "Couldn’t read the release info", "リリース情報を読めませんでした"))
                return
            }
            // 본문을 읽은 뒤에만 ETag를 남긴다. 그래야 304가 늘 읽어 둔 응답을 가리킨다.
            defaults.set(data, forKey: key("body", channel))
            if let etag = response.value(forHTTPHeaderField: "ETag") {
                defaults.set(etag, forKey: key("etag", channel))
            }
            evaluate(releases)
        case 404:
            state = .upToDate  // 아직 릴리스가 없다
        default:
            state = userInitiated ? .failed(tr("GitHub가 \(response.statusCode)로 응답했습니다", "GitHub responded \(response.statusCode)",
                                               "GitHub が \(response.statusCode) を返しました")) : .idle
        }
    }

    private func evaluate(_ releases: [GitHubRelease]?) {
        guard let releases else {
            state = .idle
            return
        }
        if let release = UpdateLogic.pick(releases, over: Self.currentVersion, channel: channel) {
            state = .available(release)
        } else {
            state = .upToDate
        }
    }

    private func cachedReleases(_ channel: UpdateChannel) -> [GitHubRelease]? {
        guard let data = defaults.data(forKey: key("body", channel)) else { return nil }
        return UpdateLogic.decode(data, channel: channel)
    }

    // MARK: - 설치

    /// 받아서 해시를 확인하고 설치한다. 설치가 끝나면 postinstall이 이 앱과 입력기를 끝내고, 설치 셸이 새 버전을 띄운다.
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
        let destination = directory.appendingPathComponent("cssgsg-\(UpdateLogic.safe(release.version)).pkg")
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try? FileManager.default.removeItem(at: destination)
            try FileManager.default.moveItem(at: location, to: destination)
        } catch {
            state = .failed(tr("받은 파일을 저장하지 못했습니다", "Couldn’t save the download", "ダウンロードを保存できませんでした"))
            return
        }
        // 관리자 권한으로 설치할 파일이다. 해시가 없거나 다르면 설치하지 않는다.
        guard UpdateLogic.fileMatchesDigest(at: destination, expected: release.pkgAsset?.digest) == true else {
            try? FileManager.default.removeItem(at: destination)
            state = .failed(tr("받은 파일의 해시가 맞지 않습니다", "The download’s checksum doesn’t match",
                               "ダウンロードのハッシュが一致しません"))
            return
        }
        runInstaller(pkg: destination, release: release)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard task === downloadTask, let error, (error as NSError).code != NSURLErrorCancelled else { return }
        state = .failed(tr("내려받지 못했습니다", "Download failed", "ダウンロードに失敗しました"))
    }

    private func runInstaller(pkg: URL, release: GitHubRelease) {
        state = .installing(release)
        let command = UpdateLogic.installCommand(pkgPath: pkg.path)
        let script = UpdateLogic.installScript(appleScript: UpdateLogic.installerScript(command: command, version: release.version))
        DispatchQueue.global(qos: .userInitiated).async {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/bin/sh")
            process.arguments = ["-c", script]
            var status: Int32 = -1
            do {
                try process.run()
                process.waitUntilExit()
                status = process.terminationStatus
            } catch {}
            DispatchQueue.main.async {
                // 설치에 성공하면 보통 여기 오기 전에 postinstall이 이 앱을 끝낸다(설치 셸이 새 앱을 띄운다).
                if status == 0 {
                    try? FileManager.default.removeItem(at: pkg)
                    self.state = .idle
                } else {
                    self.state = .failed(tr("설치를 취소했거나 설치하지 못했습니다", "Installation was cancelled or failed",
                                            "インストールが取り消されたか、失敗しました"))
                }
            }
        }
    }
}
