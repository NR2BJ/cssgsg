using Cssgsg.Settings.Pages;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;
using static Cssgsg.Settings.Lang;

namespace Cssgsg.Settings;

/// 설정 창(맥 SettingsView): 위에 경고 띠, 왼쪽에 탭 목록. 탭은 한 번 만들어 두고 보이기만 바꾼다(스크롤·웹 페이지가
/// 그대로다). 화면 언어를 바꾸면 웹 페이지 말고 모두 새로 만든다.
public sealed partial class MainWindow : Window
{
    enum Tab { General, Korean, Japanese, Learn, Practice, About }

    /// `--tab` 이름 → 탭. 모르는 이름이면 null.
    static Tab? TabNamed(string? name) => name switch
    {
        "general" => Tab.General,
        "korean" => Tab.Korean,
        "japanese" => Tab.Japanese,
        "learn" => Tab.Learn,
        "practice" => Tab.Practice,
        "about" => Tab.About,
        _ => null,
    };

    /// 이미 떠 있는 설정 앱에 탭을 보이라고 보내는 메시지(맥의 분산 알림 show-tab). wParam = 탭 번호 + 1.
    /// 두 번째로 띄운 설정 앱(작업 표시줄 아이콘 메뉴의 "배열 학습" 등)이 보내고 끝난다.
    public static readonly uint ShowTabMessage = User32.RegisterWindowMessageW("cssgsg-settings-show-tab");

    public static IntPtr ShowTabParam(string? name) => TabNamed(name) is { } tab ? (IntPtr)((int)tab + 1) : IntPtr.Zero;

    static MainWindow? shown;

    [System.Runtime.InteropServices.UnmanagedCallersOnly]
    static IntPtr WindowProc(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam, UIntPtr id, UIntPtr data)
    {
        if (msg == ShowTabMessage && shown is { } window && (int)wParam is var n and >= 1 and <= 6)
        {
            var tab = (Tab)(n - 1);
            window.DispatcherQueue.TryEnqueue(() => window.ShowTab(tab));
            return IntPtr.Zero;
        }
        return ComCtl32.DefSubclassProc(hwnd, msg, wParam, lParam);
    }

    void ShowTab(Tab tab)
    {
        var item = nav.MenuItems.OfType<NavigationViewItem>().FirstOrDefault(i => i.Tag is Tab t && t == tab);
        if (item != null) nav.SelectedItem = item;
        Select(tab);
    }

    readonly SettingsModel model = new();
    readonly UserDictionaryModel dictionary = new();
    readonly Updater updater = new();
    readonly ShortcutRecorder recorder;
    readonly NavigationView nav = new()
    {
        PaneDisplayMode = NavigationViewPaneDisplayMode.Left,
        IsBackButtonVisible = NavigationViewBackButtonVisible.Collapsed,
        IsSettingsVisible = false,
        IsPaneToggleButtonVisible = false,
        OpenPaneLength = 210,
    };
    readonly Grid body = new();
    readonly StackPanel banners = new();
    readonly Dictionary<Tab, FrameworkElement> pages = [];
    // 웹 페이지(배열 학습·타자 연습)는 언어를 바꿔도 그대로 쓴다(치던 판이 남게).
    readonly LearnPage learn = new();
    readonly PracticePage practice = new();
    Tab current;

