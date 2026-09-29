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
                Text(tr("D는 숫자 줄에도 글자가 있다. 두 배열의 자리는 배열 학습 탭에 있다.",
                        "D also has letters on the number row. Both layouts are shown in the Layouts tab.",
                        "D は数字の段にも文字があります。二つの配列は「配列の学習」タブにあります。"))
            }
            Section {
                LabeledContent(tr("한자 단축키", "Hanja shortcut", "ハンジャのショートカット"), value: ShortcutText.label(hanjaKey))
                LabeledContent(tr("고른 한자 기억", "Remembered choices", "選んだ漢字の記憶")) {
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
                    조합 중인 글자에서 한자 단축키를 누르면 한자로 바꾼다(자음 하나면 기호). 단축키는 일반 탭에서 바꾼다.
                    바꾸는 중: Space·↓ 다음, ↑ 이전, Tab 펼치기, 1–9 골라 확정, Return 확정, Esc·Delete 되돌리기. 고른 한자는 기억해서 다음에 먼저 보인다.
                    """, """
                    Press the Hanja shortcut on the syllable being composed to convert it (a lone consonant gives symbols). \
                    Change the shortcut in the General tab.
                    While converting: Space/↓ next, ↑ previous, Tab expand, 1–9 pick, Return commit, Esc/Delete revert. \
                    Your choices are remembered and shown first next time.
                    """, """
                    入力中の文字でハンジャのショートカットを押すと漢字に変換します（子音だけなら記号）。ショートカットは「一般」タブで変更します。
                    変換中：Space・↓ 次、↑ 前、Tab 展開、1–9 選んで確定、Return 確定、Esc・Delete 元に戻す。選んだ漢字は記憶して次から先に出します。
                    """))
            }
        }
        .formStyle(.grouped)
        .confirmationDialog(tr("고른 한자 기억을 모두 지울까?", "Clear all remembered Hanja choices?", "選んだ漢字の記憶をすべて消しますか？"),
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
                    Label(tr("둘 다 끄면 한자로 변환하지 않는다(가나만).", "With both off there is no kanji conversion (kana only).",
                             "両方オフにすると漢字に変換しません（かなのみ）。"),
                          systemImage: "info.circle")
                        .foregroundStyle(.secondary)
                }
            } header: {
                Text(tr("변환 키", "Conversion keys", "変換キー"))
            } footer: {
                Text(tr("가나를 치는 중에 누르면 한자로 변환한다. 끈 키는 친 가나를 그대로 확정하고, Space는 공백을 치고 Tab은 앱에 넘긴다.",
                        "Pressed while typing kana, these convert to kanji. A key turned off commits the kana as typed; Space then types a space and Tab goes to the app.",
                        "かなの入力中に押すと漢字に変換します。オフにしたキーは入力したかなをそのまま確定し、Space は空白を入力、Tab はアプリに渡します。"))
            }

            Section {
                Picker(tr("구두점", "Punctuation", "句読点"), selection: model.binding(\.ja.punctuation)) {
                    Text(tr("、。「」 일본식", "、。「」 Japanese", "、。「」 和文")).tag("japanese")
                    Text(tr("，．［］ 전각 서양식", "，．［］ Full-width Western", "，．［］ 全角欧文")).tag("full_width_western")
                    Text(tr(",.[] 반각", ",.[] Half-width", ",.[] 半角")).tag("half_width_western")
                }
                Picker(tr("공백", "Space", "空白"), selection: model.binding(\.ja.fullWidthSpace)) {
                    Text(tr("반각", "Half-width", "半角")).tag(false)
                    Text(tr("전각 (　)", "Full-width (　)", "全角（　）")).tag(true)
                }
                .pickerStyle(.segmented)
                Toggle(tr("/ 자리로 ・를 친다", "Type ・ on the / key", "/ キーで ・ を入力"), isOn: model.binding(\.ja.slashNakaguro))
                Toggle(tr("\\ 자리로 ¥를 친다 (끄면 \\)", "Type ¥ on the \\ key (off: \\)", "\\ キーで ¥ を入力（オフなら \\）"),
                       isOn: model.binding(\.ja.yenSign))
            } header: {
                Text(tr("기호", "Symbols", "記号"))
            }

            Section(tr("가타카나 (Caps Lock)", "Katakana (Caps Lock)", "カタカナ（Caps Lock）")) {
                Toggle(tr("치는 대로 바로 확정한다 (마지막 글자만 잠깐 밑줄)", "Commit as you type (only the last kana stays underlined)",
                          "入力したそばから確定（最後の一文字だけ下線）"),
                       isOn: model.binding(\.ja.katakanaDirect))
                Toggle(tr("일본어 모드를 나가면 Caps Lock을 끈다", "Turn Caps Lock off when leaving Japanese",
                          "日本語モードを抜けたら Caps Lock を切る"),
                       isOn: model.binding(\.ja.capsKatakanaAutoOff))
            }

            UserDictionarySection(dictionary: dictionary)

            Section {
                LabeledContent(tr("변환 학습 (Mozc)", "Conversion history (Mozc)", "変換の学習（Mozc）")) {
                    Button(tr("지우기…", "Clear…", "消去…")) { confirmClear = true }
                }
            } header: {
                Text(tr("학습", "Learning", "学習"))
            } footer: {
                Text(tr("Mozc가 기억한 변환(문절 나누기, 고른 후보)을 지운다. 개인 사전은 그대로다. 지우면 입력기가 다시 시작된다(다음 키 입력 때 저절로 뜬다).",
                        "Clears what Mozc learned (segmentation, chosen candidates). The user dictionary stays. The input method restarts (it comes back on the next key press).",
                        "Mozc が覚えた変換（文節の区切り、選んだ候補）を消します。ユーザー辞書はそのままです。入力メソッドが再起動します（次のキー入力で自動的に起動）。"))
            }

            Section(tr("변환 단축키", "Conversion keys at a glance", "変換キー一覧")) {
                CheatSheet(title: tr("읽기를 치는 중", "While typing a reading", "読みの入力中"), rows: Self.typingRows)
                CheatSheet(title: tr("변환 중", "While converting", "変換中"), rows: Self.convertingRows)
            }
        }
        .formStyle(.grouped)
        .confirmationDialog(tr("Mozc 변환 학습을 지우고 입력기를 다시 시작할까?", "Clear Mozc’s learning and restart the input method?",
                               "Mozc の学習を消して入力メソッドを再起動しますか？"),
                            isPresented: $confirmClear) {
            Button(tr("지우기", "Clear", "消去"), role: .destructive) { model.clearMozcLearning() }
        }
    }

    private static var typingRows: [CheatSheet.Row] {
        [
            .init(["Space", "Tab"], tr("변환 (위 설정을 따른다)", "Convert (follows the settings above)", "変換（上の設定に従う）")),
            .init(["Return"], tr("읽기를 그대로 확정", "Commit the reading as is", "読みをそのまま確定")),
            .init(["⇧ Shift + Return"], tr("확정하고 줄바꿈", "Commit and start a new line", "確定して改行")),
            .init(["⌫ Delete"], tr("한 글자 지우기", "Delete one character", "一文字消す")),
            .init(["Esc"], tr("읽기 지우기", "Clear the reading", "読みを消す")),
            .init(["Caps Lock"], tr("가타카나로 치기", "Type katakana", "カタカナで入力")),
            .init(["⇧ Shift"], tr("A–Z 자리에서는 무시(로마자 입력 없음), 그 밖의 자리에서는 Shift 기호",
                                  "Ignored on the A–Z keys (no romaji input); other keys type their shifted symbol",
                                  "A–Z の位置では無視（ローマ字入力なし）、それ以外の位置では Shift の記号")),
        ]
    }

    private static var convertingRows: [CheatSheet.Row] {
        [
            .init(["Space", "↓"], tr("다음 후보", "Next candidate", "次の候補")),
            .init(["↑"], tr("이전 후보", "Previous candidate", "前の候補")),
            .init(["Tab"], tr("후보 목록 펼치기 ↔ 접기", "Expand ↔ collapse the candidate list", "候補一覧の展開 ↔ 折りたたみ")),
            .init(["←", "→"], tr("문절 옮기기 (문절이 하나면 페이지, 펼친 목록에서는 한 칸)",
                                 "Move between segments (pages with a single segment, one cell when expanded)",
                                 "文節の移動（文節が一つならページ送り、展開中は一マス）")),
            .init(["⇧ Shift + ←", "⇧ Shift + →"], tr("문절 줄이기 · 늘리기", "Shrink · extend the segment", "文節を縮める・伸ばす")),
            .init(["Page Up", "Page Down"], tr("페이지 넘기기", "Page through candidates", "ページ送り")),
            .init(["1–9"], tr("그 번호 후보로 확정", "Commit that candidate", "その番号の候補で確定")),
            .init(["Return"], tr("확정", "Commit", "確定")),
            .init(["⇧ Shift + Return"], tr("확정하고 줄바꿈", "Commit and start a new line", "確定して改行")),
            .init(["Esc", "⌫ Delete"], tr("변환 취소 (읽기로)", "Cancel (back to the reading)", "変換を取り消す（読みに戻る）")),
            .init([tr("다른 키", "Other keys", "その他のキー")], tr("확정하고 그 키를 친다", "Commit, then type that key",
                                                              "確定してそのキーを入力")),
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
