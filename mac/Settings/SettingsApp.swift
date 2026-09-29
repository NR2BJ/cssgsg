import AppKit
import SwiftUI

/// cssgsg 설정 앱. 입력기 옆(/Library/Input Methods/cssgsgSettings.app)에 설치되고, 메뉴 막대 cssgsg 메뉴의 "설정…"이 연다.
/// 설정은 config.toml에 쓰고 입력기에 알린다(SettingsModel). 입력기 프로세스 안에 창을 두지 않는 것은
/// 입력기가 자기 창의 입력을 받는 꼴이 되지 않게 하려는 것이다(NRIME도 설정 앱을 따로 둔다).
@main
struct SettingsApp: App {
    @NSApplicationDelegateAdaptor(SettingsAppDelegate.self) private var appDelegate
    @StateObject private var model = SettingsModel()

    var body: some Scene {
        Window("cssgsg 설정", id: "settings") {
            SettingsView(model: model)
                .frame(minWidth: 560, idealWidth: 720, minHeight: 480, idealHeight: 640)
        }
        .defaultSize(width: 720, height: 640)
        .windowResizability(.contentMinSize)
    }
}

final class SettingsAppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

enum SettingsTab: String, CaseIterable {
    case general, korean, japanese, learn, about

    /// 처음 띄울 때의 `--tab 이름` 인자.
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
    @State private var tab = SettingsTab.fromArguments

    var body: some View {
        VStack(spacing: 0) {
            if let problem = model.fileProblem {
                Banner(text: "설정 파일에 오류가 있어 입력기는 기본 설정으로 돌고 있다: \(problem)\n여기서 설정을 바꾸면 틀린 파일은 config.toml.bak으로 옮기고 새로 쓴다.")
            }
            if let problem = model.writeProblem {
                Banner(text: "설정을 쓰지 못했다: \(problem)")
            }
            TabView(selection: $tab) {
                GeneralTab(model: model)
                    .tabItem { Label("일반", systemImage: "keyboard") }
                    .tag(SettingsTab.general)
                KoreanTab(model: model)
                    .tabItem { Label("한국어", systemImage: "character.book.closed") }
                    .tag(SettingsTab.korean)
                JapaneseTab(model: model)
                    .tabItem { Label("일본어", systemImage: "character.book.closed.ja") }
                    .tag(SettingsTab.japanese)
                LearnTab()
                    .tabItem { Label("배열 학습", systemImage: "graduationcap") }
                    .tag(SettingsTab.learn)
                AboutTab(model: model)
                    .tabItem { Label("정보", systemImage: "info.circle") }
                    .tag(SettingsTab.about)
            }
            .padding(.top, 8)
        }
        // 이미 떠 있을 때 입력기 메뉴(배열 학습…)가 탭을 알린다.
        .onReceive(DistributedNotificationCenter.default().publisher(for: Cssgsg.showSettingsTab)) { note in
            if let name = note.object as? String, let target = SettingsTab(rawValue: name) { tab = target }
        }
        // 다른 곳(직접 편집)에서 바뀐 것을 창이 앞으로 올 때 다시 읽고, 입력기에도 다시 읽으라고 한다
        // (설정 파일을 직접 고친 뒤 설정 앱을 열면 적용된다. 파일이 틀렸으면 입력기는 지금 설정을 지킨다).
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            model.reload()
            Cssgsg.Notice.configChanged.post()
        }
    }
}

struct Banner: View {
    let text: String

    var body: some View {
        Label(text, systemImage: "exclamationmark.triangle.fill")
            .font(.callout)
            .foregroundStyle(.primary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(10)
            .background(Color.orange.opacity(0.18))
            .textSelection(.enabled)
    }
}
