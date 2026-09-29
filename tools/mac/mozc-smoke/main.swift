// Mozc 엔진 스모크 테스트: 앱에 든 엔진과 받은 엔진 고르기, 잘못된 엔진 막기, 업데이트 확인과 설치,
// 그리고 엔진 워크플로가 내는 묶음(tools/mozc/package-component.sh)이 받아져 읽히고 변환하는지.
// NRIME의 MozcComponentTests·MozcDownloadedEngineTests를 옮겼다. 입력기의 Mozc 코드와 코어를 그대로 붙여 빌드한다(run.sh).
//   mozc-smoke <저장소 폴더>
// 받은 엔진 폴더, 학습 폴더는 임시 폴더를 쓴다. 이 컴퓨터의 입력기 상태는 건드리지 않는다.
import CryptoKit
import Foundation

var failures = 0
func check(_ cond: Bool, _ what: String) {
    print(cond ? "ok   \(what)" : "FAIL \(what)")
    if !cond { failures += 1 }
}

let fm = FileManager.default
let repository = URL(fileURLWithPath: CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : ".")
let work = fm.temporaryDirectory.appendingPathComponent("cssgsg-mozc-smoke-\(UUID().uuidString)")
let profile = work.appendingPathComponent("profile")
defer { try? fm.removeItem(at: work) }

let abi = MozcComponents.supportedABI
check(abi == 1, "코어가 아는 C API 판 = 1 (mozc 기능으로 빌드한 코어)")

/// 저장소에서 빌드한 엔진(tools/mozc/build.sh). 앱에 든 엔진 노릇을 한다.
let out = repository.appendingPathComponent("build/mozc-out")
let realBundled: MozcComponent = {
    let line = (try? String(contentsOf: out.appendingPathComponent("MOZC_VERSION"), encoding: .utf8)) ?? ""
    let parts = line.split(whereSeparator: \.isWhitespace).map(String.init)
    guard parts.count >= 3 else { fatalError("build/mozc-out/MOZC_VERSION이 없다. bash tools/mozc/build.sh를 먼저") }
    return MozcComponent(source: .bundled, libraryURL: out.appendingPathComponent("lib/libcssgsg_mozc.dylib"),
                         dataURL: out.appendingPathComponent("data/mozc.data"),
                         commit: parts[0], date: parts[1], version: parts[2], directory: nil)
}()

let fakeBundled = MozcComponent(
    source: .bundled, libraryURL: URL(fileURLWithPath: "/nonexistent/libcssgsg_mozc.dylib"),
    dataURL: URL(fileURLWithPath: "/nonexistent/mozc.data"),
    commit: "a069a88d4cb5c011de0f9aebb6c149a1c808d904", date: "2026-09-28", version: "3.34.6239.101", directory: nil)

/// 받은 엔진 폴더를 새로 만들고 앱에 든 엔진을 정한다.
func fresh(bundled: MozcComponent? = fakeBundled) -> URL {
    let downloads = work.appendingPathComponent("engines-\(UUID().uuidString)")
    try! fm.createDirectory(at: downloads, withIntermediateDirectories: true)
    MozcComponents.downloadsDirectory = downloads
    MozcComponents.bundled = bundled
    return downloads
}

/// 가짜 파일로 받은 엔진 하나.
@discardableResult
func addDownload(_ downloads: URL, commit: String, date: String, version: String = "3.35.1.101",
                 abi: Int = abi, bad: Bool = false) -> URL {
    let directory = downloads.appendingPathComponent(commit)
    try! fm.createDirectory(at: directory, withIntermediateDirectories: true)
    try! Data("lib".utf8).write(to: directory.appendingPathComponent(MozcComponents.libraryName))
    try! Data("data".utf8).write(to: directory.appendingPathComponent(MozcComponents.dataName))
    let manifest = MozcComponents.Manifest(abi: abi, commit: commit, date: date, version: version, files: [:])
    try! JSONEncoder().encode(manifest).write(to: directory.appendingPathComponent(MozcComponents.manifestName))
    if bad {
        try! Data().write(to: directory.appendingPathComponent(MozcComponents.State.bad.rawValue))
    }
    return directory
}

// MARK: - 고르기

