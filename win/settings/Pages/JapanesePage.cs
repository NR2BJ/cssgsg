using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using static Cssgsg.Settings.Lang;

namespace Cssgsg.Settings.Pages;

sealed partial class JapanesePage : SettingsPage
{
    readonly UserDictionaryModel dictionary;
    readonly UserDictionaryView dictionaryView;
    readonly TextBlock engineVersion = new() { IsTextSelectionEnabled = true, HorizontalAlignment = HorizontalAlignment.Right };
    readonly TextBlock engineSource = new() { HorizontalAlignment = HorizontalAlignment.Right };
    readonly TextBlock pendingText = new() { TextWrapping = TextWrapping.Wrap };
    readonly Grid pendingRow;
    readonly TextBlock checkCaption = new() { TextWrapping = TextWrapping.Wrap };
    readonly Button checkButton;
    bool watching;

    public JapanesePage(SettingsModel model, UserDictionaryModel dictionary) : base(model, T("일본어", "Japanese", "日本語"))
    {
        this.dictionary = dictionary;
        var bothOff = Ui.Note(T("둘 다 끄면 한자로 변환하지 않습니다(가나만 입력).", "With both off, there is no kanji conversion (kana only).",
            "両方オフにすると漢字に変換しません（かなのみ）。"), warning: false);
        bothOff.Margin = new Thickness(0, 10, 0, 10);
        OnRefresh(() => bothOff.Visibility = !M.Bool("ja.convert_with_space") && !M.Bool("ja.convert_with_tab")
            ? Visibility.Visible : Visibility.Collapsed);
        Add(Ui.Section(T("변환 키", "Conversion Keys", "変換キー"),
            T("가나를 입력하는 중에 누르면 변환을 시작합니다. 끈 키는 입력한 가나를 그대로 확정하고, Space는 공백을 입력하고 Tab은 앱에 넘깁니다.",
                "Pressed while typing kana, these start conversion. A key that’s turned off confirms the kana as typed; Space then types a space and Tab goes to the app.",
                "かなの入力中に押すと変換を始めます。オフにしたキーは入力したかなをそのまま確定し、Space は空白を入力、Tab はアプリに渡します。"),
            Ui.Row(T("Space로 변환", "Convert with Space", "Space で変換"), Toggle("ja.convert_with_space")),
            Ui.Row(T("Tab으로 변환", "Convert with Tab", "Tab で変換"), Toggle("ja.convert_with_tab")),
            bothOff));

        var width = new RadioButtons { MaxColumns = 2 };
        width.Items.Add(T("반각", "Half-width", "半角"));
        width.Items.Add(T("전각", "Full-width", "全角"));
        OnRefresh(() => width.SelectedIndex = M.Bool("ja.full_width_space") ? 1 : 0);
        width.SelectionChanged += (_, _) =>
        {
            if (!Refreshing && width.SelectedIndex >= 0) M.Set("ja.full_width_space", width.SelectedIndex == 1);
        };
        Add(Ui.Section(T("구두점 및 기호", "Punctuation and Symbols", "句読点と記号"),
            T("공백 너비는 입력 중이 아닐 때 누른 Space에 적용됩니다. ¥ 기호를 끄면 \\ 키로 반각 \\를 입력합니다.",
                "Space width applies when you’re not typing kana. With the yen sign off, the \\ key types a half-width \\.",
                "スペース幅は、入力中でないときに押した Space に適用されます。円記号をオフにすると \\ キーで半角の \\ を入力します。"),
            Ui.Row(T("구두점", "Punctuation", "句読点"), Choice("ja.punctuation",
                ("japanese", T("일본식 (、。「」)", "Japanese (、。「」)", "和文（、。「」）")),
                ("full_width_western", T("전각 서양식 (，．［］)", "Full-width Western (，．［］)", "全角欧文（，．［］）")),
                ("half_width_western", T("반각 서양식 (,.[])", "Half-width Western (,.[])", "半角欧文（,.[]）")))),
            Ui.Row(T("공백 너비", "Space Width", "スペース幅"), width),
            Ui.Row(T("/ 키 → ・ (나카구로)", "/ Key → ・ (Nakaguro)", "/ キー → ・（中黒）"), Toggle("ja.slash_nakaguro")),
            Ui.Row(T("\\ 키 → ¥ (엔 기호)", "\\ Key → ¥ (Yen Sign)", "\\ キー → ¥（円記号）"), Toggle("ja.yen_sign"))));

        Add(Ui.Section(T("가타카나 (Caps Lock)", "Katakana (Caps Lock)", "カタカナ（Caps Lock）"), null,
            Ui.Row(T("입력하는 대로 바로 확정 (마지막 글자만 잠시 밑줄)", "Confirm as You Type (only the last kana stays underlined)",
                "入力したそばから確定（最後の一文字だけ下線）"), Toggle("ja.katakana_direct")),
            Ui.Row(T("일본어 모드를 나가면 Caps Lock 끄기", "Turn Off Caps Lock When Leaving Japanese",
                "日本語モードを抜けたら Caps Lock をオフ"), Toggle("ja.caps_katakana_auto_off"))));

        dictionaryView = new UserDictionaryView(dictionary);
        Add(Ui.Section(T("개인 사전", "User Dictionary", "ユーザー辞書"), T(
                "변환 후보에 단어를 추가합니다(Mozc 사용자 사전). 읽기는 히라가나로 입력하세요. 가타카나는 히라가나로 바뀌고, " +
                "작은 가나(ゃ·っ·ぁ 등)와 장음 부호 ー도 쓸 수 있습니다. 바꾸면 바로 적용되고, 두 번 클릭하면 편집합니다.",
                "Adds words to the conversion candidates (Mozc user dictionary). Enter readings in hiragana; katakana is converted, " +
                "and small kana (ゃ·っ·ぁ …) and the long-vowel mark ー work too. Changes apply immediately. Double-click to edit.",
                "変換候補に単語を追加します（Mozc のユーザー辞書）。読みはひらがなで入力してください。カタカナはひらがなに直り、" +
                "小さいかな（ゃ・っ・ぁ など）や長音符 ー も使えます。変更はすぐに反映され、ダブルクリックで編集できます。"),
            dictionaryView));

        var versionBox = new StackPanel { Spacing = 2, Children = { engineVersion, engineSource } };
        engineSource.Style = Ui.Style("CaptionTextBlockStyle");
        engineSource.Foreground = Ui.Brush("TextFillColorSecondaryBrush");
        var apply = Ui.Button(T("지금 적용", "Apply Now", "今すぐ適用"), () => _ = ApplyEngineAsync());
        ToolTipService.SetToolTip(apply, T("엔진 호스트가 곧바로 다시 시작하며 새 Mozc를 씁니다.", "The engine host restarts right away with the new Mozc.",
            "エンジンホストがすぐに再起動し、新しい Mozc を使います。"));
        pendingRow = Ui.Row(pendingText, apply);
        pendingRow.Visibility = Visibility.Collapsed;
        checkCaption.Style = Ui.Style("CaptionTextBlockStyle");
        checkCaption.Foreground = Ui.Brush("TextFillColorSecondaryBrush");
        checkButton = Ui.Button(T("지금 확인", "Check Now", "今すぐ確認"), () => _ = CheckEngineAsync());
        var updates = new TextBlock { Text = T("업데이트", "Updates", "アップデート") };
        var updatesLabel = new StackPanel { Spacing = 2, Children = { updates, checkCaption } };
        var updatesRow = new Grid { MinHeight = 52, Padding = new Thickness(0, 8, 0, 8), ColumnSpacing = 16 };
        updatesRow.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        updatesRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        updatesLabel.VerticalAlignment = VerticalAlignment.Center;
        checkButton.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(checkButton, 1);
        updatesRow.Children.Add(updatesLabel);
        updatesRow.Children.Add(checkButton);
        Add(Ui.Section(T("변환 엔진 (Mozc)", "Conversion Engine (Mozc)", "変換エンジン（Mozc）"),
            T("Mozc는 cssgsg와 따로 업데이트됩니다. 엔진 호스트가 하루에 한 번 GitHub에서 새 버전을 확인해 내려받고, 다음에 호스트가 시작될 때부터 씁니다.",
                "Mozc updates separately from cssgsg. Once a day the engine host checks GitHub for a newer one, downloads it, and uses it from its next start.",
                "Mozc は cssgsg とは別にアップデートされます。エンジンホストが1日に1回 GitHub で新しいバージョンを確認してダウンロードし、次に起動したときから使います。"),
            Ui.Row(T("버전", "Version", "バージョン"), versionBox),
            pendingRow,
            updatesRow));

        Add(Ui.Section(T("변환 학습", "Conversion History", "変換の学習"),
            T("Mozc가 기억한 변환(문절 나누기, 고른 후보)을 지웁니다. 개인 사전은 그대로 남습니다.",
                "Clears what Mozc learned (segmentation and chosen candidates). The user dictionary stays.",
                "Mozc が覚えた変換（文節の区切り、選んだ候補）を消去します。ユーザー辞書はそのまま残ります。"),
            Ui.Row(T("Mozc 변환 학습", "Mozc Conversion History", "Mozc の変換学習"),
                Ui.Button(T("지우기…", "Clear…", "消去…"), () => _ = ClearMozcAsync()))));

        Add(Ui.Section(T("변환 단축키", "Conversion Shortcuts", "変換ショートカット"), null,
            Ui.CheatSheet(T("입력 중", "While Typing", "入力中"), TypingRows),
            Ui.CheatSheet(T("변환 중", "While Converting", "変換中"), ConvertingRows)));
        Shown();
    }

