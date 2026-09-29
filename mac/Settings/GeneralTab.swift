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
                    수식키는 왼쪽·오른쪽을 가린다(왼쪽 Option + Return과 오른쪽 Option + Return은 다르다). ⌘ 조합은 쓸 수 없다. Esc는 취소.
                    영어 ↔ 비영어: 영어와 방금 쓰던 언어(한국어·일본어)를 오간다. 한국어 ↔ 일본어: 영어에서 누르면 둘 중 방금 쓰지 않은 쪽으로 간다.
                    """, """
                    Click a button, then press the keys. Pressing a single modifier alone briefly is a tap; a modifier with another key is a combination. \
                    Left and right modifiers are distinct (Left Option + Return ≠ Right Option + Return). ⌘ combinations aren’t allowed. Esc cancels.
                    English ↔ non-English switches between English and the language you used last (Korean or Japanese). \
                    Korean ↔ Japanese, pressed in English, goes to the one you didn’t use last.
                    """, """
                    ボタンを押してから使うキーを押します。修飾キーを一つだけ短く押して離すとタップ、修飾キーと別のキーを一緒に押すと組み合わせです。\
                    修飾キーは左右を区別します（左Option + Return と右Option + Return は別）。⌘ の組み合わせは使えません。Esc で取り消し。
                    英語 ↔ 英語以外：英語と直前に使った言語（韓国語・日本語）を行き来します。韓国語 ↔ 日本語：英語で押すと直前に使っていない方へ切り替えます。
                    """))
            }

            Section {
                LabeledContent(tr("탭 인식 시간", "Tap time limit", "タップ判定時間")) {
                    ValueSlider(value: model.config.tapThresholdMs, range: 100...500, step: 10) { ms in
                        model.update { $0.tapThresholdMs = ms }
                    }
                }
            } header: {
                Text(tr("수식키 탭", "Modifier taps", "修飾キーのタップ"))
            } footer: {
                Text(tr("수식키를 탭 인식 시간보다 오래 누르면 탭이 아니다.",
                        "Holding a modifier longer than the tap time limit is not a tap.",
                        "修飾キーをタップ判定時間より長く押すとタップになりません。"))
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
                LabeledContent(tr("줄바꿈 대기 조정", "Newline delay adjustment", "改行待ち時間の調整")) {
                    ValueSlider(value: model.config.mac.newlineDelayOffsetMs, range: -50...50, step: 5, signed: true) { ms in
                        model.update { $0.mac.newlineDelayOffsetMs = ms }
                    }
                }
                LabeledContent(tr("지금 대기", "Current delays", "現在の待ち時間")) {
                    Text(tr("Electron \(model.config.mac.electronDelayMs)ms · Codex \(model.config.mac.codexDelayMs)ms",
                            "Electron \(model.config.mac.electronDelayMs) ms · Codex \(model.config.mac.codexDelayMs) ms",
                            "Electron \(model.config.mac.electronDelayMs)ms・Codex \(model.config.mac.codexDelayMs)ms"))
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                }
            } header: {
                Text(tr("조합 중 Shift+Enter", "Shift+Enter while composing", "変換中の Shift+Enter"))
            } footer: {
                Text(tr("""
                    Shift+Enter를 누르면 글자를 확정하고 조금 기다렸다 줄을 바꾼다. 기다리는 시간은 앱 종류마다 따로 있다(더하지 않는다): \
                    Discord·Slack 같은 Electron 앱은 15ms(⌘+키 다시 보내기도 같다), Codex처럼 Enter를 전송으로 받는 앱은 120ms 뒤에 \
                    Shift+Enter 키를 다시 보낸다(NRIME에서 찾은 값). 줄바꿈이 확정한 글자를 먹거나 전송되면 늘리고, 느리면 줄인다.
                    """, """
                    Shift+Enter commits the text, waits briefly, then starts a new line. Each kind of app has its own wait (they \
                    don’t add up): Electron apps like Discord and Slack 15 ms (also used to re-send ⌘+key); apps that treat Enter as \
                    send, like Codex, get Shift+Enter re-sent after 120 ms (values found in NRIME). Increase if the newline eats the \
                    text or sends the message; decrease if it feels slow.
                    """, """
                    Shift+Enter を押すと文字を確定し、少し待ってから改行します。待ち時間はアプリの種類ごとに別です（足し合わせません）：\
                    Discord・Slack などの Electron アプリは 15ms（⌘+キーの送り直しも同じ）、Codex のように Enter を送信として扱う\
                    アプリは 120ms 後に Shift+Enter を送り直します（NRIME で見つけた値）。改行が確定した文字を消したり送信されたりしたら\
                    長く、遅く感じたら短くします。
                    """))
            }
        }
        .formStyle(.grouped)
    }
}
