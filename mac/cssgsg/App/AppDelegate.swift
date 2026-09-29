import Cocoa
import InputMethodKit

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private(set) var server: IMKServer?
    let candidatePanel = CandidatePanel()
    private var statusItem: NSStatusItem?
    private var permissionItem: NSMenuItem?
    private var addSourceItem: NSMenuItem?
    private var updateItem: NSMenuItem?
    private var releaseNotesItem: NSMenuItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let connection = Bundle.main.infoDictionary?["InputMethodConnectionName"] as? String
            ?? (Bundle.main.bundleIdentifier ?? "com.cssgsg.inputmethod.app") + "_Connection"
        server = IMKServer(name: connection, bundleIdentifier: Bundle.main.bundleIdentifier)
        let engine = CoreEngine.shared
        KeyEventReposter.replayDelay = TimeInterval(engine.macSettings.newline_replay_ms) / 1000
        startMozc(engine)
        HanjaLearningStore.shared.load(into: engine)
        setupStatusItem()
        updateStatus(engine.mode)
        observeSettingsNotices()
        InputSourceSetup.register()
        InputSourceSetup.promptOnceIfNotAdded()
        requestPermissionsIfNeeded()
        Updater.shared.onChange = { [weak self] in self?.refreshUpdateItems() }
        Updater.shared.start()
        refreshUpdateItems()
        DeveloperLogger.shared.log("App", "started", metadata: [
            "connection": connection,
            "version": Updater.currentVersion,
            "configError": engine.configError ?? "none",
        ])
    }

    /// 끝낼 때(메뉴의 다시 시작 포함) 한자 학습을 바로 저장한다(평소에는 2초씩 모아 저장).
    func applicationWillTerminate(_ notification: Notification) {
        HanjaLearningStore.shared.saveNow()
    }

    /// Mozc(일본어 한자 변환)를 켠다. 준비는 10~20ms라 시작할 때 바로 한다.
    /// 학습 기록은 cssgsg 폴더 안에 따로 둔다(NRIME의 ~/Library/Application Support/Mozc와 섞이지 않게).
    private func startMozc(_ engine: CoreEngine) {
        guard let data = Bundle.main.path(forResource: "mozc", ofType: "data") else {
            DeveloperLogger.shared.log("Mozc", "no data in bundle")
            return
        }
        let profile = Cssgsg.mozcProfileURL
        try? FileManager.default.createDirectory(at: profile, withIntermediateDirectories: true)
        let started = Date()
        let ok = engine.useMozc(dataPath: data, profileDir: profile.path)
        DeveloperLogger.shared.log("Mozc", ok ? "ready" : "failed", metadata: [
            "ms": String(format: "%.0f", Date().timeIntervalSince(started) * 1000),
            "error": ok ? "-" : String(cString: cssgsg_last_error()),
        ])
    }

    // MARK: - 메뉴 막대

    private func setupStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        let menu = NSMenu()
        menu.delegate = self
        let version = NSMenuItem(title: "cssgsg \(Updater.currentVersion)", action: nil, keyEquivalent: "")
        version.isEnabled = false
        menu.addItem(version)
        let update = NSMenuItem(title: "업데이트 확인", action: #selector(updateAction), keyEquivalent: "")
        update.target = self
        menu.addItem(update)
        updateItem = update
        let notes = NSMenuItem(title: "    바뀐 점 보기", action: #selector(openReleaseNotes), keyEquivalent: "")
        notes.target = self
        notes.isHidden = true
        menu.addItem(notes)
        releaseNotesItem = notes
        menu.addItem(.separator())
        let addSource = NSMenuItem(title: "⚠︎ 입력 소스에 추가하기…", action: #selector(openKeyboardSettings), keyEquivalent: "")
        addSource.target = self
        addSource.toolTip = "시스템 설정 → 키보드 → 입력 소스 편집 → + → 영어 → cssgsg. macOS는 입력기가 스스로 추가되지 못하게 한다."
        menu.addItem(addSource)
        addSourceItem = addSource
        let permission = NSMenuItem(title: "⚠︎ 키 보내기 권한 허용…", action: #selector(openPermissionSettings), keyEquivalent: "")
        permission.target = self
        permission.toolTip = "조합 중 ⌘/Option+키와 Codex 줄바꿈에 필요하다. 없어도 입력은 된다."
        menu.addItem(permission)
        permissionItem = permission
        menu.addItem(withTitle: "설정…", action: #selector(openSettings), keyEquivalent: ",").target = self
        menu.addItem(withTitle: "배열 학습…", action: #selector(openLearn), keyEquivalent: "").target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "cssgsg 다시 시작", action: #selector(restart), keyEquivalent: "").target = self
        item.menu = menu
        statusItem = item
    }

    /// 메뉴를 열 때마다 권한과 입력 소스 추가 여부를 다시 본다. 됐으면 항목을 숨긴다.
    func menuNeedsUpdate(_ menu: NSMenu) {
        permissionItem?.isHidden = KeyEventReposter.canPostEvents
        addSourceItem?.isHidden = InputSourceSetup.isAdded
    }

    @objc private func openKeyboardSettings() {
        InputSourceSetup.openKeyboardSettings()
    }

    // MARK: - 업데이트

    /// 업데이트 상태를 메뉴 항목 제목으로 보인다(모달 창은 입력기 메인 스레드를 막으므로 쓰지 않는다).
    private func refreshUpdateItems() {
        guard let item = updateItem else { return }
        var offering = false
        item.isEnabled = true
        switch Updater.shared.state {
        case .idle:
            item.title = "업데이트 확인"
        case .checking:
            item.title = "업데이트 확인 중…"
            item.isEnabled = false
        case .upToDate:
            item.title = "최신 버전 사용 중 (다시 확인)"
        case let .available(release):
            item.title = "⬆︎ \(release.version) 업데이트 설치…"
            offering = true
        case let .downloading(release, progress):
            item.title = "\(release.version) 내려받는 중… \(Int(progress * 100))%"
            item.isEnabled = false
        case let .installing(release):
            item.title = "\(release.version) 설치 중…"
            item.isEnabled = false
        case let .failed(reason):
            item.title = "⚠︎ 업데이트 실패: \(reason) (다시 확인)"
        }
        releaseNotesItem?.isHidden = !offering
    }

    @objc private func updateAction() {
        if case .available = Updater.shared.state {
            Updater.shared.install()
        } else {
            Updater.shared.check(userInitiated: true)
        }
    }

    @objc private func openReleaseNotes() {
        guard case let .available(release) = Updater.shared.state,
              let link = release.htmlURL, let url = URL(string: link) else { return }
        NSWorkspace.shared.open(url)
    }

    func updateStatus(_ mode: InputMode) {
        guard let button = statusItem?.button else { return }
        button.image = Self.statusIcon(mode.label)
        button.title = ""
        button.toolTip = "cssgsg: \(["영어 (Graphite)", "한국어 (참신세벌식)", "일본어 (新月配列)"][Int(mode.rawValue)])"
    }

    /// 메뉴 막대 아이콘을 글자로 그린다(Retina 자동 대응, 템플릿 이미지).
    private static func statusIcon(_ text: String) -> NSImage {
        let image = NSImage(size: NSSize(width: 18, height: 18), flipped: false) { rect in
            let attrs: [NSAttributedString.Key: Any] = [
                .font: NSFont.systemFont(ofSize: 14, weight: .medium),
                .foregroundColor: NSColor.black,
            ]
            let str = NSAttributedString(string: text, attributes: attrs)
            let size = str.size()
            str.draw(at: NSPoint(x: (rect.width - size.width) / 2, y: (rect.height - size.height) / 2))
            return true
        }
        image.isTemplate = true
        return image
    }

    /// 설정 앱을 연다. 없으면(개발 중 따로 빌드한 입력기 등) 설정 파일을 Finder에서 보인다.
    @objc private func openSettings() {
        if !SettingsLauncher.open() {
            NSWorkspace.shared.activateFileViewerSelecting([Cssgsg.configURL])
        }
    }

    /// 설정 앱의 배열 학습 탭. 설정 앱이 없으면 학습 페이지를 브라우저로 연다.
    @objc private func openLearn() {
        if !SettingsLauncher.open(tab: "learn"),
           let url = Bundle.main.url(forResource: "index", withExtension: "html") {
            NSWorkspace.shared.open(url)
        }
    }

    @objc private func restart() {
        // 입력기 프로세스는 필요할 때 macOS가 다시 띄운다.
        NSApp.terminate(nil)
    }

    // MARK: - 설정 앱 알림

    /// 설정 앱이 보내는 Darwin 알림을 받는다. 콜백은 C 함수라 아무것도 붙잡지 않고, 처리는 메인 스레드에서 한다.
    private func observeSettingsNotices() {
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        let observer = UnsafeRawPointer(Unmanaged.passUnretained(self).toOpaque())
        for notice in [Cssgsg.Notice.configChanged, .hanjaLearningCleared, .restart] {
            CFNotificationCenterAddObserver(center, observer, { _, _, name, _, _ in
                guard let raw = name?.rawValue as String?, let notice = Cssgsg.Notice(rawValue: raw) else { return }
                DispatchQueue.main.async { AppDelegate.handle(notice) }
            }, notice.rawValue as CFString, nil, .deliverImmediately)
        }
    }

    private static func handle(_ notice: Cssgsg.Notice) {
        switch notice {
        case .configChanged:
            let engine = CoreEngine.shared
            let ok = engine.reloadConfig()
            KeyEventReposter.replayDelay = TimeInterval(engine.macSettings.newline_replay_ms) / 1000
            DeveloperLogger.shared.log("Settings", "config reloaded", metadata: [
                "ok": "\(ok)", "error": engine.configError ?? "none",
            ])
        case .hanjaLearningCleared:
            HanjaLearningStore.shared.clear()
            DeveloperLogger.shared.log("Settings", "hanja learning cleared")
        case .restart:
            DeveloperLogger.shared.log("Settings", "restart")
            NSApp.terminate(nil)
        }
    }

    private static let askedPostEventAccessKey = "askedPostEventAccess"

    /// ⌘/Option+키 재전송·Codex 줄바꿈(키 이벤트 보내기)에 필요하다. 시스템 창은 처음 실행 때 한 번만 띄운다.
    /// ad-hoc 서명이면 새 빌드마다 권한이 풀리는데, 그때마다 창을 띄우지 않고 메뉴에 허용 항목만 보인다.
    private func requestPermissionsIfNeeded() {
        let defaults = UserDefaults.standard
        guard !KeyEventReposter.canPostEvents, !defaults.bool(forKey: Self.askedPostEventAccessKey) else { return }
        defaults.set(true, forKey: Self.askedPostEventAccessKey)
        _ = CGRequestPostEventAccess()
    }

    /// 시스템 창을 다시 띄워 보고(이미 거절했거나 목록에 옛 빌드가 남아 있으면 안 뜬다), 손쉬운 사용 설정을 연다.
    /// 목록에 cssgsg가 켜져 있는데도 이 항목이 보이면 옛 빌드 기록이다. 빼고(-) 다시 켜면 된다.
    @objc private func openPermissionSettings() {
        _ = CGRequestPostEventAccess()
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
            NSWorkspace.shared.open(url)
        }
    }
}

extension NSApplication {
    var candidatePanel: CandidatePanel? {
        (delegate as? AppDelegate)?.candidatePanel
    }
}

/// 설정 앱(입력기 옆의 cssgsgSettings.app)을 연다. 배경 IMKit 앱에서 NSWorkspace로 띄우면 앞으로 오지 않을 때가 있어서
/// NRIME처럼 /usr/bin/open을 쓴다. 이미 떠 있으면 open의 인자는 무시되니 탭은 분산 알림으로도 알린다.
enum SettingsLauncher {
    static var appURL: URL {
        Bundle.main.bundleURL.deletingLastPathComponent().appendingPathComponent(Cssgsg.settingsAppName)
    }

    /// 열었으면 true. 설정 앱이 없으면 false.
    @discardableResult
    static func open(tab: String? = nil) -> Bool {
        let url = appURL
        guard FileManager.default.fileExists(atPath: url.path) else { return false }
        if let tab {
            DistributedNotificationCenter.default().postNotificationName(
                Cssgsg.showSettingsTab, object: tab, userInfo: nil, deliverImmediately: true)
        }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        process.arguments = ["-a", url.path] + (tab.map { ["--args", "--tab", $0] } ?? [])
        do {
            try process.run()
            return true
        } catch {
            DeveloperLogger.shared.log("Settings", "open failed", metadata: ["error": "\(error)"])
            return false
        }
    }
}