    public override void Shown()
    {
        if (dictionary.ReloadIfChanged()) dictionaryView.Fill();
        _ = ShowEngineAsync();
    }

    /// 엔진 상태: 쓰는 엔진(내려받은 것인지), 받아 두고 기다리는 엔진, 마지막 확인(맥 MozcEngineSection).
    /// 호스트가 없으면 설치된 엔진의 MOZC_VERSION("<커밋> <날짜> <버전>").
    async Task ShowEngineAsync()
    {
        var (status, info) = await HostClient.EngineStatusAsync();
        var (installed, date) = InstalledEngine();
        checkButton.IsEnabled = status == HostClient.Status.Done && info?.Checking != true;
        if (status != HostClient.Status.Done || info == null)
        {
            engineVersion.Text = installed != null ? $"{installed} ({date})" : "—";
            engineSource.Text = status == HostClient.Status.NoHost
                ? T("설치된 엔진 (엔진 호스트가 실행 중이 아님)", "Installed engine (the engine host isn’t running)",
                    "インストール済みのエンジン（エンジンホストが起動していません）")
                : T("설치된 엔진", "Installed engine", "インストール済みのエンジン");
            pendingRow.Visibility = Visibility.Collapsed;
            checkCaption.Text = status == HostClient.Status.NoHost
                ? T("엔진 호스트가 실행 중이 아닙니다", "The engine host isn’t running", "エンジンホストが起動していません")
                : "";
            return;
        }
        if (info.Active is { } active)
        {
            engineVersion.Text = $"{active.Version} ({active.Date})";
            engineSource.Text = active.Downloaded
                ? T("내려받은 엔진", "Downloaded engine", "ダウンロードしたエンジン")
                : T("설치본에 든 엔진", "Engine that came with cssgsg", "cssgsg に含まれるエンジン");
        }
        else
        {
            engineVersion.Text = installed != null ? $"{installed} ({date})" : "—";
            engineSource.Text = T("엔진을 읽지 못했습니다 (가나만 입력)", "The engine couldn’t be loaded (kana only)",
                "エンジンを読み込めませんでした（かなのみ）");
        }
        if (info.Pending is { } pending)
        {
            pendingText.Text = T($"새 Mozc {pending.Version} ({pending.Date})를 받아 두었습니다. 엔진 호스트가 다시 시작하면 적용됩니다.",
                $"New Mozc {pending.Version} ({pending.Date}) is downloaded and applies when the engine host restarts.",
                $"新しい Mozc {pending.Version}（{pending.Date}）をダウンロード済みです。エンジンホストの再起動時に適用されます。");
            pendingRow.Visibility = Visibility.Visible;
        }
        else
        {
            pendingRow.Visibility = Visibility.Collapsed;
        }
        static string When(long seconds) => DateTimeOffset.FromUnixTimeSeconds(seconds).ToLocalTime().ToString("g");
        checkCaption.Text = info.Checking
            ? T("확인 중…", "Checking…", "確認中…")
            : info.FailedAt is { } failed
                ? T("마지막 확인 실패: ", "Last check failed: ", "最終確認に失敗: ") + When(failed) + (info.Failure != null ? $" ({info.Failure})" : "")
                : info.CheckedAt is { } checkedAt
                    ? T("마지막 확인: ", "Last checked: ", "最終確認: ") + When(checkedAt)
                    : "";
    }

