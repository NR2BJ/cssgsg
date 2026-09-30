import Cocoa
import InputMethodKit

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private(set) var server: IMKServer?
    let candidatePanel = CandidatePanel()
    private var statusItem: NSStatusItem?
    private var settingsItem: NSMenuItem?
    private var restartItem: NSMenuItem?
    private var quitItem: NSMenuItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let connection = Bundle.main.infoDictionary?["InputMethodConnectionName"] as? String
            ?? (Bundle.main.bundleIdentifier ?? "com.cssgsg.inputmethod.app") + "_Connection"
        server = IMKServer(name: connection, bundleIdentifier: Bundle.main.bundleIdentifier)
        let engine = CoreEngine.shared
        Self.applyMacSettings(engine.macSettings)
        let mozc = MozcLoader.start(engine)
        MozcStatus.update { $0 = $0.started(with: mozc) }
        HanjaLearningStore.shared.load(into: engine)
        setupStatusItem()
        updateStatus(engine.mode)
        observeSettingsNotices()
        InputSourceSetup.register()
        InputSourceSetup.promptOnceIfNotAdded()
        PermissionMonitor.start()
        MozcUpdater.shared.start()
        // 0.4.0까지 업데이트를 확인하며 남긴 값, 0.5.x의 권한 값(이제 permissionStatus와 설정 앱이 쓴다).
        for key in ["updateCachedRelease", "updateETag", "updateLastCheck", "postEventAllowed", "askedPostEventAccess"] {
            UserDefaults.standard.removeObject(forKey: key)
        }
        DeveloperLogger.shared.log("App", "started", metadata: [
            "connection": connection,
            "version": Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "?",
            "configError": engine.configError ?? "none",
        ])
    }

    /// 끝낼 때(메뉴의 다시 시작 포함) 한자 학습을 바로 저장한다(평소에는 2초씩 모아 저장).
    /// Mozc 학습은 Mozc가 파일에 바로 쓴다(mmap). 이 시작은 깨끗이 끝난 것으로 적는다(MozcComponents.endRun).
    func applicationWillTerminate(_ notification: Notification) {
        HanjaLearningStore.shared.saveNow()
        MozcLoader.finish()
    }

    // MARK: - 메뉴 막대

    /// 메뉴는 설정·다시 시작·종료 셋뿐이다. 버전·업데이트·입력 소스·권한·배열 학습은 설정 앱에 있다.
    private func setupStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        let menu = NSMenu()
        menu.delegate = self
        settingsItem = menu.addItem(withTitle: "", action: #selector(openSettings), keyEquivalent: ",")
        settingsItem?.target = self
        menu.addItem(.separator())
        restartItem = menu.addItem(withTitle: "", action: #selector(restart), keyEquivalent: "")
        restartItem?.target = self
        quitItem = menu.addItem(withTitle: "", action: #selector(quit), keyEquivalent: "")
        quitItem?.target = self
        item.menu = menu
        statusItem = item
        titleMenuItems()
    }

    /// 메뉴 글자는 설정 앱에서 고른 화면 언어를 따른다.
    private func titleMenuItems() {
        settingsItem?.title = tr("설정…", "Settings…", "設定…")
        restartItem?.title = tr("cssgsg 다시 시작", "Restart cssgsg", "cssgsg を再起動")
        quitItem?.title = tr("cssgsg 종료", "Quit cssgsg", "cssgsg を終了")
    }

    /// 메뉴를 열 때마다 화면 언어와 입력기 상태를 다시 본다.
    func menuNeedsUpdate(_ menu: NSMenu) {
        UILanguage.active = UILanguage.stored()
        titleMenuItems()
        PermissionMonitor.refreshIfStale()
    }

    func updateStatus(_ mode: InputMode) {
        guard let button = statusItem?.button else { return }
        button.image = Self.statusIcon(mode.label)
        button.title = ""
        let names = [
            tr("영어 (Graphite)", "English (Graphite)", "英語 (Graphite)"),
            tr("한국어 (참신세벌식)", "Korean (Chamshin Sebeolsik)", "韓国語 (チャムシン3ボル式)"),
            tr("일본어 (新月配列)", "Japanese (Shingetsu)", "日本語 (新月配列)"),
        ]
        button.toolTip = "cssgsg: \(names[Int(mode.rawValue)])"
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

    @objc private func restart() {
        Self.relaunch()
    }

    /// 끝내고 곧바로 다시 띄운다. 이 셸은 이름이 cssgsg가 아니고 사용자 세션에서 돈다(업데이트 뒤 다시 띄우기와 같다).
    private static func relaunch() {
        let shell = Process()
        shell.executableURL = URL(fileURLWithPath: "/bin/sh")
        let path = Bundle.main.bundlePath.replacingOccurrences(of: "'", with: "'\\''")
        shell.arguments = ["-c", "/bin/sleep 0.5; /usr/bin/open -g '\(path)'"]
        try? shell.run()
        NSApp.terminate(nil)
    }

    /// 끝낸다. cssgsg가 입력 소스로 골라져 있으면 다음 키 입력 때 macOS가 다시 띄운다.
    @objc private func quit() {
        NSApp.terminate(nil)
    }

    // MARK: - 설정 앱 알림

    /// 설정 앱이 보내는 Darwin 알림을 받는다. 콜백은 C 함수라 아무것도 붙잡지 않고, 처리는 메인 스레드에서 한다.
    private func observeSettingsNotices() {
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        let observer = UnsafeRawPointer(Unmanaged.passUnretained(self).toOpaque())
        let notices: [Cssgsg.Notice] = [
            .configChanged, .hanjaLearningCleared, .restart, .statusRequest, .requestPostEventAccess, .userDictionaryChanged,
            .mozcCheckForUpdate,
        ]
        for notice in notices {
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
            applyMacSettings(engine.macSettings)
            DeveloperLogger.shared.log("Settings", "config reloaded", metadata: [
                "ok": "\(ok)", "error": engine.configError ?? "none",
            ])
        case .hanjaLearningCleared:
            HanjaLearningStore.shared.clear()
            DeveloperLogger.shared.log("Settings", "hanja learning cleared")
        case .restart:
            DeveloperLogger.shared.log("Settings", "restart")
            relaunch()
        case .statusRequest:
            PermissionMonitor.refresh(force: true)
        case .requestPostEventAccess:
            PermissionMonitor.requestMissing()
        case .userDictionaryChanged:
            let ok = CoreEngine.shared.reloadUserDictionary()
            DeveloperLogger.shared.log("Settings", "user dictionary reloaded", metadata: ["ok": "\(ok)"])
        case .mozcCheckForUpdate:
            MozcUpdater.shared.checkNow()
        }
    }

    /// 설정 파일의 [mac] 표 중 셸이 쓰는 값.
    private static func applyMacSettings(_ settings: CssgsgMacSettings) {
        CandidatePanel.fontSize = CGFloat(settings.candidate_font_size)
        KeyEventReposter.insertWait = TimeInterval(settings.newline_insert_wait_ms) / 1000
        KeyEventReposter.keyPressWait = TimeInterval(settings.newline_key_press_wait_ms) / 1000
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
