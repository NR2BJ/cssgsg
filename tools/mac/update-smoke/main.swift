// 업데이트 코드 스모크 테스트. 셸의 Update/SemanticVersion.swift, Update/Updater.swift와 같이 빌드한다(run.sh).
//
//   update-smoke <fixture.json>                  버전 순서, 권할 릴리스, GitHub 응답 해석, 해시, AppleScript
//   update-smoke --live <pkg 파일> <버전>        GitHub에 올라간 최신 릴리스를 앱과 같은 코드로 읽고, 파일을 받아 해시를 맞춰 본다
import AppKit
import CryptoKit
import Foundation

var failures = 0
func check(_ ok: Bool, _ what: String) {
    print(ok ? "ok   \(what)" : "FAIL \(what)")
    if !ok { failures += 1 }
}

func run(_ tool: String, _ args: [String]) -> (status: Int32, out: String) {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: tool)
    process.arguments = args
    let pipe = Pipe()
    process.standardOutput = pipe
    process.standardError = pipe
    try? process.run()
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    return (process.terminationStatus, String(decoding: data, as: UTF8.self))
}

func release(_ tag: String, pkg: Bool = true, draft: Bool = false, prerelease: Bool = false) -> GitHubRelease {
    let asset = GitHubAsset(name: "cssgsg.pkg", size: 1, browserDownloadURL: "https://example.invalid/cssgsg.pkg",
                            digest: "sha256:00")
    return GitHubRelease(tagName: tag, body: nil, htmlURL: nil, draft: draft, prerelease: prerelease,
                         assets: pkg ? [asset] : [])
}

