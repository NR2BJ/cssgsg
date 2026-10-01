using Microsoft.UI.Dispatching;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using WinRT;
using static Cssgsg.Settings.Lang;

namespace Cssgsg.Settings;

/// 설정 화면 조각(윈도우 설정 앱 모양: 제목, 카드 안의 줄, 아래 설명). 맥의 Form(.grouped)·Section에 해당한다.
static class Ui
{
    public static Brush Brush(string key) => Resource<Brush>(key);
    public static Style Style(string key) => Resource<Style>(key);

    /// 테마 리소스. AOT에서는 코드가 직접 만들지 않은 형식(Style 등)의 투영이 다듬어져 나가서 리소스가
    /// DependencyObject로 감싸져 온다: 그때는 원하는 형식으로 다시 감싼다(CsWinRT As).
    static T Resource<T>(string key) where T : class
    {
        Application.Current.Resources.TryGetValue(key, out var value);
        return value as T ?? value?.As<T>() ?? throw new InvalidOperationException($"no resource {key}");
    }

    public static TextBlock Text(string text, string? style = null, bool wrap = true)
    {
        var t = new TextBlock { Text = text, TextWrapping = wrap ? TextWrapping.Wrap : TextWrapping.NoWrap };
        if (style != null) t.Style = Style(style);
        return t;
    }

    public static TextBlock Secondary(string text)
    {
        var t = Text(text, "CaptionTextBlockStyle");
        t.Foreground = Brush("TextFillColorSecondaryBrush");
        return t;
    }

