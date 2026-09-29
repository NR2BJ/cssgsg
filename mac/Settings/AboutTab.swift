import SwiftUI

// MARK: - 정보

struct AboutTab: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var updater: Updater
    @Binding var language: UILanguage
    @State private var showNotices = false

    private var appVersion: String { Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "?" }

    /// 옆에 설치된 입력기의 버전.
    private var imeVersion: String {
        let url = Bundle.main.bundleURL.deletingLastPathComponent().appendingPathComponent("cssgsg.app")
        return Bundle(url: url)?.infoDictionary?["CFBundleShortVersionString"] as? String
            ?? tr("찾을 수 없음", "Not found", "見つかりません")
    }

    /// 시스템 설정에서 이 권한 목록의 이름. macOS 27은 "손쉬운 사용"을 "기기 제어 및 데이터 접근"으로 바꿨다(NRIME와 같다).
    private static var permissionName: String {
        ProcessInfo.processInfo.operatingSystemVersion.majorVersion >= 27
            ? tr("기기 제어 및 데이터 접근", "Device Control and Data Access", "デバイスの制御とデータへのアクセス")
            : tr("손쉬운 사용", "Accessibility", "アクセシビリティ")
    }

    var body: some View {
        Form {
            Section {
                LabeledContent(tr("입력기", "Input Method", "入力メソッド"), value: imeVersion)
                LabeledContent(tr("설정 앱", "Settings App", "設定アプリ"), value: appVersion)
                LabeledContent(tr("입력 소스", "Input Source", "入力ソース")) {
                    if model.inputSourceAdded {
                        Label(tr("추가됨", "Added", "追加済み"), systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                    } else {
                        HStack {
                            Text(tr("추가 안 됨", "Not added", "未追加")).foregroundStyle(.orange)
                            Button(tr("시스템 설정 열기", "Open System Settings", "システム設定を開く")) { Cssgsg.openKeyboardSettings() }
                        }
                    }
                }
                LabeledContent(Self.permissionName) {
                    switch model.postEventAllowed {
                    case true?:
                        Label(tr("허용됨", "Allowed", "許可済み"), systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                    case false?:
                        HStack {
                            Text(tr("꺼짐", "Off", "オフ")).foregroundStyle(.orange)
                            Button(tr("권한 요청…", "Request…", "許可を要求…")) { model.requestPostEventAccess() }
                        }
                    case nil where model.imeRunning:
                        Text(tr("아직 확인 안 됨", "Not checked yet", "未確認")).foregroundStyle(.secondary)
                    case nil:
                        Text(tr("알 수 없음 (입력기가 실행 중이 아님)", "Unknown (input method not running)", "不明（入力メソッドが起動していません）"))
                            .foregroundStyle(.secondary)
                    }
                }
                Link("GitHub: NR2BJ/cssgsg", destination: URL(string: "https://github.com/NR2BJ/cssgsg")!)
            } header: {
                Text("cssgsg")
            } footer: {
                Text(tr("""
                    입력 소스는 시스템 설정 → 키보드 → 입력 소스 편집… → + → 영어 → cssgsg에서 추가합니다.
                    조합을 확정한 뒤 ⌘+키나 Codex의 Shift+Enter를 앱에 다시 보내려면 권한이 필요합니다. "꺼짐"이면 \
                    시스템 설정 → 개인정보 보호 및 보안 → \(Self.permissionName)에서 cssgsg를 켜세요.
                    """, """
                    Add the input source in System Settings → Keyboard → Input Sources → Edit… → + → English → cssgsg.
                    Permission is needed to re-send ⌘+key or Codex Shift+Enter to the app after committing. If it shows Off, turn on \
                    cssgsg in System Settings → Privacy & Security → \(Self.permissionName).
                    """, """
                    入力ソースは システム設定 → キーボード → 入力ソースを編集… → + → 英語 → cssgsg で追加します。
                    確定後に ⌘+キーや Codex の Shift+Enter をアプリへ送り直すには許可が必要です。「オフ」の場合は、\
                    システム設定 → プライバシーとセキュリティ → \(Self.permissionName) で cssgsg をオンにしてください。
                    """))
            }

            UpdateSection(updater: updater)

            Section(tr("화면 언어", "Language", "表示言語")) {
                Picker(tr("화면 언어", "Language", "表示言語"), selection: $language) {
                    ForEach(UILanguage.allCases) { Text($0.name).tag($0) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
            }

            Section {
                LabeledContent(tr("설정 파일", "Settings File", "設定ファイル")) {
                    Button(tr("Finder에서 보기", "Show in Finder", "Finder で表示")) { model.revealConfigFile() }
                }
                Toggle(tr("개발자 기록", "Developer Log", "開発者ログ"),
                       isOn: Binding(get: { model.developerMode }, set: { model.setDeveloperMode($0) }))
                LabeledContent(tr("기록 파일", "Log File", "ログファイル")) {
                    Button(tr("Finder에서 보기", "Show in Finder", "Finder で表示")) {
                        NSWorkspace.shared.activateFileViewerSelecting([Cssgsg.developerLogURL])
                    }
                    .disabled(!FileManager.default.fileExists(atPath: Cssgsg.developerLogURL.path))
                }
            } header: {
                Text(tr("파일", "Files", "ファイル"))
            } footer: {
                Text(tr("설정 파일(config.toml)은 직접 고쳐도 됩니다. 개발자 기록에는 키 코드·수식키·시각만 남고, 입력한 글자는 남지 않습니다.",
                        "You can edit the settings file (config.toml) directly. The developer log records only key codes, modifiers, and times, never the text you type.",
                        "設定ファイル（config.toml）は直接編集してもかまいません。開発者ログにはキーコード・修飾キー・時刻だけが残り、入力した文字は残りません。"))
            }

            Section(tr("라이선스", "License", "ライセンス")) {
                LabeledContent("cssgsg", value: "MIT")
                Button(tr("오픈 소스 고지…", "Open Source Notices…", "オープンソースの告知…")) { showNotices = true }
            }
        }
        .formStyle(.grouped)
        .sheet(isPresented: $showNotices) { NoticesView() }
        .onAppear {
            model.refreshIMEStatus()
            updater.checkIfDue()
        }
    }
}

/// 업데이트: 채널, 상태, 지금 확인, 설치, 릴리스 노트.
struct UpdateSection: View {
    @ObservedObject var updater: Updater

    var body: some View {
        Section {
            Picker(tr("업데이트 채널", "Update Channel", "アップデートチャンネル"), selection: $updater.channel) {
                ForEach(UpdateChannel.allCases) { Text($0.name).tag($0) }
            }
            .pickerStyle(.segmented)
            .disabled(updater.state.isBusy)
            LabeledContent(tr("상태", "Status", "状態")) { status }
            if case let .available(release) = updater.state {
                notes(release)
            }
            HStack {
                Button(tr("지금 확인", "Check Now", "今すぐ確認")) { updater.check(userInitiated: true) }
                    .disabled(updater.state.isBusy)
                Spacer()
                if let last = updater.lastCheck {
                    Text(tr("마지막 확인: ", "Last checked: ", "最終確認: ") + last.formatted(date: .abbreviated, time: .shortened))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        } header: {
            Text(tr("업데이트", "Updates", "アップデート"))
        } footer: {
            Text(tr("""
                베타 채널은 정식 출시 전의 테스트 버전도 받습니다(불안정할 수 있습니다). 설치할 때 관리자 암호가 필요하고, \
                설치가 끝나면 입력기와 설정 앱이 다시 시작됩니다.
                """, """
                The beta channel also gets test builds before release (they may be unstable). Installing asks for an \
                administrator password; the input method and this app restart when it finishes.
                """, """
                ベータチャンネルは正式リリース前のテストビルドも受け取ります（不安定な場合があります）。インストールには管理者パスワードが\
                必要で、終わると入力メソッドと設定アプリが再起動します。
                """))
        }
    }

    @ViewBuilder private var status: some View {
        switch updater.state {
        case .idle:
            Text(tr("확인 전", "Not checked", "未確認")).foregroundStyle(.secondary)
        case .checking:
            HStack(spacing: 6) {
                ProgressView().controlSize(.small)
                Text(tr("확인 중…", "Checking…", "確認中…"))
            }
        case .upToDate:
            Label(tr("최신 버전입니다", "Up to date", "最新です"), systemImage: "checkmark.circle.fill").foregroundStyle(.green)
        case let .available(release):
            HStack {
                Text(tr("새 버전 \(release.version)", "Version \(release.version) available", "新しいバージョン \(release.version)"))
                    .fontWeight(.semibold)
                Button(tr("설치", "Install", "インストール")) { updater.install() }
            }
        case let .downloading(_, progress):
            HStack(spacing: 6) {
                ProgressView(value: progress).frame(width: 120)
                Text("\(Int(progress * 100))%").monospacedDigit()
            }
        case .installing:
            HStack(spacing: 6) {
                ProgressView().controlSize(.small)
                Text(tr("설치 중… (암호를 입력하세요)", "Installing… (enter your password)", "インストール中…（パスワードを入力してください）"))
            }
        case let .failed(reason):
            Label(reason, systemImage: "exclamationmark.triangle").foregroundStyle(.orange)
        }
    }

    @ViewBuilder private func notes(_ release: GitHubRelease) -> some View {
        if let body = release.body?.trimmingCharacters(in: .whitespacesAndNewlines), !body.isEmpty {
            ScrollView {
                Text(body)
                    .font(.callout)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(maxHeight: 160)
        }
        if let page = release.htmlURL.flatMap(URL.init(string:)) {
            Link(tr("릴리스 페이지", "Release Page", "リリースページ"), destination: page)
        }
    }
}

struct NoticesView: View {
    @Environment(\.dismiss) private var dismiss

    private var text: String {
        guard let url = Bundle.main.url(forResource: "THIRD_PARTY_NOTICES", withExtension: "txt") else {
            return tr("고지 파일이 없습니다.", "The notices file is missing.", "告知ファイルがありません。")
        }
        return (try? String(contentsOf: url, encoding: .utf8))
            ?? tr("고지 파일을 읽지 못했습니다.", "Couldn’t read the notices.", "告知ファイルを読めませんでした。")
    }

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                Text(text)
                    .font(.system(.caption, design: .monospaced))
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding()
            }
            Divider()
            HStack {
                Spacer()
                Button(tr("닫기", "Close", "閉じる")) { dismiss() }.keyboardShortcut(.defaultAction)
            }
            .padding(10)
        }
        .frame(width: 640, height: 520)
    }
}