do {
    let downloads = fresh()
    addDownload(downloads, commit: "b1", date: "2026-10-10")
    addDownload(downloads, commit: "c2", date: "2026-11-20")
    addDownload(downloads, commit: "old", date: "2026-09-01")              // 앱에 든 것보다 오래됨
    addDownload(downloads, commit: "bad", date: "2026-12-01", bad: true)    // 나쁜 엔진
    addDownload(downloads, commit: "abi2", date: "2026-12-24", abi: 2)      // 모르는 C API 판
    check(MozcComponents.candidates().map(\.commit) == ["c2", "b1", fakeBundled.commit],
          "차례: 새로 받은 것(새것부터) → 앱에 든 것. 오래된 것·나쁜 것·판이 다른 것은 빠진다")
    let same = MozcComponent(source: .downloaded, libraryURL: fakeBundled.libraryURL, dataURL: fakeBundled.dataURL,
                             commit: fakeBundled.commit, date: "2027-01-01", version: fakeBundled.version, directory: nil)
    check(!MozcComponents.isNewer(same, than: fakeBundled), "같은 커밋은 날짜가 달라도 새것이 아니다")
}

// MARK: - 지키기

do {
    let downloads = fresh()
    let directory = addDownload(downloads, commit: "b1", date: "2026-10-10")
    let component = MozcComponents.downloaded()[0]
    check(MozcComponents.beginLoad(component), "처음 읽기")
    // 여기서 프로세스가 죽었다: endLoad가 불리지 않았다.
    check(!MozcComponents.beginLoad(component), "읽다가 죽은 엔진은 다시 읽지 않는다")
    check(fm.fileExists(atPath: directory.appendingPathComponent("bad").path), "…나쁜 엔진으로 적는다")
    check(MozcComponents.candidates().map(\.commit) == [fakeBundled.commit], "…그러면 앱에 든 엔진을 쓴다")
}

do {
    _ = fresh()
    addDownload(MozcComponents.downloadsDirectory, commit: "b1", date: "2026-10-10")
    let component = MozcComponents.downloaded()[0]
    let now = Date()
    check(MozcComponents.beginLoad(component, now: now.addingTimeInterval(-300)), "죽고 다시 뜨기: 첫 시작")
    MozcComponents.endLoad(component)
    check(MozcComponents.beginLoad(component, now: now.addingTimeInterval(-120)), "죽고 다시 뜨기: 둘째 시작")
    MozcComponents.endLoad(component)
    check(!MozcComponents.beginLoad(component, now: now), "10분 안에 깨끗이 끝나지 않은 시작이 세 번이면 나쁜 엔진")
    check(MozcComponents.downloaded().isEmpty, "…그 엔진은 더 쓰지 않는다")
}

do {
    _ = fresh()
    addDownload(MozcComponents.downloadsDirectory, commit: "b1", date: "2026-10-10")
    let component = MozcComponents.downloaded()[0]
    let now = Date()
    var allowed = true
    for minutesAgo in [9.0, 7.0, 5.0, 3.0, 1.0, 0.0] {
        let start = now.addingTimeInterval(-minutesAgo * 60)
        allowed = allowed && MozcComponents.beginLoad(component, now: start)
        MozcComponents.endLoad(component)
        MozcComponents.endRun(component, startedAt: start)  // 깨끗이 끝났다(다시 시작, 지금 적용)
    }
    check(allowed && MozcComponents.downloaded().count == 1, "깨끗이 끝난 시작은 10분에 여섯 번이어도 세지 않는다")
}

do {
    _ = fresh()
    addDownload(MozcComponents.downloadsDirectory, commit: "b1", date: "2026-10-10")
    let component = MozcComponents.downloaded()[0]
    let now = Date()
    var allowed = true
    for hoursAgo in [5.0, 3.0, 1.0, 0.0] {
        allowed = allowed && MozcComponents.beginLoad(component, now: now.addingTimeInterval(-hoursAgo * 3600))
        MozcComponents.endLoad(component)
    }
    check(allowed, "몇 시간씩 떨어진 시작은 괜찮다")
}

do {
    let downloads = fresh()
    addDownload(downloads, commit: "a", date: "2026-10-01")
    addDownload(downloads, commit: "b", date: "2026-10-02")
    addDownload(downloads, commit: "c", date: "2026-10-03")
    let bad = addDownload(downloads, commit: "x", date: "2026-10-04", bad: true)
    MozcComponents.prune(keeping: nil)
    check(Set(MozcComponents.downloaded().map(\.commit)) == ["b", "c"], "정리: 좋은 엔진은 새것 둘만 남긴다")
    check(fm.fileExists(atPath: bad.appendingPathComponent("bad").path)
          && !fm.fileExists(atPath: bad.appendingPathComponent(MozcComponents.dataName).path),
          "정리: 나쁜 엔진은 표시만 남긴다(19MB는 지운다)")
    check(MozcComponents.badCommits() == ["x"], "나쁜 엔진의 커밋은 다시 받지 않는다")
}

