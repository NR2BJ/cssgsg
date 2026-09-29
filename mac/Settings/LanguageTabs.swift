import SwiftUI

// MARK: - 한국어

struct KoreanTab: View {
    @ObservedObject var model: SettingsModel
    @State private var confirmClear = false

    private var hanjaKey: String { model.config.shortcuts.hanja }

    var body: some View {
        Form {
            Section {
                Picker(tr("배열", "Layout", "配列"), selection: model.binding(\.koLayout)) {
                    Text(tr("참신세벌식 v18 (기본형)", "Chamshin Sebeolsik v18 (standard)", "チャムシン3ボル式 v18（基本形）"))
                        .tag("chamshin-v18")
                    Text(tr("참신세벌식 D v19", "Chamshin Sebeolsik D v19", "チャムシン3ボル式 D v19")).tag("chamshin-d-v19")
                }
                .pickerStyle(.radioGroup)
            } header: {
                Text(tr("배열", "Layout", "配列"))
            } footer: {
                Text(tr("D는 숫자 줄에도 글자가 있습니다. 두 배열은 배열 학습 탭에서 볼 수 있습니다.",
                        "D also puts letters on the number row. Both layouts are shown in the Layouts tab.",
                        "D は数字の段にも文字があります。二つの配列は「配列の学習」タブで見られます。"))
            }
            Section {
                LabeledContent(tr("한자 단축키", "Hanja Shortcut", "ハンジャのショートカット"), value: ShortcutText.label(hanjaKey))
                LabeledContent(tr("고른 한자 기억", "Remembered Choices", "選んだ漢字の記憶")) {
                    HStack {
                        Text(tr("\(model.hanjaLearningCount)개", "\(model.hanjaLearningCount)", "\(model.hanjaLearningCount) 件"))
                            .monospacedDigit()
                        Button(tr("지우기…", "Clear…", "消去…")) { confirmClear = true }
                            .disabled(model.hanjaLearningCount == 0)
                    }
                }
            } header: {
                Text(tr("한자", "Hanja", "ハンジャ（漢字）"))
            } footer: {
                Text(tr("""
                    조합 중인 글자에서 한자 단축키를 누르면 한자로 바꿉니다(자음 하나면 기호). 단축키는 일반 탭에서 바꿀 수 있습니다.
                    변환 중에는 Space·↓로 다음 후보, ↑로 이전 후보, Tab으로 후보 펼치기, 1–9로 번호 확정, Return으로 확정, \
                    Esc·Delete로 원래대로 돌아갑니다. 고른 한자는 기억해 두었다가 다음에 먼저 보여 줍니다.
                    """, """
                    Press the Hanja shortcut on the syllable being composed to convert it (a lone consonant opens symbols). You can \
                    change the shortcut in the General tab.
                    While converting: Space/↓ next candidate, ↑ previous, Tab expands the list, 1–9 picks by number, Return confirms, \
                    Esc/Delete reverts. Your choices are remembered and shown first next time.
                    """, """
                    入力中の文字でハンジャのショートカットを押すと漢字に変換します（子音だけなら記号）。ショートカットは「一般」タブで\
                    変更できます。
                    変換中は Space・↓ で次の候補、↑ で前の候補、Tab で候補を展開、1–9 で番号を選んで確定、Return で確定、\
                    Esc・Delete で元に戻ります。選んだ漢字は記憶して、次から先に表示します。
                    """))
            }
        }
        .formStyle(.grouped)
        .confirmationDialog(tr("고른 한자 기억을 모두 지우시겠습니까?", "Clear all remembered Hanja choices?",
                               "選んだ漢字の記憶をすべて消去しますか？"),
                            isPresented: $confirmClear) {
            Button(tr("지우기", "Clear", "消去"), role: .destructive) { model.clearHanjaLearning() }
        }
    }
}

// MARK: - 일본어

