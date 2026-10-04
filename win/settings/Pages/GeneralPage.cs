using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using static Cssgsg.Settings.Lang;

namespace Cssgsg.Settings.Pages;

// 화면 문구는 맥 설정 앱처럼 쓴다: 이름은 명사형("탭 인식 시간"), 설명은 합니다체.
// 맥의 "Shift+Enter 줄바꿈 대기"는 없다: 윈도우는 조합을 동기 편집으로 먼저 확정하고 원래 키를 넘겨서 기다릴 일이 없다.
sealed partial class GeneralPage : SettingsPage
{
    public GeneralPage(SettingsModel model, ShortcutRecorder recorder) : base(model, T("일반", "General", "一般"))
    {
        var rows = new List<UIElement>();
        foreach (var field in SettingsModel.ShortcutFields)
            rows.Add(Ui.Row(ShortcutText.ActionName(field), new ShortcutEditor(model, recorder, field, this)));
        var noEnglish = Ui.Note(T(
            "영어로 전환하는 단축키가 없습니다. 작업 표시줄의 모드 아이콘(G/ㅊ/月) 메뉴에서도 고를 수 있지만, 치면서 바꾸려면 하나는 정해 두세요.",
            "No shortcut switches to English. You can still pick a mode from the mode icon (G/ㅊ/月) menu on the taskbar, but set one to switch while typing.",
            "英語に切り替えるショートカットがありません。タスクバーのモードアイコン（G/ㅊ/月）のメニューからも選べますが、入力しながら切り替えるには一つ設定してください。"), warning: true);
        noEnglish.Margin = new Thickness(0, 10, 0, 10);
        OnRefresh(() => noEnglish.Visibility = M.Str("shortcuts.toggle_english").Length == 0 ? Visibility.Visible : Visibility.Collapsed);
        rows.Add(noEnglish);
        Add(Ui.Section(T("단축키", "Shortcuts", "ショートカット"), T(
            "단추를 누른 뒤 원하는 키를 누르세요. 수식키만 짧게 눌렀다 떼면 탭으로, 수식키와 다른 키를 함께 누르면 조합으로 " +
            "기록됩니다. 수식키는 왼쪽과 오른쪽을 구분합니다(왼쪽 Ctrl + Space와 오른쪽 Ctrl + Space는 다른 단축키입니다). " +
            "Alt와 Windows 키는 쓸 수 없고(Alt는 앱 메뉴가, Windows 키는 시작 메뉴가 먼저 가져갑니다), Esc를 누르면 취소됩니다.\n" +
            "영어 ↔ 비영어는 영어와 직전에 쓴 언어(한국어·일본어) 사이를 오갑니다. 한국어 ↔ 일본어를 영어 모드에서 누르면 " +
            "둘 중 직전에 쓰지 않은 쪽으로 전환합니다. 한국 키보드의 한자 키는 오른쪽 Ctrl 자리입니다.",
            "Click a button, then press the keys you want. A single modifier pressed briefly is recorded as a tap; a modifier " +
            "pressed with another key is recorded as a combination. Left and right modifiers are distinct (Left Ctrl + Space and " +
            "Right Ctrl + Space are different shortcuts). Alt and the Windows key can’t be used (Alt goes to the app’s menu and " +
            "the Windows key to the Start menu). Press Esc to cancel.\n" +
            "English ↔ Non-English switches between English and the language you used last (Korean or Japanese). " +
            "Korean ↔ Japanese, pressed in English mode, switches to whichever of the two you didn’t use last. " +
            "The Hanja key of Korean keyboards is the Right Ctrl position.",
            "ボタンを押してから、使いたいキーを押してください。修飾キーだけを短く押して離すとタップ、修飾キーと別のキーを一緒に押すと" +
            "組み合わせとして記録されます。修飾キーは左右を区別します（左Ctrl + Space と右Ctrl + Space は別のショートカットです）。" +
            "Alt と Windows キーは使えません（Alt はアプリのメニューが、Windows キーはスタートメニューが先に受け取ります）。Esc で取り消します。\n" +
            "英語 ↔ 英語以外は、英語と直前に使った言語（韓国語・日本語）を行き来します。韓国語 ↔ 日本語を英語モードで押すと、" +
            "二つのうち直前に使っていない方に切り替わります。韓国語キーボードのハンジャキーは右Ctrlの位置です。"),
            [.. rows]));

        Add(Ui.Section(T("수식키 탭", "Modifier Taps", "修飾キーのタップ"), T(
                "수식키를 이 시간보다 오래 누르고 있으면 탭으로 보지 않습니다.\n" +
                "빠른 탭 전환 보정: Shift를 탭하고 곧바로 글자를 쳐서 글자가 Shift보다 먼저 눌려도 언어를 바꿉니다. Shift가 " +
                "아무 뜻이 없는 키(新月의 글자 키)는 글자를 누르고 80ms 안에 Shift를 떼면 전환하고, Shift가 글자를 바꾸는 키" +
                "(영어 대문자, 참신세벌식의 Shift 기호)는 30ms 안에 뗐을 때만 전환합니다. 한글 조합 중(낱말 가운데)에는 보정하지 않습니다.",
                "Holding a modifier longer than this doesn’t count as a tap.\n" +
                "Fast tap-switch correction switches languages even when you tap Shift and type so quickly that the letter " +
                "goes down before Shift comes up. On keys where Shift means nothing (Shingetsu letter keys) it switches when " +
                "Shift comes up within 80 ms of the letter; on keys where Shift changes the letter (English capitals, " +
                "Chamshin Shift symbols) only within 30 ms. It never applies in the middle of a Korean syllable.",
                "修飾キーをこの時間より長く押していると、タップとみなしません。\n" +
                "高速タップ切替補正：Shiftをタップしてすぐに文字を打ち、文字がShiftより先に押されても言語を切り替えます。" +
                "Shiftに意味のないキー（新月の文字キー）は文字を押してから80ms以内にShiftを離すと切り替え、Shiftが文字を" +
                "変えるキー（英語の大文字、チャムシン3ボル式のShift記号）は30ms以内に離したときだけ切り替えます。" +
                "ハングル入力中（単語の途中）には補正しません。"),
            Ui.Row(T("탭 인식 시간", "Tap Recognition Time", "タップ判定時間"), ValueSlider("tap_threshold_ms", 100, 500, 10, "ms")),
            Ui.Row(T("빠른 탭 전환 보정 (실험적)", "Fast Tap-Switch Correction (Experimental)", "高速タップ切替補正（実験的）"),
                Toggle("tap_buffering"))));

        Add(Ui.Section(T("후보창", "Candidate Window", "候補ウィンドウ"),
            T("화면 배율 100%일 때의 픽셀입니다. 배율을 높이면 같이 커집니다.",
                "Pixels at 100% display scaling; it grows with the scaling.",
                "表示スケール100%のときのピクセルです。スケールを上げると一緒に大きくなります。"),
            Ui.Row(T("글자 크기", "Font Size", "文字サイズ"), ValueSlider("windows.candidate_font_size", 10, 28, 1, "px"))));

        var position = Choice("windows.hud_position",
            ("caret", T("입력 커서 위 (모르면 마우스 옆)", "Above the text cursor (next to the mouse if unknown)",
                "入力カーソルの上（分からなければマウスの横）")),
            ("mouse", T("마우스 포인터 옆", "Next to the mouse pointer", "マウスポインタの横")));
        OnRefresh(() => position.IsEnabled = M.Bool("windows.hud"));
        Add(Ui.Section(T("모드 표시", "Mode Indicator", "モード表示"), null,
            Ui.Row(T("모드 전환 시 G · ㅊ · 月 표시", "Show G · ㅊ · 月 When Switching Modes", "モード切り替え時に G · ㅊ · 月 を表示"),
                Toggle("windows.hud")),
            Ui.Row(T("위치", "Position", "位置"), position)));
    }
}