// MARK: - 진짜 엔진: 잘못 받은 것은 버리고 앱에 든 것을 쓴다

do {
    let downloads = fresh(bundled: realBundled)
    let broken = addDownload(downloads, commit: "fffffff", date: "2099-01-01")  // "lib"은 라이브러리가 아니다
    let engine = CoreEngine(configTOML: nil)
    let active = MozcLoader.start(engine, profile: profile)
    check(active?.source == .bundled && active?.commit == realBundled.commit, "받은 엔진이 망가졌으면 앱에 든 엔진을 쓴다")
    check(fm.fileExists(atPath: broken.appendingPathComponent("bad").path), "…망가진 엔진은 나쁜 엔진으로 적는다")
    let reason = (try? String(contentsOf: broken.appendingPathComponent("bad"), encoding: .utf8)) ?? ""
    check(reason.contains("dlopen"), "…까닭은 코어가 알려 준다(\(reason.prefix(60))…)")
}

do {
    _ = fresh(bundled: nil)
    let engine = CoreEngine(configTOML: nil)
    check(MozcLoader.start(engine, profile: profile) == nil, "엔진이 하나도 없으면 nil(일본어는 가나만)")
}

// MARK: - 업데이트 확인

func release(_ tag: String, asset: String = MozcUpdater.assetName, digest: String? = "sha256:00",
             draft: Bool = false) -> GitHubRelease {
    let json: [String: Any] = [
        "tag_name": tag, "prerelease": true, "draft": draft,
        "assets": [[
            "name": asset, "size": 1,
            "browser_download_url": "https://github.com/NR2BJ/cssgsg/releases/download/\(tag)/\(asset)",
            "digest": digest.map { $0 as Any } ?? NSNull(),
        ]],
    ]
    return try! JSONDecoder().decode(GitHubRelease.self, from: JSONSerialization.data(withJSONObject: json))
}

do {
    let offer = MozcUpdater.offer(from: release("mozc-1-20261120-c2c2c2c"))
    check(offer?.abi == 1 && offer?.date == "2026-11-20" && offer?.commitPrefix == "c2c2c2c", "엔진 릴리스를 읽는다")
    check(MozcUpdater.offer(from: release("v0.6.0", asset: "cssgsg-0.6.0.pkg")) == nil, "앱 릴리스는 엔진이 아니다")
    check(MozcUpdater.offer(from: release("mozc-1-2026112-c2c2c2c")) == nil, "날짜가 틀린 태그")
    check(MozcUpdater.offer(from: release("mozc-1-20261120-c2c2c2c", asset: "other.zip")) == nil, "묶음 이름이 다르다")
    check(MozcUpdater.offer(from: release("mozc-1-20261120-c2c2c2c", digest: nil)) == nil, "해시가 없으면 받지 않는다")
    check(MozcUpdater.offer(from: release("mozc-1-20261120-c2c2c2c", draft: true)) == nil, "초안")
}

do {
    let releases = [
        release("mozc-1-20261010-b1b1b1b"), release("mozc-1-20261120-c2c2c2c"),
        release("mozc-2-20261224-d3d3d3d"), release("mozc-1-20261201-e4e4e4e"),
    ]
    check(MozcUpdater.newest(in: releases, abi: 1, bad: [])?.commitPrefix == "e4e4e4e", "이 판의 가장 새 엔진")
    check(MozcUpdater.newest(in: releases, abi: 1, bad: ["e4e4e4e9"])?.commitPrefix == "c2c2c2c", "나쁜 엔진은 건너뛴다")
    let offer = MozcUpdater.offer(from: release("mozc-1-20261120-c2c2c2c"))!
    check(MozcUpdater.isWanted(offer, installed: [fakeBundled]), "앱에 든 것보다 새것은 받는다")
    let later = MozcComponent(source: .downloaded, libraryURL: fakeBundled.libraryURL, dataURL: fakeBundled.dataURL,
                              commit: "f0f0f0f0", date: "2026-11-21", version: "3.35.1.101", directory: nil)
    check(!MozcUpdater.isWanted(offer, installed: [fakeBundled, later]), "여기 있는 것보다 오래된 것은 받지 않는다")
    let same = MozcComponent(source: .downloaded, libraryURL: later.libraryURL, dataURL: later.dataURL,
                             commit: "c2c2c2c8", date: "2026-11-20", version: "3.35.1.101", directory: nil)
    check(!MozcUpdater.isWanted(offer, installed: [same]), "이미 받은 것은 받지 않는다")
}