func offline(fixture: String) {
    // 버전 순서(semver)
    check(SemanticVersion.isNewer(remote: "0.1.1", than: "0.1.0"), "0.1.1 > 0.1.0")
    check(!SemanticVersion.isNewer(remote: "0.1.0", than: "0.1.0"), "같은 버전은 새것이 아님")
    check(!SemanticVersion.isNewer(remote: "0.1.0", than: "0.1.1"), "낮은 버전은 새것이 아님")
    check(SemanticVersion.isNewer(remote: "0.10.0", than: "0.9.9"), "0.10.0 > 0.9.9 (숫자로 비교)")
    check(SemanticVersion.isNewer(remote: "v0.2.0", than: "0.1.9"), "태그의 v를 뗀다")
    check(SemanticVersion.isNewer(remote: "0.1.0", than: "0.1.0-beta.3"), "정식 > 같은 버전의 베타")
    check(!SemanticVersion.isNewer(remote: "garbage", than: "0.1.0"), "읽을 수 없는 태그는 권하지 않음")

    // 권할 릴리스
    check(Updater.offers(release("v0.1.1"), over: "0.1.0"), "새 정식 릴리스는 권함")
    check(!Updater.offers(release("v0.1.0"), over: "0.1.0"), "같은 버전은 안 권함")
    check(!Updater.offers(release("v0.1.1", pkg: false), over: "0.1.0"), "pkg 없는 릴리스는 안 권함")
    check(!Updater.offers(release("v0.1.1", draft: true), over: "0.1.0"), "초안은 안 권함")
    check(!Updater.offers(release("v0.2.0-beta.1", prerelease: true), over: "0.1.0"), "prerelease는 안 권함")

    // 실제 GitHub 응답 모양
    do {
        let data = try Data(contentsOf: URL(fileURLWithPath: fixture))
        let r = try JSONDecoder().decode(GitHubRelease.self, from: data)
        check(r.version == "1.0.10", "응답 해석: tag_name → 버전 \(r.version)")
        check(r.pkgAsset?.name.hasSuffix(".pkg") == true, "응답 해석: pkg 파일")
        check(r.pkgAsset?.digest?.hasPrefix("sha256:") == true, "응답 해석: sha256 해시")
        check(r.htmlURL?.hasPrefix("https://github.com/") == true, "응답 해석: 릴리스 페이지 주소")
        check(r.draft == false && r.prerelease == false, "응답 해석: draft/prerelease")
    } catch {
        check(false, "응답 해석: \(error)")
    }

    // 해시
    let temp = FileManager.default.temporaryDirectory.appendingPathComponent("cssgsg-update-smoke-\(getpid())")
    try? Data("abc".utf8).write(to: temp)
    defer { try? FileManager.default.removeItem(at: temp) }
    let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    check(Updater.fileMatchesDigest(at: temp, expected: "sha256:" + abc) == true, "해시 일치")
    check(Updater.fileMatchesDigest(at: temp, expected: "SHA256:" + abc.uppercased()) == true, "해시 대소문자 무시")
    check(Updater.fileMatchesDigest(at: temp, expected: "sha256:" + String(repeating: "0", count: 64)) == false, "해시 불일치")
    check(Updater.fileMatchesDigest(at: temp, expected: nil) == nil, "해시 없음 → nil(설치 안 함)")
    check(Updater.fileMatchesDigest(at: URL(fileURLWithPath: "/nonexistent/x"), expected: "sha256:" + abc) == false,
          "파일 없음 → 불일치")

    // 버전 문자열 거르기
    check(Updater.safe("0.1.1-beta.2") == "0.1.1-beta.2", "safe: 보통 버전은 그대로")
    check(Updater.safe("1.0\"; rm -rf /") == "1.0rm-rf", "safe: 따옴표·공백·기호 제거")
    check(Updater.safe("1.0한") == "1.0", "safe: ASCII만")

    // 설치 AppleScript: 문법 확인 + 셸에 넘어가는 명령
    for path in ["/Users/me/Library/Caches/com.cssgsg.inputmethod.app/updates/cssgsg-0.1.1.pkg",
                 "/tmp/a \"b\"/c d\\e's.pkg"] {
        let script = Updater.installerScript(pkgPath: path, version: "0.1.1")
        let compiled = run("/usr/bin/osacompile", ["-e", script, "-o", temp.path + ".scpt"])
        try? FileManager.default.removeItem(atPath: temp.path + ".scpt")
        check(compiled.status == 0, "AppleScript 문법: \(path) \(compiled.status == 0 ? "" : compiled.out)")
        check(script.contains("with administrator privileges") && script.contains("with prompt"),
              "AppleScript: 관리자 권한 + 안내 문구")
        // do shell script가 받을 명령 문자열을 그대로 만들어 보고, 셸이 경로를 한 인자로 받는지 본다.
        let expression = script.replacingOccurrences(of: "do shell script ", with: "")
            .components(separatedBy: " with administrator privileges")[0]
        let command = run("/usr/bin/osascript", ["-e", expression]).out.trimmingCharacters(in: .newlines)
        let argv = run("/bin/sh", ["-c", "printf '%s\\n' " + command.replacingOccurrences(of: "/usr/sbin/installer ", with: "")])
            .out.split(separator: "\n").map(String.init)
        check(argv == ["-pkg", path, "-target", "/"], "셸 인자: \(argv)")
    }
}

func setupPlan() {
    typealias S = InputSourceSetup.Source
    let method = { (on: Bool) in S(id: "app", isMethod: true, enabled: on, enableCapable: true) }
    let mode = { (on: Bool) in S(id: "app.en", isMethod: false, enabled: on, enableCapable: true) }
    check(InputSourceSetup.plan([method(false), mode(false)], firstRun: true) == ["app", "app.en"],
          "입력 소스: 처음 실행이면 본체와 모드를 모두 켠다(본체 먼저)")
    check(InputSourceSetup.plan([mode(false), method(false)], firstRun: true) == ["app", "app.en"],
          "입력 소스: 목록 순서와 상관없이 본체 먼저")
    check(InputSourceSetup.plan([method(true), mode(true)], firstRun: true).isEmpty, "입력 소스: 이미 켜져 있으면 그대로")
    check(InputSourceSetup.plan([method(false), mode(true)], firstRun: false) == ["app"],
          "입력 소스: 모드만 켜진 반쪽 상태(0.1.0)면 본체를 켠다")
    check(InputSourceSetup.plan([method(false), mode(false)], firstRun: false).isEmpty,
          "입력 소스: 사용자가 뺐으면(둘 다 꺼짐) 건드리지 않는다")
    check(InputSourceSetup.plan([method(true), mode(false)], firstRun: false).isEmpty,
          "입력 소스: 모드를 끈 것도 건드리지 않는다")
    let stuck = S(id: "app", isMethod: true, enabled: false, enableCapable: false)
    check(InputSourceSetup.plan([stuck, mode(true)], firstRun: false).isEmpty, "입력 소스: 켤 수 없는 것은 건너뛴다")
}