/// 단축키 한 줄의 조절기: 지금 단축키(누르면 녹화), 지우기, 기본값, 받아 주지 않은 까닭.
sealed partial class ShortcutEditor : StackPanel
{
    public ShortcutEditor(SettingsModel model, ShortcutRecorder recorder, string field, SettingsPage page)
    {
        Spacing = 4;
        var path = $"shortcuts.{field}";
        var defaultValue = SettingsModel.DefaultStr(path);
        var record = new Button { MinWidth = 220 };
        var clear = new Button { Content = new FontIcon { Glyph = "", FontSize = 12 }, Padding = new Thickness(8, 6, 8, 6) };
        var clearName = T("단축키 지우기", "Clear shortcut", "ショートカットを消去");
        ToolTipService.SetToolTip(clear, clearName);
        AutomationProperties.SetName(clear, clearName);
        var reset = new Button { Content = new FontIcon { Glyph = "", FontSize = 12 }, Padding = new Thickness(8, 6, 8, 6) };
        var resetName = T($"기본값으로 되돌리기: {ShortcutText.Label(defaultValue)}",
            $"Reset to default: {ShortcutText.Label(defaultValue)}", $"既定に戻す: {ShortcutText.Label(defaultValue)}");
        ToolTipService.SetToolTip(reset, resetName);
        AutomationProperties.SetName(reset, resetName);
        var problem = new TextBlock
        {
            Foreground = Ui.Brush("SystemFillColorCautionBrush"),
            TextWrapping = TextWrapping.Wrap,
            MaxWidth = 320,
            Visibility = Visibility.Collapsed,
            HorizontalAlignment = HorizontalAlignment.Right,
        };
        var mine = false;
        void Show(string? reason)
        {
            problem.Text = reason ?? "";
            problem.Visibility = reason == null ? Visibility.Collapsed : Visibility.Visible;
        }
        void Paint()
        {
            var value = model.Str(path);
            record.Content = mine && recorder.Active
                ? T("키를 누르세요… (Esc: 취소)", "Press keys… (Esc: cancel)", "キーを押してください…（Esc：取消）")
                : ShortcutText.Label(value);
            clear.IsEnabled = value.Length > 0;
            reset.IsEnabled = value != defaultValue;
        }
        void OnActiveChanged()
        {
            if (!recorder.Active) mine = false;
            Paint();
        }
        Action onActiveChanged = OnActiveChanged;
        recorder.ActiveChanged += onActiveChanged;
        record.Click += (_, _) =>
        {
            Show(null);
            if (mine && recorder.Active)
            {
                recorder.Stop();
                return;
            }
            recorder.Start(recorded =>
            {
                if (recorded != null) Show(model.SetShortcut(field, recorded));
            }, Show);
            mine = true;
            Paint();
        };
        clear.Click += (_, _) =>
        {
            recorder.Stop();
            Show(model.SetShortcut(field, ""));
        };
        reset.Click += (_, _) =>
        {
            recorder.Stop();
            Show(model.SetShortcut(field, defaultValue));
        };
        // 화면 언어를 바꾸면 탭을 새로 만든다: 옛 조절기는 녹화를 거두고 알림을 끊는다.
        Unloaded += (_, _) =>
        {
            if (mine) recorder.Stop();
            recorder.ActiveChanged -= onActiveChanged;
        };
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, HorizontalAlignment = HorizontalAlignment.Right };
        buttons.Children.Add(record);
        buttons.Children.Add(clear);
        buttons.Children.Add(reset);
        Children.Add(buttons);
        Children.Add(problem);
        page.OnRefresh(Paint);
    }
}
