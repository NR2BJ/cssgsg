import SwiftUI

// MARK: - 일반

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
                    Label(tr("영어로 오가는 단축키가 없다. 메뉴 막대로는 모드를 바꿀 수 없으니 하나는 정해 두자.",
                             "There is no shortcut to switch to English. The menu bar can’t switch modes, so set one.",
                             "英語に切り替えるショートカットがありません。メニューバーからは切り替えられないので設定してください。"),
                          systemImage: "exclamationmark.triangle")
                        .foregroundStyle(.orange)
                }
            } header: {
                Text(tr("단축키", "Shortcuts", "ショートカット"))
            } footer: {
                Text(tr("""
                    단추를 누르고 쓸 키를 누른다. 수식키 하나만 혼자 짧게 눌렀다 떼면 탭, 수식키와 다른 키를 같이 누르면 조합이다. \
                    조합의 수식키는 왼쪽·오른쪽을 가리지 않는다. Esc는 취소.
                    영어 ↔ 비영어: 영어와 방금 쓰던 언어(한국어·일본어)를 오간다. 한국어 ↔ 일본어: 영어에서 누르면 둘 중 방금 쓰지 않은 쪽으로 간다.
                    """, """
                    Click a button, then press the keys. Pressing a single modifier alone briefly is a tap; a modifier with another key is a combination. \
                    Combinations don’t distinguish left and right modifiers. Esc cancels.
                    English ↔ non-English switches between English and the language you used last (Korean or Japanese). \
                    Korean ↔ Japanese, pressed in English, goes to the one you didn’t use last.
                    """, """
                    ボタンを押してから使うキーを押します。修飾キーを一つだけ短く押して離すとタップ、修飾キーと別のキーを一緒に押すと組み合わせです。\
                    組み合わせの修飾キーは左右を区別しません。Esc で取り消し。
                    英語 ↔ 英語以外：英語と直前に使った言語（韓国語・日本語）を行き来します。韓国語 ↔ 日本語：英語で押すと直前に使っていない方へ切り替えます。
                    """))
            }

            Section {
                LabeledContent(tr("탭 인식 시간", "Tap time limit", "タップ判定時間")) {
                    ValueSlider(value: model.config.tapThresholdMs, range: 100...500, step: 10) { ms in
                        model.update { $0.tapThresholdMs = ms }
                    }
                }
                Toggle(tr("빠른 탭 전환 보정 (실험적)", "Fast tap correction (experimental)", "高速タップ補正（実験的）"),
                       isOn: model.binding(\.tapBuffering))
                if model.config.tapBuffering {
                    LabeledContent(tr("겹침 허용", "Overlap allowance", "重なりの許容")) {
                        ValueSlider(value: model.config.tapOverlapMs, range: 30...80, step: 5) { ms in
                            model.update { $0.tapOverlapMs = ms }
                        }
                    }
                }
            } header: {
                Text(tr("수식키 탭", "Modifier taps", "修飾キーのタップ"))
            } footer: {
                Text(tr("""
                    수식키를 탭 인식 시간보다 오래 누르면 탭이 아니다.
                    빠른 탭 전환 보정: 탭 수식키를 떼기 전에 다음 글자를 눌러 버려도, 겹친 시간이 겹침 허용보다 짧으면 탭으로 보고 \
                    전환한 뒤 그 글자를 친다. 켜면 탭 수식키를 누른 채 친 글자는 겹침 허용만큼 늦게 나온다. \
                    그 수식키를 조합 단축키에도 쓰면 보정하지 않는다. Shift 탭을 쓰면 Shift로 대문자를 아주 빠르게 칠 때 전환으로 잘못 볼 수 있다.
                    """, """
                    Holding a modifier longer than the tap time limit is not a tap.
                    Fast tap correction: if you press the next letter before releasing a tap modifier and the overlap is shorter \
                    than the allowance, it still counts as a tap — the mode switches, then the letter is typed. Letters typed while \
                    holding a tap modifier appear after the allowance delay. Not applied when that modifier is also used in a combination. \
                    With a Shift tap, very fast Shift+letter capitals can be mistaken for a switch.
                    """, """
                    修飾キーをタップ判定時間より長く押すとタップになりません。
                    高速タップ補正：タップ用の修飾キーを離す前に次の文字を押しても、重なりが許容時間より短ければタップとみなし、\
                    切り替えてからその文字を入力します。オンにすると、タップ用の修飾キーを押したまま打った文字は許容時間だけ遅れて出ます。\
                    その修飾キーを組み合わせにも使っている場合は補正しません。Shift のタップを使うと、Shift で大文字をとても速く打ったときに切り替えと誤認することがあります。
                    """))
            }

            Section(tr("후보창", "Candidate window", "候補ウィンドウ")) {
                LabeledContent(tr("글자 크기", "Font size", "文字の大きさ")) {
                    ValueSlider(value: model.config.mac.candidateFontSize, range: 10...28, step: 1, unit: "pt") { size in
                        model.update { $0.mac.candidateFontSize = size }
                    }
                }
            }

            Section(tr("모드 표시", "Mode indicator", "モード表示")) {
                Toggle(tr("모드를 바꿀 때 G · ㅊ · 月을 잠깐 보인다", "Briefly show G · ㅊ · 月 when the mode changes",
                          "モードを切り替えたとき G · ㅊ · 月 を一瞬表示する"),
                       isOn: model.binding(\.mac.hud))
                Picker(tr("자리", "Position", "位置"), selection: model.binding(\.mac.hudPosition)) {
                    Text(tr("커서 위 (커서 자리를 모르는 앱에서는 안 보임)", "Above the cursor (hidden in apps that don’t report it)",
                            "カーソルの上（位置が分からないアプリでは出ない）")).tag("caret")
                    Text(tr("마우스 옆", "Next to the mouse", "マウスの横")).tag("mouse")
                }
                .disabled(!model.config.mac.hud)
            }

            Section {
                LabeledContent(tr("Shift+Enter 대기", "Shift+Enter delay", "Shift+Enter の待ち時間")) {
                    ValueSlider(value: model.config.mac.shiftEnterDelayMs, range: 5...50, step: 5) { ms in
                        model.update { $0.mac.shiftEnterDelayMs = ms }
                    }
                }
                DisclosureGroup(tr("고급", "Advanced", "詳細")) {
                    LabeledContent(tr("Codex 줄바꿈 대기", "Codex newline delay", "Codex の改行待ち時間")) {
                        ValueSlider(value: model.config.mac.newlineReplayMs, range: 20...500, step: 10) { ms in
                            model.update { $0.mac.newlineReplayMs = ms }
                        }
                    }
                }
            } header: {
                Text(tr("Electron 앱 (Discord, Slack …)", "Electron apps (Discord, Slack …)", "Electron アプリ（Discord、Slack …）"))
            } footer: {
                Text(tr("""
                    조합 중 Shift+Enter를 누르면 글자를 확정하고 이만큼 기다렸다 줄을 바꾼다(조합 중 ⌘+키도 이만큼 기다렸다 다시 보낸다). \
                    줄바꿈이 확정한 글자를 먹으면 늘린다.
                    고급: Codex처럼 Enter를 전송으로 받는 앱은 줄바꿈 대신 Shift+Enter 키를 다시 보내는데, 그 전에 기다리는 시간이다.
                    """, """
                    Shift+Enter while composing commits the text, waits this long, then inserts the newline (⌘+key while composing \
                    is re-sent after the same wait). Increase it if the newline swallows the committed text.
                    Advanced: apps that treat Enter as send, like Codex, get Shift+Enter re-sent instead; this is the wait before that.
                    """, """
                    変換中に Shift+Enter を押すと、文字を確定してこの時間だけ待ってから改行します（変換中の ⌘+キーも同じだけ待って送り直します）。\
                    改行が確定した文字を消してしまう場合は長くします。
                    詳細：Codex のように Enter を送信として扱うアプリでは改行の代わりに Shift+Enter を送り直します。その前の待ち時間です。
                    """))
            }
        }
        .formStyle(.grouped)
    }
}
