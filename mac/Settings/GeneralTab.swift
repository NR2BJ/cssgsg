import SwiftUI

// MARK: - 일반

// 화면 문구는 NRIME 설정 앱처럼 쓴다: 이름은 명사형("탭 인식 시간"), 설명은 합니다체(2026-09-30 사용자 요청).
struct GeneralTab: View {
    @ObservedObject var model: SettingsModel

    private var shortcuts: CssgsgConfig.Shortcuts { model.config.shortcuts }
    private var defaults: CssgsgConfig.Shortcuts { SettingsModel.defaults.shortcuts }

    var body: some View {
        Form {
            Section {
                ShortcutRow(title: ShortcutText.actionName(\.toggleEnglish), value: shortcuts.toggleEnglish,
                            defaultValue: defaults.toggleEnglish) { model.setShortcut(\.toggleEnglish, to: $0) }
                ShortcutRow(title: ShortcutText.actionName(\.toggleNonEnglish), value: shortcuts.toggleNonEnglish,
                            defaultValue: defaults.toggleNonEnglish) { model.setShortcut(\.toggleNonEnglish, to: $0) }
                ShortcutRow(title: ShortcutText.actionName(\.hanja), value: shortcuts.hanja,
                            defaultValue: defaults.hanja) { model.setShortcut(\.hanja, to: $0) }
                if shortcuts.toggleEnglish.isEmpty {
                    Label(tr("영어로 전환하는 단축키가 없습니다. 메뉴 막대에서는 모드를 바꿀 수 없으니 하나는 정해 두세요.",
                             "No shortcut switches to English. The menu bar can’t change modes, so please set one.",
                             "英語に切り替えるショートカットがありません。メニューバーからはモードを変えられないので、一つは設定してください。"),
                          systemImage: "exclamationmark.triangle")
                        .foregroundStyle(.orange)
                }
            } header: {
                Text(tr("단축키", "Shortcuts", "ショートカット"))
            } footer: {
                Text(tr("""
                    단추를 누른 뒤 원하는 키를 누르세요. 수식키만 짧게 눌렀다 떼면 탭으로, 수식키와 다른 키를 함께 누르면 조합으로 \
                    기록됩니다. 수식키는 왼쪽과 오른쪽을 구분합니다(왼쪽 Option + Return과 오른쪽 Option + Return은 다른 단축키입니다). \
                    ⌘ 조합은 쓸 수 없고, Esc를 누르면 취소됩니다.
                    영어 ↔ 비영어는 영어와 직전에 쓴 언어(한국어·일본어) 사이를 오갑니다. 한국어 ↔ 일본어를 영어 모드에서 누르면 \
                    둘 중 직전에 쓰지 않은 쪽으로 전환합니다.
                    """, """
                    Click a button, then press the keys you want. A single modifier pressed briefly is recorded as a tap; a modifier \
                    pressed with another key is recorded as a combination. Left and right modifiers are distinct (Left Option + Return \
                    and Right Option + Return are different shortcuts). ⌘ combinations aren’t allowed. Press Esc to cancel.
                    English ↔ Non-English switches between English and the language you used last (Korean or Japanese). \
                    Korean ↔ Japanese, pressed in English mode, switches to whichever of the two you didn’t use last.
                    """, """
                    ボタンを押してから、使いたいキーを押してください。修飾キーだけを短く押して離すとタップ、修飾キーと別のキーを一緒に押すと\
                    組み合わせとして記録されます。修飾キーは左右を区別します（左Option + Return と右Option + Return は別のショートカットです）。\
                    ⌘ の組み合わせは使えません。Esc で取り消します。
                    英語 ↔ 英語以外は、英語と直前に使った言語（韓国語・日本語）を行き来します。韓国語 ↔ 日本語を英語モードで押すと、\
                    二つのうち直前に使っていない方に切り替わります。
                    """))
            }

            Section {
                LabeledContent(tr("탭 인식 시간", "Tap Recognition Time", "タップ判定時間")) {
                    ValueSlider(value: model.config.tapThresholdMs, range: 100...500, step: 10) { ms in
                        model.update { $0.tapThresholdMs = ms }
                    }
                }
            } header: {
                Text(tr("수식키 탭", "Modifier Taps", "修飾キーのタップ"))
            } footer: {
                Text(tr("수식키를 이 시간보다 오래 누르고 있으면 탭으로 보지 않습니다.",
                        "Holding a modifier longer than this doesn’t count as a tap.",
                        "修飾キーをこの時間より長く押していると、タップとみなしません。"))
            }

            Section(tr("후보창", "Candidate Window", "候補ウィンドウ")) {
                LabeledContent(tr("글자 크기", "Font Size", "文字サイズ")) {
                    ValueSlider(value: model.config.mac.candidateFontSize, range: 10...28, step: 1, unit: "pt") { size in
                        model.update { $0.mac.candidateFontSize = size }
                    }
                }
            }

            Section(tr("모드 표시", "Mode Indicator", "モード表示")) {
                Toggle(tr("모드 전환 시 G · ㅊ · 月 표시", "Show G · ㅊ · 月 When Switching Modes", "モード切り替え時に G · ㅊ · 月 を表示"),
                       isOn: model.binding(\.mac.hud))
                Picker(tr("위치", "Position", "位置"), selection: model.binding(\.mac.hudPosition)) {
                    Text(tr("커서 위 (커서 위치를 알 수 없는 앱에서는 표시하지 않음)", "Above the cursor (hidden in apps that don’t report it)",
                            "カーソルの上（位置が分からないアプリでは表示しない）")).tag("caret")
                    Text(tr("마우스 포인터 옆", "Next to the mouse pointer", "マウスポインタの横")).tag("mouse")
                }
                .disabled(!model.config.mac.hud)
            }

            Section {
                LabeledContent(tr("Electron 앱", "Electron Apps", "Electron アプリ")) {
                    ValueSlider(value: model.config.mac.shiftEnterDelayMs, range: 5...100, step: 5) { ms in
                        model.update { $0.mac.shiftEnterDelayMs = ms }
                    }
                }
                DisclosureGroup(tr("고급", "Advanced", "詳細")) {
                    LabeledContent("Codex") {
                        ValueSlider(value: model.config.mac.newlineReplayMs, range: 20...500, step: 10) { ms in
                            model.update { $0.mac.newlineReplayMs = ms }
                        }
                    }
                }
            } header: {
                Text(tr("Shift+Enter 줄바꿈 대기", "Shift+Enter Newline Delay", "Shift+Enter 改行の待ち時間"))
            } footer: {
                Text(tr("""
                    조합 중인 글자를 확정한 뒤 줄바꿈을 넣기까지 기다리는 시간입니다. Discord·Slack 같은 Electron 앱에서 조합 중인 \
                    글자가 사라지면 값을 높여 보세요. 조합 중에 누른 ⌘ 단축키도 이만큼 기다렸다가 앱에 다시 보냅니다.
                    고급의 Codex 값은 따로 쓰입니다. Codex는 줄바꿈 글자를 넣으면 메시지를 보내 버려서, 확정한 뒤 이만큼 기다렸다가 \
                    Shift+Enter를 다시 보냅니다. 확정만 되고 줄바꿈이 안 되면 값을 높이고, 잘 되면 낮춰서 되는 가장 작은 값을 찾으세요.
                    """, """
                    How long to wait between committing the composition and inserting the newline. Increase it if composing text \
                    disappears in Electron apps such as Discord and Slack. ⌘ shortcuts pressed while composing are re-sent after the \
                    same wait.
                    The Codex value under Advanced is separate. Codex sends the message when a newline character is inserted, so cssgsg \
                    commits, waits this long, and re-sends Shift+Enter. If you only get the commit and no newline, increase it; if it \
                    works, lower it to find the smallest value that still works.
                    """, """
                    入力中の文字を確定してから改行を入れるまでの待ち時間です。Discord・Slack などの Electron アプリで入力中の文字が\
                    消える場合は値を上げてください。入力中に押した ⌘ ショートカットも、同じだけ待ってからアプリへ送り直します。
                    詳細の Codex の値は別に使われます。Codex は改行文字を入れるとメッセージを送信してしまうため、確定後この時間だけ\
                    待ってから Shift+Enter を送り直します。確定だけで改行されない場合は値を上げ、うまくいく場合は下げて、動作する最小の\
                    値を見つけてください。
                    """))
            }
        }
        .formStyle(.grouped)
    }
}
