import AppKit
import SwiftUI

/// cssgsg 설정 앱. 입력기 옆(/Library/Input Methods/cssgsgSettings.app)에 설치되고, 메뉴 막대 cssgsg 메뉴의 "설정…"이 연다.
/// 설정은 config.toml에 쓰고 입력기에 알린다(SettingsModel). 입력기 프로세스 안에 창을 두지 않는 것은
/// 입력기가 자기 창의 입력을 받는 꼴이 되지 않게 하려는 것이다(NRIME도 설정 앱을 따로 둔다).
@main
struct SettingsApp: App {
    @NSApplicationDelegateAdaptor(SettingsAppDelegate.self) private var appDelegate
    @StateObject private var model = SettingsModel()
    @StateObject private var dictionary = UserDictionaryModel()
    @StateObject private var updater = Updater()

    var body: some Scene {
        Window("cssgsg", id: "settings") {
            SettingsView(model: model, dictionary: dictionary, updater: updater)
                .frame(minWidth: 600, idealWidth: 740, minHeight: 520, idealHeight: 700)
        }
        .defaultSize(width: 740, height: 700)
        .windowResizability(.contentMinSize)
    }
}

final class SettingsAppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        // 배열 학습 페이지를 미리 읽어 둔다(탭을 누르면 바로 보이게).
        _ = LearnPage.shared
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

enum SettingsTab: String, CaseIterable {
    case general, korean, japanese, learn, about

    /// 처음 띄울 때의 `--tab 이름` 인자(업데이트 뒤에는 about).
    static var fromArguments: SettingsTab {
        let args = CommandLine.arguments
        guard let i = args.firstIndex(of: "--tab"), i + 1 < args.count, let tab = SettingsTab(rawValue: args[i + 1]) else {
            return .general
        }
        return tab
    }
}

struct SettingsView: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var dictionary: UserDictionaryModel
    @ObservedObject var updater: Updater
    @State private var tab = SettingsTab.fromArguments
    /// 화면 언어. 바꾸면 탭들을 새로 그린다(.id). 고른 탭은 이 뷰의 상태라 그대로다.
    @AppStorage(UILanguage.preferenceKey) private var languageCode = UILanguage.stored().rawValue

    private var language: Binding<UILanguage> {
        Binding(
            get: { UILanguage(rawValue: languageCode) ?? .ko },
            set: { lang in
                UILanguage.active = lang
                languageCode = lang.rawValue
            })
    }

    var body: some View {
        let _ = (UILanguage.active = UILanguage(rawValue: languageCode) ?? .ko)
        VStack(spacing: 0) {
            if let problem = model.fileProblem {
                Banner(text: tr("""
                    설정 파일에 오류가 있어 입력기가 기본 설정으로 동작하고 있습니다: \(problem)
                    여기서 설정을 바꾸면 잘못된 파일은 config.toml.bak으로 옮기고 새로 만듭니다.
                    """, """
                    The settings file has an error, so the input method is running with default settings: \(problem)
                    Changing a setting here moves the broken file to config.toml.bak and creates a new one.
                    """, """
                    設定ファイルにエラーがあるため、入力メソッドは既定の設定で動作しています: \(problem)
                    ここで設定を変更すると、壊れたファイルを config.toml.bak に移して新しく作成します。
                    """))
            }
            if let problem = model.writeProblem {
                Banner(text: tr("설정을 저장하지 못했습니다: \(problem)", "Couldn’t save the settings: \(problem)",
                                "設定を保存できませんでした: \(problem)"))
            }
            TabView(selection: $tab) {
                GeneralTab(model: model)
                    .tabItem { Label(tr("일반", "General", "一般"), systemImage: "keyboard") }
                    .tag(SettingsTab.general)
                KoreanTab(model: model)
                    .tabItem { Label(tr("한국어", "Korean", "韓国語"), systemImage: "character.book.closed") }
                    .tag(SettingsTab.korean)
                JapaneseTab(model: model, dictionary: dictionary)
                    .tabItem { Label(tr("일본어", "Japanese", "日本語"), systemImage: "character.book.closed.ja") }
                    .tag(SettingsTab.japanese)
                LearnTab()
                    .tabItem { Label(tr("배열 학습", "Layouts", "配列の学習"), systemImage: "graduationcap") }
                    .tag(SettingsTab.learn)
                AboutTab(model: model, updater: updater, language: language)
                    .tabItem { Label(tr("정보", "About", "情報"), systemImage: "info.circle") }
                    .tag(SettingsTab.about)
            }
            .padding(.top, 8)
        }
        .id(languageCode)
        .navigationTitle(tr("cssgsg 설정", "cssgsg Settings", "cssgsg 設定"))
        // 이미 떠 있을 때 탭을 알려 온다(`--tab`은 처음 띄울 때만 먹는다).
        .onReceive(DistributedNotificationCenter.default().publisher(for: Cssgsg.showSettingsTab)) { note in
            if let name = note.object as? String, let target = SettingsTab(rawValue: name) { tab = target }
        }
        // 다른 곳(직접 편집)에서 바뀐 것을 창이 앞으로 올 때 다시 읽고, 입력기에도 다시 읽으라고 한다
        // (설정 파일을 직접 고친 뒤 설정 앱을 열면 적용된다. 파일이 틀렸으면 입력기는 지금 설정을 지킨다).
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            model.reload()
            model.refreshIMEStatus()
            dictionary.reloadIfChanged()
            Cssgsg.Notice.configChanged.post()
        }
    }
}