// MARK: - 받은 묶음 설치

func sha256(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }

/// package-component.sh처럼 cssgsg-mozc.zip을 만든다(가짜 파일).
func makeZip(commit: String, date: String, abi: Int = 1, tamperLibraryHash: Bool = false) -> (zip: URL, digest: String) {
    let stage = work.appendingPathComponent(".stage-\(UUID().uuidString)")
    try! fm.createDirectory(at: stage, withIntermediateDirectories: true)
    let library = Data((0..<4096).map { UInt8($0 % 251) })
    let data = Data((0..<8192).map { UInt8($0 % 241) })
    try! library.write(to: stage.appendingPathComponent(MozcComponents.libraryName))
    try! data.write(to: stage.appendingPathComponent(MozcComponents.dataName))
    let manifest = MozcComponents.Manifest(
        abi: abi, commit: commit, date: date, version: "3.35.1.101",
        files: [MozcComponents.libraryName: tamperLibraryHash ? String(repeating: "0", count: 64) : sha256(library),
                MozcComponents.dataName: sha256(data)])
    try! JSONEncoder().encode(manifest).write(to: stage.appendingPathComponent(MozcComponents.manifestName))
    let zip = work.appendingPathComponent("\(UUID().uuidString).zip")
    let ditto = Process()
    ditto.executableURL = URL(fileURLWithPath: "/usr/bin/ditto")
    ditto.arguments = ["-c", "-k", "--norsrc", "--noextattr", stage.path, zip.path]
    try! ditto.run()
    ditto.waitUntilExit()
    try! fm.removeItem(at: stage)
    return (zip, "sha256:" + sha256(try! Data(contentsOf: zip)))
}

func offer(commit: String, date: String, digest: String, abi: Int = 1) -> MozcUpdater.Offer {
    MozcUpdater.Offer(abi: abi, date: date, commitPrefix: String(commit.prefix(7)),
                      downloadURL: URL(string: "https://example.invalid/cssgsg-mozc.zip")!, digest: digest)
}

do {
    let downloads = fresh()
    let commit = "c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2"
    let (zip, digest) = makeZip(commit: commit, date: "2026-11-20")
    let component = try? MozcUpdater.install(zip: zip, offer: offer(commit: commit, date: "2026-11-20", digest: digest),
                                             abi: abi, into: downloads)
    check(component?.commit == commit && MozcComponents.downloaded().map(\.commit) == [commit], "확인한 묶음을 설치한다")
    check(MozcComponents.candidates().first?.commit == commit, "…다음 시작 때 그것부터 읽는다")
}

do {
    let downloads = fresh()
    let commit = "c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2"
    let (zip, _) = makeZip(commit: commit, date: "2026-11-20")
    let wrong = "sha256:" + String(repeating: "a", count: 64)
    let installed = try? MozcUpdater.install(zip: zip, offer: offer(commit: commit, date: "2026-11-20", digest: wrong),
                                             abi: abi, into: downloads)
    check(installed == nil && MozcComponents.downloaded().isEmpty, "GitHub 해시와 다르면 설치하지 않는다")
}

do {
    let downloads = fresh()
    let commit = "c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2"
    let (zip, digest) = makeZip(commit: commit, date: "2026-11-20", tamperLibraryHash: true)
    let installed = try? MozcUpdater.install(zip: zip, offer: offer(commit: commit, date: "2026-11-20", digest: digest),
                                             abi: abi, into: downloads)
    check(installed == nil && MozcComponents.downloaded().isEmpty, "manifest의 파일 해시와 다르면 설치하지 않는다")
}

