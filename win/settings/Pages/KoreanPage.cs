using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using static Cssgsg.Settings.Lang;

namespace Cssgsg.Settings.Pages;

sealed partial class KoreanPage : SettingsPage
{
    readonly TextBlock count = new() { VerticalAlignment = VerticalAlignment.Center };
    readonly Button clear;

    public KoreanPage(SettingsModel model) : base(model, T("한국어", "Korean", "韓国語"))
    {
        (string Value, string Label)[] layouts =
        [
            ("chamshin-v18", T("참신세벌식 v18 (기본형)", "Chamshin Sebeolsik v18 (standard)", "チャムシン3ボル式 v18（基本形）")),
            ("chamshin-d-v19", T("참신세벌식 D v19", "Chamshin Sebeolsik D v19", "チャムシン3ボル式 D v19")),
        ];
        var radios = new RadioButtons();
        foreach (var (_, label) in layouts) radios.Items.Add(label);
        OnRefresh(() => radios.SelectedIndex = Math.Max(0, Array.FindIndex(layouts, l => l.Value == M.Str("ko_layout"))));
        radios.SelectionChanged += (_, _) =>
        {
            if (!Refreshing && radios.SelectedIndex >= 0) M.Set("ko_layout", layouts[radios.SelectedIndex].Value);
        };
        Add(Ui.Section(T("배열", "Layout", "配列"),
            T("D는 숫자 줄에도 글자가 있습니다. 두 배열은 배열 학습 탭에서 볼 수 있습니다.",
                "D also puts letters on the number row. Both layouts are shown in the Layouts tab.",
                "D は数字の段にも文字があります。二つの配列は「配列の学習」タブで見られます。"),
            Ui.Block(radios)));

        var hanjaKey = new TextBlock();
        OnRefresh(() => hanjaKey.Text = ShortcutText.Label(M.Str("shortcuts.hanja")));
        clear = Ui.Button(T("지우기…", "Clear…", "消去…"), () => _ = ClearAsync());
        var learned = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12, Children = { count, clear } };
        Add(Ui.Section(T("한자", "Hanja", "ハンジャ（漢字）"), T(
                "조합 중인 글자에서 한자 단축키를 누르면 한자로 바꿉니다(자음 하나면 기호). 단축키는 일반 탭에서 바꿀 수 있습니다.\n" +
                "변환 중에는 Space·↓로 다음 후보, ↑로 이전 후보, Tab으로 후보 펼치기, 1–9로 번호 확정, Enter로 확정, " +
                "Esc·Backspace로 원래대로 돌아갑니다. 고른 한자는 기억해 두었다가 다음에 먼저 보여 줍니다.",
                "Press the Hanja shortcut on the syllable being composed to convert it (a lone consonant opens symbols). You can " +
                "change the shortcut in the General tab.\n" +
                "While converting: Space/↓ next candidate, ↑ previous, Tab expands the list, 1–9 picks by number, Enter confirms, " +
                "Esc/Backspace reverts. Your choices are remembered and shown first next time.",
                "入力中の文字でハンジャのショートカットを押すと漢字に変換します（子音だけなら記号）。ショートカットは「一般」タブで" +
                "変更できます。\n" +
                "変換中は Space・↓ で次の候補、↑ で前の候補、Tab で候補を展開、1–9 で番号を選んで確定、Enter で確定、" +
                "Esc・Backspace で元に戻ります。選んだ漢字は記憶して、次から先に表示します。"),
            Ui.Row(T("한자 단축키", "Hanja Shortcut", "ハンジャのショートカット"), hanjaKey),
            Ui.Row(T("고른 한자 기억", "Remembered Choices", "選んだ漢字の記憶"), learned)));
        Shown();
    }

    public override void Shown()
    {
        var n = SettingsModel.HanjaLearningCount();
        count.Text = T($"{n}개", $"{n}", $"{n} 件");
        clear.IsEnabled = n > 0;
    }

    /// 고른 한자 기억을 지운다. 기억은 엔진 호스트가 들고 있다가 저장하므로 호스트에 시킨다(떠 있지 않으면 파일만 비운다).
    async Task ClearAsync()
    {
        if (!await Ui.Confirm(XamlRoot, T("고른 한자 기억을 모두 지우시겠습니까?", "Clear all remembered Hanja choices?",
                "選んだ漢字の記憶をすべて消去しますか？"), T("지우기", "Clear", "消去")))
            return;
        var reply = await HostClient.ClearHanjaLearningAsync();
        if (reply.Status == HostClient.Status.NoHost)
        {
            try { File.Delete(Paths.HanjaLearning); } catch (Exception) { }
        }
        else if (!reply.Ok)
        {
            await Ui.Dialog(XamlRoot, T("지우지 못했습니다", "Couldn’t clear", "消去できませんでした"),
                reply.Message ?? reply.Status.ToString(), "", T("닫기", "Close", "閉じる")).ShowAsync();
        }
        Shown();
    }
}
