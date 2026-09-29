// 업데이트 코드 스모크 테스트. 셸의 Update/SemanticVersion.swift, Update/Updater.swift와 같이 빌드한다(run.sh).
//
//   update-smoke <fixture.json>                  버전 순서, 권할 릴리스, GitHub 응답 해석, 해시, AppleScript
//   update-smoke --live <pkg 파일> <버전>        GitHub에 올라간 최신 릴리스를 앱과 같은 코드로 읽고, 파일을 받아 해시를 맞춰 본다
import AppKit
import Carbon
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

    // 셸 인용: 까다로운 글자가 든 문자열이 셸을 거쳐 그대로 돌아오는지
    for text in ["/tmp/a \"b\"/c d\\e's.pkg", "it's", "$(rm -rf /)", "`x`", "a;b&&c|d", "공백 있는 이름"] {
        let echoed = run("/bin/sh", ["-c", "printf '%s' " + Updater.shellQuote(text)]).out
        check(echoed == text, "셸 인용 왕복: \(text)")
    }

    // 설치 명령(관리자 권한): installer 하나. 셸이 어떤 인자로 받는지
    let pkgPath = "/tmp/a \"b\"/c d\\e's.pkg"
    let command = Updater.installCommand(pkgPath: pkgPath)
    func argv(_ step: String) -> [String] {
        run("/bin/sh", ["-c", "printf '%s\\n' " + step]).out.split(separator: "\n", omittingEmptySubsequences: false).dropLast().map(String.init)
    }
    check(argv(command) == ["/usr/sbin/installer", "-pkg", pkgPath, "-target", "/"], "설치 명령 인자: \(argv(command))")

    // 다시 띄우기: osascript가 끝날 때까지 기다렸다가 사용자 세션에서 연다(여는 명령은 echo로 바꿔 시험)
    let sleeper = Process()
    sleeper.executableURL = URL(fileURLWithPath: "/bin/sleep")
    sleeper.arguments = ["0.6"]
    try? sleeper.run()
    let started = Date()
    let relaunch = run("/bin/sh", ["-c", Updater.relaunchScript(waitingFor: sleeper.processIdentifier, opener: "/bin/echo")])
    let waited = Date().timeIntervalSince(started)
    check(relaunch.out == "-g \(Updater.installedAppPath)\n", "다시 띄우기: open -g 설치 자리 (\(relaunch.out.trimmingCharacters(in: .newlines)))")
    check(waited >= 1.5, String(format: "다시 띄우기: 설치(0.6초)가 끝나고 1초 더 기다린 뒤 연다 (%.2f초)", waited))

    // AppleScript: 문법 확인 + 리터럴을 되읽으면 셸 명령과 한 글자도 다르지 않은지
    let script = Updater.installerScript(command: command, version: "0.1.2")
    let compiled = run("/usr/bin/osacompile", ["-e", script, "-o", temp.path + ".scpt"])
    try? FileManager.default.removeItem(atPath: temp.path + ".scpt")
    check(compiled.status == 0, "AppleScript 문법 \(compiled.status == 0 ? "" : compiled.out)")
    check(script.hasPrefix("do shell script ") && script.contains(" with administrator privileges with prompt "),
          "AppleScript: 관리자 권한 + 안내 문구")
    let literal = script.dropFirst("do shell script ".count).components(separatedBy: " with administrator privileges")[0]
    let roundTrip = run("/usr/bin/osascript", ["-e", "return " + literal]).out
    check(roundTrip.hasSuffix("\n") && String(roundTrip.dropLast()) == command, "AppleScript 리터럴 왕복")
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
} else if args.first == "--input-status" {
    // 이 컴퓨터에서 cssgsg 입력 소스가 어떻게 보이는지(바꾸지 않는다).
    let id = "com.cssgsg.inputmethod.app.en"
    let condition = [kTISPropertyInputSourceID as String: id] as CFDictionary
    let installed = (TISCreateInputSourceList(condition, true)?.takeRetainedValue() as? [TISInputSource]) ?? []
    let name = installed.first.flatMap { TISGetInputSourceProperty($0, kTISPropertyLocalizedName) }
        .map { Unmanaged<CFString>.fromOpaque($0).takeUnretainedValue() as String } ?? "(없음)"
    print("설치됨: \(!installed.isEmpty), 목록에 보이는 이름: \(name), 입력 소스에 추가됨: \(InputSourceSetup.isAdded)")
    exit(0)
} else if args.count == 1 {
    offline(fixture: args[0])
} else {
    print("사용법: update-smoke <fixture.json> | update-smoke --live <pkg> <버전>")
    exit(2)
}
print(failures == 0 ? "모두 통과" : "실패 \(failures)개")
exit(failures == 0 ? 0 : 1)