    /// "지금 확인": 호스트가 뒤에서 확인하는 동안 상태를 1초마다 다시 읽는다(2분까지).
    async Task CheckEngineAsync()
    {
        if (watching) return;
        watching = true;
        checkButton.IsEnabled = false;
        try
        {
            if (!(await HostClient.CheckEngineAsync()).Ok) return;
            for (var i = 0; i < 120; i++)
            {
                await Task.Delay(1000);
                await ShowEngineAsync();
                var (_, info) = await HostClient.EngineStatusAsync();
                if (info?.Checking != true) break;
            }
        }
        finally
        {
            watching = false;
            await ShowEngineAsync();
        }
    }

    /// "지금 적용": 엔진 호스트를 다시 띄운다. 뜰 때 받아 둔 새 엔진을 읽는다.
    async Task ApplyEngineAsync()
    {
        pendingRow.Visibility = Visibility.Collapsed;
        engineSource.Text = T("다시 시작하는 중…", "Restarting…", "再起動中…");
        await HostClient.RestartAsync();
        for (var i = 0; i < 30 && await HostClient.EngineVersionAsync() == null; i++) await Task.Delay(200);
        await ShowEngineAsync();
    }

    static (string? Version, string? Date) InstalledEngine()
    {
        try
        {
            var parts = File.ReadAllText(Paths.MozcVersion).Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
            if (parts.Length >= 3) return (parts[2], parts[1]);
        }
        catch (Exception) { }
        return (null, null);
    }