    /// 경고(주황) 또는 알림(흐림) 한 줄.
    public static StackPanel Note(string text, bool warning)
    {
        var icon = new FontIcon
        {
            Glyph = warning ? "" : "",
            FontSize = 14,
            Foreground = warning ? Brush("SystemFillColorCautionBrush") : Brush("TextFillColorSecondaryBrush"),
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(0, 2, 0, 0),
        };
        var t = Text(text);
        if (!warning) t.Foreground = Brush("TextFillColorSecondaryBrush");
        return new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Children = { icon, t } };
    }

    /// 제목 + 카드(줄들) + 설명.
    public static StackPanel Section(string header, string? footer, params UIElement[] rows)
    {
        var panel = new StackPanel { Spacing = 6, Margin = new Thickness(0, 0, 0, 22) };
        panel.Children.Add(new TextBlock { Text = header, Style = Style("BodyStrongTextBlockStyle"), Margin = new Thickness(2, 0, 0, 2) });
        panel.Children.Add(Card(rows));
        if (footer != null)
        {
            var f = Secondary(footer);
            f.Margin = new Thickness(2, 2, 2, 0);
            f.IsTextSelectionEnabled = true;
            panel.Children.Add(f);
        }
        return panel;
    }

    public static Border Card(params UIElement[] rows)
    {
        var stack = new StackPanel();
        for (var i = 0; i < rows.Length; i++)
        {
            if (i > 0) stack.Children.Add(new Border { Height = 1, Background = Brush("DividerStrokeColorDefaultBrush") });
            stack.Children.Add(rows[i]);
        }
        return new Border
        {
            Background = Brush("CardBackgroundFillColorDefaultBrush"),
            BorderBrush = Brush("CardStrokeColorDefaultBrush"),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(6),
            Padding = new Thickness(16, 2, 16, 2),
            Child = stack,
        };
    }

    /// 이름(왼쪽)과 조절기(오른쪽) 한 줄. `caption`은 이름 아래 작은 글자.
    public static Grid Row(string label, UIElement? control, string? caption = null) => Row(Text(label), control, caption);

    public static Grid Row(TextBlock label, UIElement? control, string? caption = null)
    {
        var grid = new Grid { MinHeight = 52, Padding = new Thickness(0, 8, 0, 8), ColumnSpacing = 16 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var left = new StackPanel { VerticalAlignment = VerticalAlignment.Center, Spacing = 2 };
        left.Children.Add(label);
        if (caption != null) left.Children.Add(Secondary(caption));
        grid.Children.Add(left);
        Name(control, label.Text);
        if (control is FrameworkElement fe)
        {
            fe.VerticalAlignment = VerticalAlignment.Center;
            fe.HorizontalAlignment = HorizontalAlignment.Right;
            Grid.SetColumn(fe, 1);
            grid.Children.Add(fe);
        }
        return grid;
    }

    /// 줄 이름을 글자 없는 조절기(스위치, 슬라이더, 고르기)의 접근성 이름으로 붙인다(화면 읽기 프로그램, UI 자동화).
    /// 묶음이면 안의 첫 조절기에. 글자가 있는 단추는 그 글자가 이름이다.
    static void Name(UIElement? control, string name)
    {
        static bool Unnamed(UIElement e) => e is ToggleSwitch or Slider or ComboBox or RadioButtons;
        var target = control switch
        {
            Panel panel => panel.Children.FirstOrDefault(Unnamed),
            { } c when Unnamed(c) => c,
            _ => null,
        };
        if (target != null && string.IsNullOrEmpty(AutomationProperties.GetName(target)))
            AutomationProperties.SetName(target, name);
    }

    /// 카드 안에 줄 대신 넣는 덩어리(목록, 긴 글).
    public static Border Block(UIElement content) => new() { Padding = new Thickness(0, 10, 0, 10), Child = content };

    public static Button Button(string text, Action click, bool accent = false)
    {
        var b = new Button { Content = text };
        if (accent) b.Style = Style("AccentButtonStyle");
        b.Click += (_, _) => click();
        return b;
    }

    public static HyperlinkButton Link(string text, string uri)
    {
        var b = new HyperlinkButton { Content = text, Padding = new Thickness(4, 2, 4, 2) };
        b.Click += (_, _) => Shell.Open(uri);
        return b;
    }

    /// 키 모양 글자(설명서).
    public static Border KeyCap(string text) => new()
    {
        Background = Brush("ControlFillColorSecondaryBrush"),
        BorderBrush = Brush("ControlStrokeColorDefaultBrush"),
        BorderThickness = new Thickness(1),
        CornerRadius = new CornerRadius(4),
        Padding = new Thickness(6, 1, 6, 2),
        Child = new TextBlock { Text = text, FontWeight = FontWeights.SemiBold, FontSize = 13 },
    };

    /// 키와 설명 표.
    public static StackPanel CheatSheet(string title, (string[] Keys, string Text)[] rows)
    {
        var grid = new Grid { ColumnSpacing = 14, RowSpacing = 6 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        for (var i = 0; i < rows.Length; i++)
        {
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var keys = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4, HorizontalAlignment = HorizontalAlignment.Right };
            foreach (var k in rows[i].Keys) keys.Children.Add(KeyCap(k));
            Grid.SetRow(keys, i);
            grid.Children.Add(keys);
            var t = Text(rows[i].Text);
            t.Foreground = Brush("TextFillColorSecondaryBrush");
            t.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetRow(t, i);
            Grid.SetColumn(t, 1);
            grid.Children.Add(t);
        }
        var panel = new StackPanel { Spacing = 8, Padding = new Thickness(0, 10, 0, 10) };
        panel.Children.Add(Text(title, "BodyStrongTextBlockStyle"));
        panel.Children.Add(grid);
        return panel;
    }

    public static ContentDialog Dialog(XamlRoot root, string title, object content, string primary, string close)
    {
        return new ContentDialog
        {
            XamlRoot = root,
            Title = title,
            Content = content,
            PrimaryButtonText = primary,
            CloseButtonText = close,
            DefaultButton = ContentDialogButton.Close,
            Language = Tag,
        };
    }

    /// 확인을 받는다(지우기 등). 고르면 true.
    public static async Task<bool> Confirm(XamlRoot root, string question, string action)
    {
        var d = Dialog(root, question, "", action, T("취소", "Cancel", "キャンセル"));
        d.Content = null;
        return await d.ShowAsync() == ContentDialogResult.Primary;
    }
}

