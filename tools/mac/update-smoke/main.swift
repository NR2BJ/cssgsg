// 업데이트 코드 스모크 테스트. 설정 앱의 Update/Updater.swift, 공용 SemanticVersion.swift·GitHubRelease.swift와 같이 빌드한다(run.sh).
//
//   update-smoke <fixture.json>                  버전 순서, 채널별 권할 릴리스, GitHub 응답 해석, 해시, 설치 스크립트
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
    // 설치 안내 문구는 화면 언어를 따른다. 시험은 한국어로.
    UILanguage.active = .ko

    // 버전 순서(semver)
    check(SemanticVersion.isNewer(remote: "0.1.1", than: "0.1.0"), "0.1.1 > 0.1.0")
    check(!SemanticVersion.isNewer(remote: "0.1.0", than: "0.1.0"), "같은 버전은 새것이 아님")
    check(!SemanticVersion.isNewer(remote: "0.1.0", than: "0.1.1"), "낮은 버전은 새것이 아님")
    check(SemanticVersion.isNewer(remote: "0.10.0", than: "0.9.9"), "0.10.0 > 0.9.9 (숫자로 비교)")
    check(SemanticVersion.isNewer(remote: "v0.2.0", than: "0.1.9"), "태그의 v를 뗀다")
    check(SemanticVersion.isNewer(remote: "0.1.0", than: "0.1.0-beta.3"), "정식 > 같은 버전의 베타")
    check(!SemanticVersion.isNewer(remote: "garbage", than: "0.1.0"), "읽을 수 없는 태그는 권하지 않음")

    // 권할 릴리스: 정식 채널
    check(UpdateLogic.offers(release("v0.1.1"), over: "0.1.0", channel: .stable), "새 정식 릴리스는 권함")
    check(!UpdateLogic.offers(release("v0.1.0"), over: "0.1.0", channel: .stable), "같은 버전은 안 권함")
    check(!UpdateLogic.offers(release("v0.1.1", pkg: false), over: "0.1.0", channel: .stable), "pkg 없는 릴리스는 안 권함")
    check(!UpdateLogic.offers(release("v0.1.1", draft: true), over: "0.1.0", channel: .stable), "초안은 안 권함")
    check(!UpdateLogic.offers(release("v0.2.0-beta.1", prerelease: true), over: "0.1.0", channel: .stable),
          "정식 채널은 prerelease를 안 권함")
    // 베타 채널: prerelease도 권하고, 목록에서 가장 높은 버전을 고른다
    check(UpdateLogic.offers(release("v0.2.0-beta.1", prerelease: true), over: "0.1.0", channel: .beta),
          "베타 채널은 prerelease를 권함")
    check(!UpdateLogic.offers(release("v0.2.0-beta.1", draft: true, prerelease: true), over: "0.1.0", channel: .beta),
          "베타 채널도 초안은 안 권함")
    let list = [
        release("v0.2.0-beta.2", prerelease: true), release("v0.1.1"), release("v0.2.0-beta.10", prerelease: true),
        release("v0.3.0-beta.1", pkg: false, prerelease: true), release("v0.9.0", draft: true), release("garbage"),
    ]
    check(UpdateLogic.pick(list, over: "0.1.0", channel: .beta)?.version == "0.2.0-beta.10",
          "베타: 목록에서 가장 높은 버전(beta.10 > beta.2, pkg 없는 것·초안 제외)")
    check(UpdateLogic.pick(list, over: "0.1.0", channel: .stable)?.version == "0.1.1", "정식: prerelease 제외")
    check(UpdateLogic.pick(list + [release("v0.2.0")], over: "0.2.0-beta.10", channel: .beta)?.version == "0.2.0",
          "베타를 쓰던 사람도 같은 버전의 정식 릴리스를 받음")
    check(UpdateLogic.pick(list, over: "0.2.0-beta.10", channel: .beta) == nil, "더 높은 것이 없으면 권하지 않음")
    // Mozc 엔진 릴리스(mozc-<판>-<날짜>-<커밋>, prerelease, pkg 없이 cssgsg-mozc.zip)는 앱 업데이트가 아니다.
    // 태그가 버전으로 읽히더라도 pkg가 없어서 권하지 않는다.
    let engine = GitHubRelease(
        tagName: "mozc-1-20261120-c2c2c2c", body: nil, htmlURL: nil, draft: false, prerelease: true,
        assets: [GitHubAsset(name: "cssgsg-mozc.zip", size: 1, browserDownloadURL: "https://example.invalid/cssgsg-mozc.zip",
                             digest: "sha256:00")])
    check(UpdateLogic.pick(list + [engine], over: "0.2.0-beta.10", channel: .beta) == nil
          && !UpdateLogic.offers(engine, over: "0.0.1", channel: .beta) && !UpdateLogic.offers(engine, over: "0.0.1", channel: .stable),
          "Mozc 엔진 릴리스는 어느 채널에서도 앱 업데이트로 권하지 않음")

    // 실제 GitHub 응답 모양. 정식(/releases/latest)은 릴리스 하나, 베타(/releases)는 목록이다.
    do {
        let data = try Data(contentsOf: URL(fileURLWithPath: fixture))
        let r = try JSONDecoder().decode(GitHubRelease.self, from: data)
        check(r.version == "1.0.10", "응답 해석: tag_name → 버전 \(r.version)")
        check(r.pkgAsset?.name.hasSuffix(".pkg") == true, "응답 해석: pkg 파일")
        check(r.pkgAsset?.digest?.hasPrefix("sha256:") == true, "응답 해석: sha256 해시")
        check(r.htmlURL?.hasPrefix("https://github.com/") == true, "응답 해석: 릴리스 페이지 주소")
        check(r.draft == false && r.prerelease == false, "응답 해석: draft/prerelease")
        check(UpdateLogic.decode(data, channel: .stable)?.map(\.version) == ["1.0.10"], "응답 해석: 정식 채널은 하나")
        let listData = Data("[".utf8) + data + Data(",".utf8) + data + Data("]".utf8)
        check(UpdateLogic.decode(listData, channel: .beta)?.count == 2, "응답 해석: 베타 채널은 목록")
        check(UpdateLogic.decode(data, channel: .beta) == nil, "응답 해석: 모양이 다르면 nil")
    } catch {
        check(false, "응답 해석: \(error)")
    }
    check(UpdateChannel.stable.url.path.hasSuffix("/releases/latest"), "정식 채널 주소")
    check(UpdateChannel.beta.url.path.hasSuffix("/releases") && UpdateChannel.beta.url.query == "per_page=100",
          "베타 채널 주소(Mozc 엔진 릴리스가 섞여도 앱 릴리스가 목록에 들도록 100개)")

    // 해시
    let temp = FileManager.default.temporaryDirectory.appendingPathComponent("cssgsg-update-smoke-\(getpid())")
    try? Data("abc".utf8).write(to: temp)
    defer { try? FileManager.default.removeItem(at: temp) }
    let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    check(GitHub.fileMatchesDigest(at: temp, expected: "sha256:" + abc) == true, "해시 일치")
    check(GitHub.fileMatchesDigest(at: temp, expected: "SHA256:" + abc.uppercased()) == true, "해시 대소문자 무시")
    check(GitHub.fileMatchesDigest(at: temp, expected: "sha256:" + String(repeating: "0", count: 64)) == false, "해시 불일치")
    check(GitHub.fileMatchesDigest(at: temp, expected: nil) == nil, "해시 없음 → nil(설치 안 함)")
    check(GitHub.fileMatchesDigest(at: URL(fileURLWithPath: "/nonexistent/x"), expected: "sha256:" + abc) == false,
          "파일 없음 → 불일치")

    // 버전 문자열 거르기
    check(UpdateLogic.safe("0.1.1-beta.2") == "0.1.1-beta.2", "safe: 보통 버전은 그대로")
    check(UpdateLogic.safe("1.0\"; rm -rf /") == "1.0rm-rf", "safe: 따옴표·공백·기호 제거")
    check(UpdateLogic.safe("1.0한") == "1.0", "safe: ASCII만")

    // 셸 인용: 까다로운 글자가 든 문자열이 셸을 거쳐 그대로 돌아오는지
    for text in ["/tmp/a \"b\"/c d\\e's.pkg", "it's", "$(rm -rf /)", "`x`", "a;b&&c|d", "공백 있는 이름"] {
        let echoed = run("/bin/sh", ["-c", "printf '%s' " + UpdateLogic.shellQuote(text)]).out
        check(echoed == text, "셸 인용 왕복: \(text)")
    }

    // 설치 명령(관리자 권한): installer 하나. 셸이 어떤 인자로 받는지
    let pkgPath = "/tmp/a \"b\"/c d\\e's.pkg"
    let command = UpdateLogic.installCommand(pkgPath: pkgPath)
    func argv(_ step: String) -> [String] {
        run("/bin/sh", ["-c", "printf '%s\\n' " + step]).out.split(separator: "\n", omittingEmptySubsequences: false).dropLast().map(String.init)
    }
    check(argv(command) == ["/usr/sbin/installer", "-pkg", pkgPath, "-target", "/"], "설치 명령 인자: \(argv(command))")

    // 설치 셸: 설치(osascript)가 성공하면 1초 뒤 입력기와 설정 앱을 연다(여는 명령은 echo로 바꿔 시험).
    let apple = "do shell script \"it's \\\"quoted\\\" $(x)\""
    let started = Date()
    let ok = run("/bin/sh", ["-c", UpdateLogic.installScript(appleScript: apple, osascript: "/bin/echo", opener: "/bin/echo")])
    let waited = Date().timeIntervalSince(started)
    let lines = ok.out.split(separator: "\n").map(String.init)
    check(ok.status == 0 && lines.count == 3, "설치 셸: 성공하면 세 줄 (\(lines.count))")
    check(lines.first == "-e " + apple, "설치 셸: AppleScript가 osascript에 그대로 간다")
    check(lines.dropFirst().first == "-g \(UpdateLogic.installedAppPath)", "설치 셸: 입력기를 뒤에서 연다")
    check(lines.last == "-a \(UpdateLogic.installedSettingsPath) --args --tab about", "설치 셸: 설정 앱을 정보 탭으로 연다")
    check(waited >= 1, String(format: "설치 셸: 설치가 끝나고 1초 기다린 뒤 연다 (%.2f초)", waited))
    let cancelled = run("/bin/sh", ["-c", UpdateLogic.installScript(appleScript: apple, osascript: "/usr/bin/false", opener: "/bin/echo")])
    check(cancelled.status != 0 && cancelled.out.isEmpty, "설치 셸: 취소·실패하면 아무것도 열지 않는다")

    // AppleScript: 문법 확인 + 리터럴을 되읽으면 셸 명령과 한 글자도 다르지 않은지
    let script = UpdateLogic.installerScript(command: command, version: "0.1.2")
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
    var request = URLRequest(url: UpdateChannel.stable.url)
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
    check(UpdateLogic.offers(r, over: "0.0.1", channel: .stable), "옛 버전 앱은 이 릴리스를 권받음")
    check(!UpdateLogic.offers(r, over: version, channel: .stable), "같은 버전 앱은 권받지 않음")
    // 베타 채널 목록에도 있고, 베타 채널의 옛 버전 앱은 이것이나 더 높은 시험판을 권받는다.
    var betaBody: Data?
    URLSession.shared.dataTask(with: URLRequest(url: UpdateChannel.beta.url)) { data, _, _ in
        betaBody = data
        semaphore.signal()
    }.resume()
    semaphore.wait()
    let betaList = betaBody.flatMap { UpdateLogic.decode($0, channel: .beta) } ?? []
    check(betaList.contains { $0.version == version }, "베타 채널 목록에 \(version)이 있음")
    let betaPick = UpdateLogic.pick(betaList, over: "0.0.1", channel: .beta)
    check(betaPick.map { !SemanticVersion.isNewer(remote: version, than: $0.version) } == true,
          "베타 채널 옛 버전 앱은 \(betaPick?.version ?? "없음")을 권받음(≥ \(version))")
    guard let asset = r.pkgAsset, let url = URL(string: asset.browserDownloadURL) else {
        check(false, "pkg 파일이 붙어 있음")
        return
    }
    check(asset.name == "cssgsg.pkg", "파일 이름 cssgsg.pkg (고정 링크 releases/latest/download/cssgsg.pkg)")
    check(GitHub.fileMatchesDigest(at: URL(fileURLWithPath: pkg), expected: asset.digest) == true,
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
        check(GitHub.fileMatchesDigest(at: downloaded, expected: asset.digest) == true, "받은 파일 해시 일치(앱과 같은 확인)")
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
