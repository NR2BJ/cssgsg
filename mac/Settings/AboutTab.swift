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

    var body: some View {
        Form {
            Section {
                LabeledContent(tr("입력기", "Input method", "入力メソッド"), value: imeVersion)
                LabeledContent(tr("설정 앱", "Settings app", "設定アプリ"), value: appVersion)
                LabeledContent(tr("입력 소스", "Input source", "入力ソース")) {
                    if model.inputSourceAdded {
                        Label(tr("추가됨", "Added", "追加済み"), systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                    } else {
                        HStack {
                            Text(tr("추가 안 됨", "Not added", "未追加")).foregroundStyle(.orange)
                            Button(tr("시스템 설정 열기", "Open System Settings", "システム設定を開く")) { Cssgsg.openKeyboardSettings() }
                        }
                    }
                }
                LabeledContent(tr("키 보내기 권한", "Key-sending permission", "キー送信の許可")) {
                    switch model.postEventAllowed {
                    case true?:
                        Label(tr("허용됨", "Allowed", "許可済み"), systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                    case false?:
                        HStack {
                            Text(tr("허용 안 됨", "Not allowed", "未許可")).foregroundStyle(.orange)
                            Button(tr("허용하기…", "Allow…", "許可…")) { model.requestPostEventAccess() }
                        }
                    case nil where model.imeRunning:
                        Text(tr("알 수 없음", "Unknown", "不明")).foregroundStyle(.secondary)
                    case nil:
                        Text(tr("알 수 없음 (입력기가 떠 있지 않다)", "Unknown (input method not running)", "不明（入力メソッドが起動していない）"))
                            .foregroundStyle(.secondary)
                    }
                }
                Link("GitHub: NR2BJ/cssgsg", destination: URL(string: "https://github.com/NR2BJ/cssgsg")!)
            } header: {
                Text("cssgsg")
            } footer: {
                Text(tr("""
                    입력 소스는 시스템 설정 → 키보드 → 입력 소스 편집 → + → 영어 → cssgsg로 추가한다.
                    키 보내기 권한(개인정보 보호 및 보안 → 손쉬운 사용의 cssgsg)은 조합 중 ⌘+키를 앱에 다시 보낼 때와 Codex류 앱의 줄바꿈에 쓴다.
                    """, """
                    Add the input source in System Settings → Keyboard → Input Sources → Edit → + → English → cssgsg.
                    Key-sending permission (Privacy & Security → Accessibility → cssgsg) is used to re-send ⌘+key while composing \
                    and for newlines in Codex-like apps.
                    """, """
                    入力ソースは システム設定 → キーボード → 入力ソースを編集 → + → 英語 → cssgsg で追加します。
                    キー送信の許可（プライバシーとセキュリティ → アクセシビリティ の cssgsg）は、変換中の ⌘+キーの送り直しと Codex 系アプリの改行に使います。
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
                LabeledContent(tr("설정 파일", "Settings file", "設定ファイル")) {
                    Button(tr("Finder에서 보기", "Show in Finder", "Finder で表示")) { model.revealConfigFile() }
                }
                Toggle(tr("개발자 기록", "Developer log", "開発者ログ"),
                       isOn: Binding(get: { model.developerMode }, set: { model.setDeveloperMode($0) }))
                LabeledContent(tr("기록 파일", "Log file", "ログファイル")) {
                    Button(tr("Finder에서 보기", "Show in Finder", "Finder で表示")) {
                        NSWorkspace.shared.activateFileViewerSelecting([Cssgsg.developerLogURL])
                    }
                    .disabled(!FileManager.default.fileExists(atPath: Cssgsg.developerLogURL.path))
                }
            } header: {
                Text(tr("파일", "Files", "ファイル"))
            } footer: {
                Text(tr("설정 파일(config.toml)을 직접 고쳐도 된다. 개발자 기록에는 키 코드·수식키·시각만 남고 글자 내용은 남지 않는다.",
                        "You can edit the settings file (config.toml) directly. The developer log records only key codes, modifiers and times, never the text.",
                        "設定ファイル（config.toml）を直接編集してもかまいません。開発者ログにはキーコード・修飾キー・時刻だけが残り、文字の内容は残りません。"))
            }

            Section(tr("라이선스", "License", "ライセンス")) {
                LabeledContent("cssgsg", value: "MIT")
                Button(tr("다른 소프트웨어·데이터 고지문…", "Third-party notices…", "サードパーティの告知…")) { showNotices = true }
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
            Picker(tr("채널", "Channel", "チャンネル"), selection: $updater.channel) {
                ForEach(UpdateChannel.allCases) { Text($0.name).tag($0) }
            }
            .pickerStyle(.segmented)
            .disabled(updater.state.isBusy)
            LabeledContent(tr("상태", "Status", "状態")) { status }
            if case let .available(release) = updater.state {
                notes(release)
            }
            HStack {
                Button(tr("지금 확인", "Check now", "今すぐ確認")) { updater.check(userInitiated: true) }
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
                베타 채널은 정식 릴리스 전의 시험판(prerelease)도 받는다. 설치에는 관리자 암호가 필요하고, 끝나면 입력기와 설정 앱이 다시 시작된다.
                """, """
                The beta channel also offers prereleases. Installing asks for an administrator password; the input method and \
                this app restart afterwards.
                """, """
                ベータチャンネルは正式リリース前の試験版（プレリリース）も受け取ります。インストールには管理者パスワードが必要で、\
                終わると入力メソッドと設定アプリが再起動します。
                """))
        }
    }

    @ViewBuilder private var status: some View {
        switch updater.state {
        case .idle:
            Text(tr("확인하지 않았다", "Not checked", "未確認")).foregroundStyle(.secondary)
        case .checking:
            HStack(spacing: 6) {
                ProgressView().controlSize(.small)
                Text(tr("확인 중…", "Checking…", "確認中…"))
            }
        case .upToDate:
            Label(tr("최신 버전이다", "Up to date", "最新です"), systemImage: "checkmark.circle.fill").foregroundStyle(.green)
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
                Text(tr("설치 중… (암호 창)", "Installing… (password prompt)", "インストール中…（パスワード）"))
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
            Link(tr("릴리스 페이지", "Release page", "リリースページ"), destination: page)
        }
    }
}

struct NoticesView: View {
    @Environment(\.dismiss) private var dismiss

    private var text: String {
        guard let url = Bundle.main.url(forResource: "THIRD_PARTY_NOTICES", withExtension: "txt") else {
            return tr("고지문 파일이 없다", "The notices file is missing", "告知ファイルがありません")
        }
        return (try? String(contentsOf: url, encoding: .utf8)) ?? tr("고지문을 읽지 못했다", "Could not read the notices", "告知を読めません")
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