func live(pkg: String, version: String) {
    let semaphore = DispatchSemaphore(value: 0)
    var body: Data?
    var status = 0
    var request = URLRequest(url: Updater.latestReleaseURL)
    request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
    URLSession.shared.dataTask(with: request) { data, response, _ in
        body = data
        status = (response as? HTTPURLResponse)?.statusCode ?? 0
        semaphore.signal()
    }.resume()
    semaphore.wait()
    guard status == 200, let body, let r = try? JSONDecoder().decode(GitHubRelease.self, from: body) else {
        check(false, "최신 릴리스 읽기 (HTTP \(status))")
        return
    }
    check(r.version == version, "최신 릴리스 = \(r.version) (기대 \(version))")
    check(Updater.offers(r, over: "0.0.1"), "옛 버전 앱은 이 릴리스를 권받음")
    check(!Updater.offers(r, over: version), "같은 버전 앱은 권받지 않음")
    guard let asset = r.pkgAsset, let url = URL(string: asset.browserDownloadURL) else {
        check(false, "pkg 파일이 붙어 있음")
        return
    }
    check(asset.name == "cssgsg.pkg", "파일 이름 cssgsg.pkg (고정 링크 releases/latest/download/cssgsg.pkg)")
    check(Updater.fileMatchesDigest(at: URL(fileURLWithPath: pkg), expected: asset.digest) == true,
          "GitHub 해시 = 로컬 pkg 해시")
    var downloaded: URL?
    URLSession.shared.downloadTask(with: url) { location, _, _ in
        if let location {
            let keep = FileManager.default.temporaryDirectory.appendingPathComponent("cssgsg-live-\(getpid()).pkg")
            try? FileManager.default.removeItem(at: keep)
            try? FileManager.default.moveItem(at: location, to: keep)
            downloaded = keep
        }
        semaphore.signal()
    }.resume()
    semaphore.wait()
    if let downloaded {
        check(Updater.fileMatchesDigest(at: downloaded, expected: asset.digest) == true, "받은 파일 해시 일치(앱과 같은 확인)")
        try? FileManager.default.removeItem(at: downloaded)
    } else {
        check(false, "파일 받기")
    }
}

let args = Array(CommandLine.arguments.dropFirst())
if args.first == "--live", args.count == 3 {
    live(pkg: args[1], version: args[2])
} else if args.first == "--setup-plan", args.count == 2 {
    // 이 컴퓨터의 실제 상태로 계획만 세워 본다(켜지는 않는다).
    let found = InputSourceSetup.installed(args[1])
    for item in found {
        print("  \(item.info.id) 본체=\(item.info.isMethod) 켜짐=\(item.info.enabled) 켜기가능=\(item.info.enableCapable)")
    }
    print("처음 실행이면 켤 것:", InputSourceSetup.plan(found.map(\.info), firstRun: true))
    print("그 뒤 실행이면 켤 것:", InputSourceSetup.plan(found.map(\.info), firstRun: false))
    exit(0)
} else if args.count == 1 {
    offline(fixture: args[0])
    setupPlan()
} else {
    print("사용법: update-smoke <fixture.json> | update-smoke --live <pkg> <버전>")
    exit(2)
}
print(failures == 0 ? "모두 통과" : "실패 \(failures)개")
exit(failures == 0 ? 0 : 1)