/// 설정 탭 하나. 조절기마다 "모델 → 화면" 맞추기를 등록해 두고, 모델이 바뀌면(파일을 다시 읽음, 고쳐 씀) 모두 다시 맞춘다.
/// 탭을 새로 만들지 않으니 스크롤과 녹화 중인 단추가 그대로다. 맞추는 동안 생긴 이벤트는 모델에 다시 쓰지 않는다.
abstract partial class SettingsPage : UserControl
{
    protected readonly SettingsModel M;
    readonly List<Action> refreshers = [];
    protected bool Refreshing { get; private set; }
    readonly StackPanel body = new() { MaxWidth = 860, Padding = new Thickness(28, 20, 28, 28) };

    protected SettingsPage(SettingsModel model, string title)
    {
        M = model;
        body.Children.Add(new TextBlock { Text = title, Style = Ui.Style("TitleTextBlockStyle"), Margin = new Thickness(2, 0, 0, 18) });
        Content = new ScrollViewer { Content = body, HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled };
    }

    protected void Add(UIElement e) => body.Children.Add(e);

    public void OnRefresh(Action refresh)
    {
        refreshers.Add(refresh);
        Quietly(refresh);
    }

    /// 조절기 값을 코드로 맞춘다(그 이벤트로 모델에 다시 쓰지 않게).
    protected void Quietly(Action set)
    {
        var was = Refreshing;
        Refreshing = true;
        try { set(); } finally { Refreshing = was; }
    }

    public virtual void Refresh()
    {
        Refreshing = true;
        try
        {
            foreach (var r in refreshers) r();
        }
        finally
        {
            Refreshing = false;
        }
    }

    /// 탭이 보일 때(상태를 새로 읽을 것이 있으면).
    public virtual void Shown() { }

    protected ToggleSwitch Toggle(string path, Func<bool, bool>? map = null)
    {
        var t = new ToggleSwitch { OnContent = T("켬", "On", "オン"), OffContent = T("끔", "Off", "オフ"), MinWidth = 0 };
        OnRefresh(() => t.IsOn = M.Bool(path));
        t.Toggled += (_, _) =>
        {
            if (!Refreshing) M.Set(path, t.IsOn);
        };
        return t;
    }

    protected ComboBox Choice(string path, params (string Value, string Label)[] options)
    {
        var c = new ComboBox { MinWidth = 220 };
        foreach (var (_, label) in options) c.Items.Add(label);
        OnRefresh(() => c.SelectedIndex = Math.Max(0, Array.FindIndex(options, o => o.Value == M.Str(path))));
        c.SelectionChanged += (_, _) =>
        {
            if (!Refreshing && c.SelectedIndex >= 0) M.Set(path, options[c.SelectedIndex].Value);
        };
        return c;
    }

    /// 수 슬라이더. 끄는 동안은 숫자만 바꾸고, 멈춘 뒤에 한 번 쓴다(파일을 여러 번 쓰지 않게).
    protected StackPanel ValueSlider(string path, int min, int max, int step, string unit, string? zeroLabel = null)
    {
        var slider = new Slider
        {
            Minimum = min,
            Maximum = max,
            StepFrequency = step,
            SmallChange = step,
            LargeChange = step * 5,
            SnapsTo = SliderSnapsTo.StepValues,
            Width = 220,
            IsThumbToolTipEnabled = false,
        };
        var label = new TextBlock { MinWidth = 64, TextAlignment = TextAlignment.Right, VerticalAlignment = VerticalAlignment.Center };
        string Label(int v) => v == 0 && zeroLabel != null ? zeroLabel : $"{v}{unit}";
        var timer = DispatcherQueue.GetForCurrentThread().CreateTimer();
        timer.Interval = TimeSpan.FromMilliseconds(400);
        timer.IsRepeating = false;
        timer.Tick += (_, _) => M.Set(path, (int)Math.Round(slider.Value));
        OnRefresh(() =>
        {
            timer.Stop();
            slider.Value = M.Int(path);
            label.Text = Label(M.Int(path));
        });
        slider.ValueChanged += (_, e) =>
        {
            label.Text = Label((int)Math.Round(e.NewValue));
            if (Refreshing) return;
            timer.Stop();
            timer.Start();
        };
        return new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Children = { slider, label } };
    }
}
