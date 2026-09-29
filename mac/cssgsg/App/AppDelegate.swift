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
        setupStatusItem()
        updateStatus(engine.mode)
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
        menu.addItem(withTitle: "배열 학습 열기", action: #selector(openLearn), keyEquivalent: "").target = self
        menu.addItem(withTitle: "설정 파일 열기", action: #selector(openConfig), keyEquivalent: "").target = self
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

    @objc private func openLearn() {
        guard let url = Bundle.main.url(forResource: "index", withExtension: "html") else { return }
        NSWorkspace.shared.open(url)
    }

    /// 설정 앱이 생기기 전까지는 config.toml을 직접 고친다. 없으면 설명이 달린 틀을 만들어 준다.
    @objc private func openConfig() {
        let url = CoreEngine.configURL
        let fm = FileManager.default
        try? fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        if !fm.fileExists(atPath: url.path) {
            try? Self.configTemplate.write(to: url, atomically: true, encoding: .utf8)
        }
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }

    @objc private func restart() {
        // 입력기 프로세스는 필요할 때 macOS가 다시 띄운다. 설정 파일을 다시 읽을 때 쓴다.
        NSApp.terminate(nil)
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

    private static let configTemplate = """
    # cssgsg 설정. 고친 뒤 메뉴 막대의 cssgsg 메뉴에서 "cssgsg 다시 시작"을 누르면 적용된다.
    # 줄 앞의 #을 지우면 그 설정이 켜진다. 적지 않은 값은 기본값이다.

    # 한국어 배열: "chamshin-v18"(기본형) 또는 "chamshin-d-v19"(D)
    # ko_layout = "chamshin-v18"

    # 수식키 탭 인식 시간(밀리초)
    # tap_threshold_ms = 200

    # 수식키별 탭 동작: "toggle_english", "toggle_non_english", "none"
    # 이 표를 적으면 기본값(오른쪽 Shift = 영어 토글, 왼쪽 Shift = 한↔일)을 대신한다.
    # [taps]
    # shift_right = "toggle_english"
    # shift_left = "toggle_non_english"

    # [ja]
    # 구두점: "japanese"(、。), "full_width_western"(，．), "half_width_western"(,.)
    # punctuation = "japanese"
    # / 자리를 ・로
    # slash_nakaguro = true
    # 읽기가 없을 때 Space를 전각 스페이스로
    # full_width_space = false
    # 일본어 모드에서 켠 Caps Lock(가타카나)을 다른 모드로 나갈 때 끈다
    # caps_katakana_auto_off = true
    # Caps Lock 가타카나는 치는 대로 바로 확정한다(마지막 글자만 잠깐 조합). false면 히라가나처럼 조합으로 들고 있다
    # katakana_direct = true

    # [mac]
    # 모드를 바꿀 때 커서 근처에 A/한/あ를 잠깐 보인다
    # hud = true
    # HUD 자리: "caret"(커서 위, 커서 자리를 모르면 안 보임) 또는 "mouse"(마우스 옆)
    # hud_position = "caret"
    # Codex처럼 줄바꿈 입력을 전송으로 받는 앱에서, 조합 중 Shift+Enter로 확정한 뒤 줄을 바꾸기까지 기다리는 시간(밀리초, 20~1000).
    # 짧으면 빨라지지만 줄바꿈이 먹힐 수 있다
    # newline_replay_ms = 120

    """
}

extension NSApplication {
    var candidatePanel: CandidatePanel? {
        (delegate as? AppDelegate)?.candidatePanel
    }
}
