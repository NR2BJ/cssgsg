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

            MozcEngineSection(model: model)

            Section {
                LabeledContent(tr("Mozc 변환 학습", "Mozc Conversion History", "Mozc の変換学習")) {
                    Button(tr("지우기…", "Clear…", "消去…")) { confirmClear = true }
                }
            } header: {
                Text(tr("변환 학습", "Conversion History", "変換の学習"))
            } footer: {
                Text(tr("Mozc가 기억한 변환(문절 나누기, 고른 후보)을 지웁니다. 개인 사전은 그대로 남습니다. 지우면 입력기가 다시 시작됩니다.",
                        "Clears what Mozc learned (segmentation and chosen candidates). The user dictionary stays. The input method restarts.",
                        "Mozc が覚えた変換（文節の区切り、選んだ候補）を消去します。ユーザー辞書はそのまま残ります。入力メソッドが再起動します。"))
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

/// 변환 엔진(Mozc): 입력기가 쓰는 판, 받아 두고 기다리는 새 판, 업데이트 확인. Mozc는 cssgsg와 따로 업데이트된다
/// (입력기의 MozcUpdater). 상태는 입력기가 적어 두고 알린다(MozcStatus). NRIME 설정 앱의 일본어 → 변환 엔진과 같다.
struct MozcEngineSection: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Section {
            LabeledContent(tr("버전", "Version", "バージョン")) {
                VStack(alignment: .trailing, spacing: 2) {
                    Text(verbatim: versionText)
                        .monospacedDigit()
                        .textSelection(.enabled)
                    Text(sourceText)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            if let pending = model.mozc?.pending {
                HStack {
                    Text(tr("새 Mozc \(pending.version) (\(pending.date))를 받아 두었습니다. 입력기가 다시 시작하면 적용됩니다.",
                            "New Mozc \(pending.version) (\(pending.date)) is downloaded and applies when the input method restarts.",
                            "新しい Mozc \(pending.version)（\(pending.date)）をダウンロード済みです。入力メソッドの再起動時に適用されます。"))
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer()
                    Button(tr("지금 적용", "Apply Now", "今すぐ適用")) { model.applyMozcUpdate() }
                        .help(tr("입력기가 곧바로 다시 시작하며 새 Mozc를 씁니다.", "The input method restarts right away with the new Mozc.",
                                 "入力メソッドがすぐに再起動し、新しい Mozc を使います。"))
                }
            }

            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(tr("업데이트", "Updates", "アップデート"))
                    if let caption = checkCaption {
                        Text(caption)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                Spacer()
                if model.mozcChecking {
                    ProgressView().controlSize(.small)
                }
                Button(tr("지금 확인", "Check Now", "今すぐ確認")) { model.checkMozcUpdate() }
                    .disabled(model.mozcChecking || !model.imeRunning)
            }
        } header: {
            Text(tr("변환 엔진 (Mozc)", "Conversion Engine (Mozc)", "変換エンジン（Mozc）"))
        } footer: {
            Text(tr("Mozc는 cssgsg와 따로 업데이트됩니다. 입력기가 하루에 한 번 GitHub에서 새 버전을 확인해 내려받고, 다음에 입력기가 시작될 때부터 씁니다.",
                    "Mozc updates separately from cssgsg. Once a day the input method checks GitHub for a newer one, downloads it, and uses it from its next start.",
                    "Mozc は cssgsg とは別にアップデートされます。入力メソッドが1日に1回 GitHub で新しいバージョンを確認してダウンロードし、次に起動したときから使います。"))
        }
        .onAppear {
            model.refreshIMEStatus()
            model.readMozcStatus()
        }
    }

    /// "3.34.6239.101 (2026-09-28)": 입력기가 쓴다고 적은 것, 아직 적지 않았으면 앱에 든 것.
    private var versionText: String {
        guard let build = model.mozc?.active ?? SettingsModel.bundledMozc else { return "—" }
        return "\(build.version) (\(build.date))"
    }

    private var sourceText: String {
        guard let status = model.mozc, status.active != nil else {
            return model.mozc == nil ? tr("앱에 들어 있는 엔진", "Engine inside the app", "アプリ内のエンジン")
                : tr("엔진을 읽지 못했습니다 (가나만 입력)", "The engine couldn’t be loaded (kana only)",
                     "エンジンを読み込めませんでした（かなのみ）")
        }
        return status.activeSource == "downloaded"
            ? tr("내려받은 엔진", "Downloaded engine", "ダウンロードしたエンジン")
            : tr("앱에 들어 있는 엔진", "Engine inside the app", "アプリ内のエンジン")
    }

    private var checkCaption: String? {
        if !model.imeRunning {
            return tr("입력기가 실행 중이 아닙니다", "The input method isn’t running", "入力メソッドが起動していません")
        }
        if let failed = model.mozc?.checkFailedAt, failed > (model.mozc?.checkedAt ?? .distantPast) {
            return tr("마지막 확인 실패: ", "Last check failed: ", "最終確認に失敗: ")
                + failed.formatted(date: .abbreviated, time: .shortened)
        }
        if let checked = model.mozc?.checkedAt {
            return tr("마지막 확인: ", "Last checked: ", "最終確認: ") + checked.formatted(date: .abbreviated, time: .shortened)
        }
        return nil
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