struct JapaneseTab: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var dictionary: UserDictionaryModel
    @State private var confirmClear = false

    var body: some View {
        Form {
            Section {
                Toggle(tr("Space로 변환", "Convert with Space", "Space で変換"), isOn: model.binding(\.ja.convertWithSpace))
                Toggle(tr("Tab으로 변환", "Convert with Tab", "Tab で変換"), isOn: model.binding(\.ja.convertWithTab))
                if !model.config.ja.convertWithSpace && !model.config.ja.convertWithTab {
                    Label(tr("둘 다 끄면 한자로 변환하지 않습니다(가나만 입력).", "With both off, there is no kanji conversion (kana only).",
                             "両方オフにすると漢字に変換しません（かなのみ）。"),
                          systemImage: "info.circle")
                        .foregroundStyle(.secondary)
                }
            } header: {
                Text(tr("변환 키", "Conversion Keys", "変換キー"))
            } footer: {
                Text(tr("가나를 입력하는 중에 누르면 변환을 시작합니다. 끈 키는 입력한 가나를 그대로 확정하고, Space는 공백을 입력하고 Tab은 앱에 넘깁니다.",
                        "Pressed while typing kana, these start conversion. A key that’s turned off confirms the kana as typed; Space then types a space and Tab goes to the app.",
                        "かなの入力中に押すと変換を始めます。オフにしたキーは入力したかなをそのまま確定し、Space は空白を入力、Tab はアプリに渡します。"))
            }

            Section {
                Picker(tr("구두점", "Punctuation", "句読点"), selection: model.binding(\.ja.punctuation)) {
                    Text(tr("일본식 (、。「」)", "Japanese (、。「」)", "和文（、。「」）")).tag("japanese")
                    Text(tr("전각 서양식 (，．［］)", "Full-width Western (，．［］)", "全角欧文（，．［］）")).tag("full_width_western")
                    Text(tr("반각 서양식 (,.[])", "Half-width Western (,.[])", "半角欧文（,.[]）")).tag("half_width_western")
                }
                Picker(tr("공백 너비", "Space Width", "スペース幅"), selection: model.binding(\.ja.fullWidthSpace)) {
                    Text(tr("반각", "Half-width", "半角")).tag(false)
                    Text(tr("전각", "Full-width", "全角")).tag(true)
                }
                .pickerStyle(.segmented)
                Toggle(tr("/ 키 → ・ (나카구로)", "/ Key → ・ (Nakaguro)", "/ キー → ・（中黒）"), isOn: model.binding(\.ja.slashNakaguro))
                Toggle(tr("\\ 키 → ¥ (엔 기호)", "\\ Key → ¥ (Yen Sign)", "\\ キー → ¥（円記号）"), isOn: model.binding(\.ja.yenSign))
            } header: {
                Text(tr("구두점 및 기호", "Punctuation and Symbols", "句読点と記号"))
            } footer: {
                Text(tr("공백 너비는 입력 중이 아닐 때 누른 Space에 적용됩니다. ¥ 기호를 끄면 \\ 키로 반각 \\를 입력합니다.",
                        "Space width applies when you’re not typing kana. With the yen sign off, the \\ key types a half-width \\.",
                        "スペース幅は、入力中でないときに押した Space に適用されます。円記号をオフにすると \\ キーで半角の \\ を入力します。"))
            }

            Section(tr("가타카나 (Caps Lock)", "Katakana (Caps Lock)", "カタカナ（Caps Lock）")) {
                Toggle(tr("입력하는 대로 바로 확정 (마지막 글자만 잠시 밑줄)", "Confirm as You Type (only the last kana stays underlined)",
                          "入力したそばから確定（最後の一文字だけ下線）"),
                       isOn: model.binding(\.ja.katakanaDirect))
                Toggle(tr("일본어 모드를 나가면 Caps Lock 끄기", "Turn Off Caps Lock When Leaving Japanese",
                          "日本語モードを抜けたら Caps Lock をオフ"),
                       isOn: model.binding(\.ja.capsKatakanaAutoOff))
            }

            UserDictionarySection(dictionary: dictionary)

            Section {
                LabeledContent(tr("Mozc 변환 학습", "Mozc Conversion History", "Mozc の変換学習")) {
                    Button(tr("지우기…", "Clear…", "消去…")) { confirmClear = true }
                }
            } header: {
                Text(tr("변환 학습", "Conversion History", "変換の学習"))
            } footer: {
                Text(tr("Mozc가 기억한 변환(문절 나누기, 고른 후보)을 지웁니다. 개인 사전은 그대로 남습니다. 지우면 입력기가 다시 시작됩니다(다음 키 입력 때 자동으로 켜집니다).",
                        "Clears what Mozc learned (segmentation and chosen candidates). The user dictionary stays. The input method restarts and comes back on the next key press.",
                        "Mozc が覚えた変換（文節の区切り、選んだ候補）を消去します。ユーザー辞書はそのまま残ります。入力メソッドが再起動します（次のキー入力で自動的に起動します）。"))
            }

            Section(tr("변환 단축키", "Conversion Shortcuts", "変換ショートカット")) {
                CheatSheet(title: tr("입력 중", "While Typing", "入力中"), rows: Self.typingRows)
                CheatSheet(title: tr("변환 중", "While Converting", "変換中"), rows: Self.convertingRows)
            }
        }
        .formStyle(.grouped)
        .confirmationDialog(tr("Mozc 변환 학습을 지우고 입력기를 다시 시작하시겠습니까?", "Clear Mozc’s history and restart the input method?",
                               "Mozc の学習を消去して、入力メソッドを再起動しますか？"),
                            isPresented: $confirmClear) {
            Button(tr("지우기", "Clear", "消去"), role: .destructive) { model.clearMozcLearning() }
        }
    }

    private static var typingRows: [CheatSheet.Row] {
        [
            .init(["Space", "Tab"], tr("변환 시작 (위 설정에 따름)", "Start conversion (per the settings above)", "変換開始（上の設定に従う）")),
            .init(["Return"], tr("입력한 그대로 확정", "Confirm as typed", "入力どおりに確定")),
            .init(["⇧ Shift + Return"], tr("확정하고 줄바꿈", "Confirm and start a new line", "確定して改行")),
            .init(["⌫ Delete"], tr("한 글자 지우기", "Delete one character", "1文字削除")),
            .init(["Esc"], tr("입력 취소", "Cancel input", "入力を取り消し")),
            .init(["Caps Lock"], tr("가타카나 입력", "Type katakana", "カタカナ入力")),
            .init(["⇧ Shift"], tr("A–Z 자리에서는 무시(로마자 입력 없음), 그 밖의 자리에서는 Shift 기호 입력",
                                  "Ignored on the A–Z keys (no romaji input); other keys type their shifted symbol",
                                  "A–Z の位置では無視（ローマ字入力なし）、それ以外の位置では Shift の記号を入力")),
        ]
    }

    private static var convertingRows: [CheatSheet.Row] {
        [
            .init(["Space", "↓"], tr("다음 후보", "Next candidate", "次の候補")),
            .init(["↑"], tr("이전 후보", "Previous candidate", "前の候補")),
            .init(["Tab"], tr("후보 펼치기 / 접기", "Expand / collapse candidates", "候補の展開 / 折りたたみ")),
            .init(["←", "→"], tr("문절 이동 (문절이 하나면 후보 페이지 넘김, 펼친 목록에서는 한 칸 이동)",
                                 "Move between segments (pages the list when there’s one segment; one cell when expanded)",
                                 "文節間を移動（文節が1つなら候補のページ送り、展開中は1マス移動）")),
            .init(["⇧ Shift + ←", "⇧ Shift + →"], tr("문절 길이 조절", "Resize segment", "文節の長さを変更")),
            .init(["Page Up", "Page Down"], tr("후보 페이지 넘김", "Page through candidates", "候補のページ送り")),
            .init(["1–9"], tr("번호로 후보 확정", "Pick a candidate by number", "番号で候補を確定")),
            .init(["Return"], tr("변환 확정", "Confirm conversion", "変換を確定")),
            .init(["⇧ Shift + Return"], tr("확정하고 줄바꿈", "Confirm and start a new line", "確定して改行")),
            .init(["Esc", "⌫ Delete"], tr("변환 취소 (입력으로 돌아감)", "Cancel conversion (back to input)", "変換をキャンセル（入力に戻る）")),
            .init([tr("다른 키", "Other keys", "その他のキー")], tr("확정하고 이어서 입력", "Confirm and keep typing", "確定して入力を続ける")),
        ]
    }
}

/// 키와 설명 표.
struct CheatSheet: View {
    struct Row: Identifiable {
        let id = UUID()
        let keys: [String]
        let text: String

        init(_ keys: [String], _ text: String) {
            (self.keys, self.text) = (keys, text)
        }
    }

    let title: String
    let rows: [Row]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.subheadline.weight(.semibold))
            Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 14, verticalSpacing: 6) {
                ForEach(rows) { row in
                    GridRow {
                        HStack(spacing: 4) {
                            ForEach(row.keys, id: \.self) { KeyCap(text: $0) }
                        }
                        .gridColumnAlignment(.trailing)
                        Text(row.text)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }
}