do {
    let downloads = fresh()
    let (zip, digest) = makeZip(commit: "d3d3d3d3d3d3d3d3d3d3", date: "2026-11-20")
    let installed = try? MozcUpdater.install(zip: zip, offer: offer(commit: "c2c2c2c2c2c2c2c2c2c2", date: "2026-11-20",
                                                                    digest: digest), abi: abi, into: downloads)
    check(installed == nil, "다른 빌드의 manifest면 설치하지 않는다")
    let (zip2, digest2) = makeZip(commit: "c2c2c2c2c2c2c2c2c2c2", date: "2026-11-20", abi: 2)
    let other = try? MozcUpdater.install(zip: zip2, offer: offer(commit: "c2c2c2c2c2c2c2c2c2c2", date: "2026-11-20",
                                                                  digest: digest2, abi: 2), abi: abi, into: downloads)
    check(other == nil, "모르는 C API 판이면 설치하지 않는다")
}

// MARK: - 상태

do {
    let old = MozcStatus.Build(version: "3.34.6239.101", date: "2026-09-28", commit: "a069a88")
    let new = MozcStatus.Build(version: "3.35.1.101", date: "2026-11-20", commit: "c2c2c2c")
    var status = MozcStatus()
    status.pending = new
    let bundled = MozcComponent(source: .bundled, libraryURL: fakeBundled.libraryURL, dataURL: fakeBundled.dataURL,
                                commit: old.commit, date: old.date, version: old.version, directory: nil)
    let afterOld = status.started(with: bundled)
    check(afterOld.active == old && afterOld.activeSource == "bundled" && afterOld.pending == new,
          "상태: 앱에 든 엔진으로 떴으면 받아 둔 새것은 계속 기다린다")
    let downloaded = MozcComponent(source: .downloaded, libraryURL: fakeBundled.libraryURL, dataURL: fakeBundled.dataURL,
                                   commit: new.commit, date: new.date, version: new.version, directory: nil)
    let afterNew = status.started(with: downloaded)
    check(afterNew.active == new && afterNew.activeSource == "downloaded" && afterNew.pending == nil,
          "상태: 받아 둔 새것으로 떴으면 기다림 표시를 지운다")
    check(status.started(with: nil).active == nil, "상태: 엔진이 없으면 쓰는 엔진도 없다")
}

// MARK: - 엔진 워크플로가 내는 묶음: 받아서, 설치하고, 앱에 든 것 대신 읽는다

do {
    let zip = repository.appendingPathComponent("build/mozc-component/cssgsg-mozc.zip")
    if !fm.fileExists(atPath: zip.path) {
        print("skip 실제 묶음 없음(bash tools/mozc/package-component.sh를 먼저)")
    } else {
        let unzip = Process()
        unzip.executableURL = URL(fileURLWithPath: "/usr/bin/unzip")
        unzip.arguments = ["-p", zip.path, MozcComponents.manifestName]
        let pipe = Pipe()
        unzip.standardOutput = pipe
        try! unzip.run()
        let manifestData = pipe.fileHandleForReading.readDataToEndOfFile()
        unzip.waitUntilExit()
        let manifest = try! JSONDecoder().decode(MozcComponents.Manifest.self, from: manifestData)
        // 앱에 든 엔진은 더 오래된 것으로 친다. 그러면 받은 엔진을 고른다.
        let downloads = fresh(bundled: MozcComponent(
            source: .bundled, libraryURL: realBundled.libraryURL, dataURL: realBundled.dataURL,
            commit: "0000000000", date: "2000-01-01", version: realBundled.version, directory: nil))
        let digest = "sha256:" + sha256(try! Data(contentsOf: zip))
        let installed = try? MozcUpdater.install(
            zip: zip, offer: offer(commit: manifest.commit, date: manifest.date, digest: digest, abi: manifest.abi),
            abi: abi, into: downloads)
        check(installed != nil, "실제 묶음 설치: \(manifest.version) (\(manifest.date), \(manifest.commit.prefix(7)))")
        let engine = CoreEngine(configTOML: nil)
        let active = MozcLoader.start(engine, profile: profile)
        check(active?.source == .downloaded
              && active?.libraryURL.standardizedFileURL == installed?.libraryURL.standardizedFileURL,
              "…앱에 든 것이 아니라 받은 엔진을 읽는다(읽을 때 にほんご를 변환해 본다)")
        if let installed {
            let loading = installed.directory!.appendingPathComponent("loading").path
            check(!fm.fileExists(atPath: loading), "…다 읽었으니 읽는 중 표시는 없다")
        }
        MozcLoader.finish()
    }
}

print(failures == 0 ? "모두 통과" : "실패 \(failures)개")
exit(failures == 0 ? 0 : 1)