    public MainWindow(string? startTab)
    {
        InitializeComponent();
        recorder = new ShortcutRecorder(Root);
        SystemBackdrop = new MicaBackdrop();
        Root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        Root.Children.Add(banners);
        Grid.SetRow(nav, 1);
        Root.Children.Add(nav);
        nav.Content = body;
        nav.SelectionChanged += (_, e) =>
        {
            if (e.SelectedItem is NavigationViewItem { Tag: Tab tab }) Select(tab);
        };
        current = TabNamed(startTab) ?? Tab.General;
        Build();

        model.Changed += () =>
        {
            Banners();
            foreach (var page in pages.Values.OfType<SettingsPage>()) page.Refresh();
        };
        // 다른 곳(직접 편집, 다른 설정 창)에서 바뀐 것을 창이 앞으로 올 때 다시 읽는다.
        Activated += (_, e) =>
        {
            if (e.WindowActivationState == WindowActivationState.Deactivated)
            {
                recorder.Stop();
                return;
            }
            model.Reload();
            switch (pages[current])
            {
                case SettingsPage p:
                    p.Shown();
                    break;
                case WebPage w:
                    w.Shown();
                    break;
            }
        };

        var icon = Path.Combine(Paths.AppDir, "Assets", "cssgsg.ico");
        if (File.Exists(icon)) AppWindow.SetIcon(icon);
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        shown = this;
        unsafe { ComCtl32.SetWindowSubclass(hwnd, &WindowProc, 1, 0); }
        var scale = User32.GetDpiForWindow(hwnd) / 96.0;
        if (scale <= 0) scale = 1;
        AppWindow.Resize(new SizeInt32((int)(980 * scale), (int)(780 * scale)));
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.PreferredMinimumWidth = (int)(720 * scale);
            presenter.PreferredMinimumHeight = (int)(520 * scale);
        }
    }

    /// 탭과 탭 목록을 지금 화면 언어로 만든다.
    void Build()
    {
        Title = T("cssgsg 설정", "cssgsg Settings", "cssgsg 設定");
        Root.Language = Tag;
        // 웹 페이지는 트리에서 빼지 않는다(빼면 웹 뷰가 닫힐 수 있다).
        foreach (var old in pages.Values.OfType<SettingsPage>()) body.Children.Remove(old);
        pages.Clear();
        pages[Tab.General] = new GeneralPage(model, recorder);
        pages[Tab.Korean] = new KoreanPage(model);
        pages[Tab.Japanese] = new JapanesePage(model, dictionary);
        pages[Tab.Learn] = learn;
        pages[Tab.Practice] = practice;
        pages[Tab.About] = new AboutPage(model, updater, ChangeLanguage);
        foreach (var page in pages.Values)
        {
            page.Visibility = Visibility.Collapsed;
            if (!body.Children.Contains(page)) body.Children.Add(page);
        }

        nav.MenuItems.Clear();
        (Tab, string, IconElement)[] items =
        [
            (Tab.General, T("일반", "General", "一般"), new FontIcon { Glyph = "" }),
            (Tab.Korean, T("한국어", "Korean", "韓国語"), Letter("가")),
            (Tab.Japanese, T("일본어", "Japanese", "日本語"), Letter("あ")),
            (Tab.Learn, T("배열 학습", "Layouts", "配列の学習"), new FontIcon { Glyph = "" }),
            (Tab.Practice, T("타자 연습", "Typing Practice", "タイピング練習"), new FontIcon { Glyph = "" }),
            (Tab.About, T("정보", "About", "情報"), new FontIcon { Glyph = "" }),
        ];
        NavigationViewItem? selected = null;
        foreach (var (tab, label, icon) in items)
        {
            var item = new NavigationViewItem { Content = label, Icon = icon, Tag = tab };
            nav.MenuItems.Add(item);
            if (tab == current) selected = item;
        }
        nav.SelectedItem = selected;
        Select(current);
        Banners();
    }

    static FontIcon Letter(string text) => new() { Glyph = text, FontFamily = new FontFamily("Malgun Gothic, Yu Gothic UI") };

    void Select(Tab tab)
    {
        if (tab != current) recorder.Stop();
        current = tab;
        foreach (var (t, page) in pages)
        {
            page.Visibility = t == tab ? Visibility.Visible : Visibility.Collapsed;
            if (t != tab && page is WebPage hidden) hidden.Hidden();
        }
        switch (pages[tab])
        {
            case SettingsPage p:
                p.Shown();
                break;
            case WebPage w:
                w.Shown();
                break;
        }
    }

    void ChangeLanguage(UiLanguage language)
    {
        Lang.Store(language);
        // 고르는 중인 라디오 단추가 이벤트를 다 낸 뒤에 바꾼다.
        DispatcherQueue.TryEnqueue(Build);
    }

    /// 설정 파일 문제(맥 Banner). 틀린 파일은 입력기가 앞의 올바른 설정으로 돈다.
    void Banners()
    {
        banners.Children.Clear();
        if (model.FileProblem is { } problem)
        {
            banners.Children.Add(new InfoBar
            {
                IsOpen = true,
                IsClosable = false,
                Severity = InfoBarSeverity.Warning,
                Message = T(
                    $"설정 파일에 오류가 있어 입력기가 앞의 올바른 설정(처음이면 기본 설정)으로 동작하고 있습니다: {problem}\n" +
                    "여기서 설정을 바꾸면 잘못된 파일은 config.toml.bak으로 옮기고 새로 만듭니다.",
                    $"The settings file has an error, so the input method keeps its last valid settings (the defaults at first): {problem}\n" +
                    "Changing a setting here moves the broken file to config.toml.bak and creates a new one.",
                    $"設定ファイルにエラーがあるため、入力メソッドは直前の正しい設定（最初は既定の設定）で動作しています: {problem}\n" +
                    "ここで設定を変更すると、壊れたファイルを config.toml.bak に移して新しく作成します。"),
            });
        }
        if (model.WriteProblem is { } write)
        {
            banners.Children.Add(new InfoBar
            {
                IsOpen = true,
                IsClosable = false,
                Severity = InfoBarSeverity.Error,
                Message = T($"설정을 저장하지 못했습니다: {write}", $"Couldn’t save the settings: {write}", $"設定を保存できませんでした: {write}"),
            });
        }
    }
}