    /// Mozc 학습 파일을 지운다. 엔진이 파일을 열어 두고 있어서 호스트에 시킨다(엔진을 내리고 지운 뒤 다시 읽는다).
    async Task ClearMozcAsync()
    {
        if (!await Ui.Confirm(XamlRoot, T("Mozc 변환 학습을 지우시겠습니까?", "Clear Mozc’s conversion history?", "Mozc の変換学習を消去しますか？"),
                T("지우기", "Clear", "消去")))
            return;
        var reply = await HostClient.ClearMozcLearningAsync();
        string? problem = null;
        if (reply.Status == HostClient.Status.NoHost)
        {
            foreach (var name in new[] { "segment.db", "boundary.db", "cform.db", ".history.db" })
            {
                try { File.Delete(Path.Combine(Paths.MozcProfile, name)); }
                catch (Exception e) { problem = e.Message; }
            }
        }
        else if (!reply.Ok)
        {
            problem = reply.Message ?? reply.Status.ToString();
        }
        if (problem != null)
            await Ui.Dialog(XamlRoot, T("지우지 못했습니다", "Couldn’t clear", "消去できませんでした"), problem, "", T("닫기", "Close", "閉じる")).ShowAsync();
    }

    static (string[], string)[] TypingRows =>
    [
        (["Space", "Tab"], T("변환 시작 (위 설정에 따름)", "Start conversion (per the settings above)", "変換開始（上の設定に従う）")),
        (["Enter"], T("입력한 그대로 확정", "Confirm as typed", "入力どおりに確定")),
        (["Shift + Enter"], T("확정하고 줄바꿈", "Confirm and start a new line", "確定して改行")),
        (["Backspace"], T("한 글자 지우기", "Delete one character", "1文字削除")),
        (["Esc"], T("입력 취소", "Cancel input", "入力を取り消し")),
        (["Caps Lock"], T("가타카나 입력", "Type katakana", "カタカナ入力")),
        (["Shift"], T("A–Z 자리에서는 무시(로마자 입력 없음), 그 밖의 자리에서는 Shift 기호 입력",
            "Ignored on the A–Z keys (no romaji input); other keys type their shifted symbol",
            "A–Z の位置では無視（ローマ字入力なし）、それ以外の位置では Shift の記号を入力")),
    ];

    static (string[], string)[] ConvertingRows =>
    [
        (["Space", "↓"], T("다음 후보", "Next candidate", "次の候補")),
        (["↑"], T("이전 후보", "Previous candidate", "前の候補")),
        (["Tab"], T("후보 펼치기 / 접기", "Expand / collapse candidates", "候補の展開 / 折りたたみ")),
        (["←", "→"], T("문절 이동 (문절이 하나면 후보 페이지 넘김, 펼친 목록에서는 한 칸 이동)",
            "Move between segments (pages the list when there’s one segment; one cell when expanded)",
            "文節間を移動（文節が1つなら候補のページ送り、展開中は1マス移動）")),
        (["Shift + ←", "Shift + →"], T("문절 길이 조절", "Resize segment", "文節の長さを変更")),
        (["Page Up", "Page Down"], T("후보 페이지 넘김", "Page through candidates", "候補のページ送り")),
        (["1–9"], T("번호로 후보 확정", "Pick a candidate by number", "番号で候補を確定")),
        (["Enter"], T("변환 확정", "Confirm conversion", "変換を確定")),
        (["Shift + Enter"], T("확정하고 줄바꿈", "Confirm and start a new line", "確定して改行")),
        (["Esc", "Backspace"], T("변환 취소 (입력으로 돌아감)", "Cancel conversion (back to input)", "変換をキャンセル（入力に戻る）")),
        ([T("다른 키", "Other keys", "その他のキー")], T("확정하고 이어서 입력", "Confirm and keep typing", "確定して入力を続ける")),
    ];
}
