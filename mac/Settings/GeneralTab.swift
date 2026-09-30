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
                Toggle(tr("빠른 탭 전환 보정 (실험적)", "Fast Tap-Switch Correction (Experimental)", "高速タップ切替補正（実験的）"),
                       isOn: model.binding(\.tapBuffering))
            } header: {
                Text(tr("수식키 탭", "Modifier Taps", "修飾キーのタップ"))
            } footer: {
                Text(tr("""
                    수식키를 이 시간보다 오래 누르고 있으면 탭으로 보지 않습니다.
                    빠른 탭 전환 보정: Shift를 탭하고 곧바로 글자를 쳐서 글자가 Shift보다 먼저 눌려도 언어를 바꿉니다. Shift가 \
                    아무 뜻이 없는 키(新月의 글자 키)는 글자를 누르고 80ms 안에 Shift를 떼면 전환하고, Shift가 글자를 바꾸는 키\
                    (영어 대문자, 참신세벌식의 Shift 기호)는 30ms 안에 뗐을 때만 전환합니다. 한글 조합 중(낱말 가운데)에는 \
                    보정하지 않습니다.
                    """, """
                    Holding a modifier longer than this doesn’t count as a tap.
                    Fast tap-switch correction switches languages even when you tap Shift and type so quickly that the letter \
                    goes down before Shift comes up. On keys where Shift means nothing (Shingetsu letter keys) it switches when \
                    Shift comes up within 80 ms of the letter; on keys where Shift changes the letter (English capitals, \
                    Chamshin Shift symbols) only within 30 ms. It never applies in the middle of a Korean syllable.
                    """, """
                    修飾キーをこの時間より長く押していると、タップとみなしません。
                    高速タップ切替補正：Shiftをタップしてすぐに文字を打ち、文字がShiftより先に押されても言語を切り替えます。\
                    Shiftに意味のないキー（新月の文字キー）は文字を押してから80ms以内にShiftを離すと切り替え、Shiftが文字を\
                    変えるキー（英語の大文字、チャムシン3ボル式のShift記号）は30ms以内に離したときだけ切り替えます。\
                    ハングル入力中（単語の途中）には補正しません。
                    """))
            }

            Section(tr("후보창", "Candidate Window", "候補ウィンドウ")) {
                LabeledContent(tr("글자 크기", "Font Size", "文字サイズ")) {
                    ValueSlider(value: model.config.mac.candidateFontSize, range: 10...28, step: 1, unit: "pt") { size in
                        model.update { $0.mac.candidateFontSize = size }
                    }
                }
            }

            Section {
                LabeledContent(tr("Electron·Chromium 앱", "Electron and Chromium Apps", "Electron・Chromiumアプリ")) {
                    ValueSlider(value: model.config.mac.newlineInsertWaitMs, range: 0...100, step: 5,
                                zeroLabel: tr("대기 없음", "No wait", "待ちなし")) { ms in
                        model.update { $0.mac.newlineInsertWaitMs = ms }
                    }
                }
                LabeledContent(tr("Shift+Enter를 다시 보내는 앱", "Apps That Get Shift+Enter Re-sent", "Shift+Enterを送り直すアプリ")) {
                    ValueSlider(value: model.config.mac.newlineKeyPressWaitMs, range: 0...200, step: 5,
                                zeroLabel: tr("대기 없음", "No wait", "待ちなし")) { ms in
                        model.update { $0.mac.newlineKeyPressWaitMs = ms }
                    }
                }
            } header: {
                Text(tr("Shift+Enter 줄바꿈 대기", "Shift+Enter Newline Wait", "Shift+Enter 改行の待ち時間"))
            } footer: {
                Text(tr("""
                    조합 중인 글자를 확정한 뒤 줄바꿈을 넣기까지 기다리는 시간입니다. 조합 중이던 글자가 사라지면 값을 늘려 보세요. \
                    알맞은 값은 Mac마다 다릅니다.
                    Electron·Chromium 앱: Discord, Slack, Claude, VS Code처럼 웹 기술로 만든 앱 전체입니다. 조합 중에 누른 ⌘ 단축키도 \
                    이만큼 기다렸다 다시 보냅니다. 기본 20ms.
                    Shift+Enter를 다시 보내는 앱: 줄바꿈 글자를 넣으면 메시지를 보내 버리는 앱(지금은 Codex)에는 확정한 뒤 \
                    Shift+Enter 키를 다시 보냅니다. 입력기 권한(정보 탭)이 필요합니다. 기본 50ms.
                    """, """
                    How long to wait after committing the composition before the newline goes in. If the syllable being composed \
                    disappears, increase it. The right value differs from Mac to Mac.
                    Electron and Chromium apps: every app built on web technology, such as Discord, Slack, Claude and VS Code. \
                    ⌘ shortcuts pressed while composing are re-sent after the same wait. Default 20 ms.
                    Apps that get Shift+Enter re-sent: apps that send the message when a newline character is inserted (Codex \
                    for now) get the Shift+Enter key press again after the commit. Needs the input method permission (About tab). \
                    Default 50 ms.
                    """, """
                    入力中の文字を確定してから改行を入れるまでの待ち時間です。入力中の文字が消える場合は値を上げてください。\
                    適切な値はMacごとに異なります。
                    Electron・Chromiumアプリ：Discord、Slack、Claude、VS CodeなどWeb技術で作られたアプリすべてです。\
                    入力中に押した⌘ショートカットも、同じだけ待ってから送り直します。デフォルト20ms。
                    Shift+Enterを送り直すアプリ：改行文字を入れるとメッセージを送信してしまうアプリ（現在はCodex）には、\
                    確定後にShift+Enterキーを送り直します。入力メソッドの許可（情報タブ）が必要です。デフォルト50ms。
                    """))
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

        }
        .formStyle(.grouped)
    }
}
