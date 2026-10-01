using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using static Cssgsg.Settings.Lang;

namespace Cssgsg.Settings.Pages;

// 맥 정보 탭의 "입력기 권한" 대신 윈도우는 엔진 호스트(Mozc·설정·한자 기억을 맡는 상주 프로세스)의 상태를 보인다.
sealed partial class AboutPage : SettingsPage
{
    readonly Updater updater;
    readonly Action<UiLanguage> changeLanguage;
    readonly TextBlock inputList = new();
    readonly Button addInput;
    readonly TextBlock host = new() { TextWrapping = TextWrapping.Wrap, TextAlignment = TextAlignment.Right };
    readonly Button restartHost;
    readonly TextBlock startup = new();
    readonly StackPanel updateStatus = new() { Orientation = Orientation.Horizontal, Spacing = 8 };
    readonly TextBlock lastCheck = new();
    readonly Button checkNow;
    readonly RadioButtons channel = new() { MaxColumns = 2 };
    readonly StackPanel notes = new() { Spacing = 6, MaxWidth = 420 };
    readonly Button logFile, practiceFile;
    readonly ToggleSwitch developer = new();

    public AboutPage(SettingsModel model, Updater updater, Action<UiLanguage> changeLanguage) : base(model, T("정보", "About", "情報"))
    {
        this.updater = updater;
        this.changeLanguage = changeLanguage;

        addInput = Ui.Button(T("추가", "Add", "追加"), () => _ = AddInputAsync());
        var inputRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Children = { inputList, addInput } };
        Add(Ui.Section("cssgsg", T(
                "입력기 목록은 윈도우 설정 → 시간 및 언어 → 언어 및 지역 → 영어(미국) → 언어 옵션 → 키보드에도 있습니다. " +
                "입력기는 Win+Space(또는 왼쪽 Alt+Shift)로 바꿉니다.",
                "The input list is also in Windows Settings → Time & language → Language & region → English (United States) → " +
                "Language options → Keyboards. Switch input methods with Win+Space (or Left Alt+Shift).",
                "入力方式の一覧は Windows の設定 → 時刻と言語 → 言語と地域 → 英語（米国）→ 言語のオプション → キーボード にもあります。" +
                "入力方式は Win+Space（または左Alt+Shift）で切り替えます。"),
            Ui.Row(T("버전", "Version", "バージョン"), new TextBlock { Text = BuildInfo.Version, IsTextSelectionEnabled = true }),
            Ui.Row(T("입력기 목록", "Input List", "入力方式の一覧"), inputRow),
            Ui.Row(T("윈도우 설정", "Windows Settings", "Windows の設定"),
                Ui.Button(T("언어 및 지역 열기", "Open Language & Region", "言語と地域を開く"), () => Shell.Open("ms-settings:regionlanguage"))),
            Ui.Row("GitHub", Ui.Link("NR2BJ/cssgsg", "https://github.com/NR2BJ/cssgsg"))));

        restartHost = Ui.Button(T("다시 시작", "Restart", "再起動"), () => _ = RestartHostAsync());
        var hostRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Children = { host, restartHost } };
        Add(Ui.Section(T("엔진 호스트", "Engine Host", "エンジンホスト"), T(
                "엔진 호스트(cssgsg-host.exe)는 일본어 변환(Mozc), 설정 파일, 고른 한자 기억을 맡는 프로세스입니다. 로그인할 때 " +
                "시작되어 계속 떠 있습니다(메모리 20MB 안팎). 꺼져 있으면 일본어는 가나만 입력되고, 다음에 일본어로 바꿀 때 다시 뜹니다.",
                "The engine host (cssgsg-host.exe) handles Japanese conversion (Mozc), the settings file, and remembered Hanja " +
                "choices. It starts when you sign in and keeps running (about 20 MB of memory). While it’s off, Japanese types " +
                "kana only, and it starts again the next time you switch to Japanese.",
                "エンジンホスト（cssgsg-host.exe）は日本語変換（Mozc）、設定ファイル、選んだ漢字の記憶を受け持つプロセスです。" +
                "サインイン時に起動して常駐します（メモリ約20MB）。停止中は日本語がかなのみになり、次に日本語に切り替えたときに再び起動します。"),
            Ui.Row(T("상태", "Status", "状態"), hostRow),
            Ui.Row(T("로그인할 때 시작", "Start at Sign-in", "サインイン時に起動"), startup)));

        foreach (var name in new[] { T("정식", "Stable", "正式版"), T("베타", "Beta", "ベータ") }) channel.Items.Add(name);
        channel.SelectedIndex = updater.Beta ? 1 : 0;
        channel.SelectionChanged += (_, _) =>
        {
            if (channel.SelectedIndex >= 0) updater.Beta = channel.SelectedIndex == 1;
        };
        checkNow = Ui.Button(T("지금 확인", "Check Now", "今すぐ確認"), () => _ = updater.CheckAsync(userInitiated: true));
        lastCheck.Style = Ui.Style("CaptionTextBlockStyle");
        lastCheck.Foreground = Ui.Brush("TextFillColorSecondaryBrush");
        var checkRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Children = { lastCheck, checkNow } };
        lastCheck.VerticalAlignment = VerticalAlignment.Center;
        Add(Ui.Section(T("업데이트", "Updates", "アップデート"), T(
                "베타 채널은 정식 출시 전의 테스트 버전도 받습니다(불안정할 수 있습니다). 설치할 때 관리자 권한을 묻고, 끝나면 이 앱이 " +
                "다시 열립니다. 이미 열려 있던 앱은 다시 열면 새 입력기를 씁니다.",
                "The beta channel also gets test builds before release (they may be unstable). Installing asks for administrator " +
                "permission, and this app opens again when it finishes. Apps that were already open use the new version after you reopen them.",
                "ベータチャンネルは正式リリース前のテストビルドも受け取ります（不安定な場合があります）。インストールには管理者の許可が必要で、" +
                "終わるとこのアプリが再び開きます。すでに開いていたアプリは、開き直すと新しい入力メソッドを使います。"),
            Ui.Row(T("업데이트 채널", "Update Channel", "アップデートチャンネル"), channel),
            Ui.Row(T("상태", "Status", "状態"), new StackPanel { Spacing = 8, Children = { updateStatus, notes } }),
            Ui.Row(T("확인", "Check", "確認"), checkRow)));

        var language = new RadioButtons { MaxColumns = 3 };
        foreach (var l in Lang.All) language.Items.Add(Lang.Name(l));
        language.SelectedIndex = Array.IndexOf(Lang.All, Active);
        language.SelectionChanged += (_, _) =>
        {
            if (language.SelectedIndex >= 0 && Lang.All[language.SelectedIndex] != Active)
                changeLanguage(Lang.All[language.SelectedIndex]);
        };
        Add(Ui.Section(T("화면 언어", "Language", "表示言語"), null, Ui.Block(language)));

        developer.OnContent = T("켬", "On", "オン");
        developer.OffContent = T("끔", "Off", "オフ");
        developer.Toggled += (_, _) =>
        {
            if (!Refreshing) Shell.DeveloperLog = developer.IsOn;
        };
        logFile = Ui.Button(T("탐색기에서 보기", "Show in Explorer", "エクスプローラーで表示"), () => Shell.Reveal(Paths.DeveloperLog));
        practiceFile = Ui.Button(T("탐색기에서 보기", "Show in Explorer", "エクスプローラーで表示"), () => Shell.Reveal(Paths.PracticeRecord));
        Add(Ui.Section(T("파일", "Files", "ファイル"), T(
                "설정 파일(config.toml)은 직접 고쳐도 됩니다. 저장한 뒤 입력칸을 다시 누르거나 앱을 바꾸면 적용됩니다. " +
                "개발자 기록에는 키 코드·수식키·시각만 남고, 입력한 글자는 남지 않습니다. 켜고 끈 것은 앱을 새로 열 때부터, " +
                "엔진 호스트는 다시 시작한 뒤부터 적용됩니다. 타자 연습 기록(practice.json)에는 끝낸 단계와 자주 틀린 키가 남습니다.",
                "You can edit the settings file (config.toml) directly; it applies when you click into a text field again or switch apps. " +
                "The developer log records only key codes, modifiers, and times, never the text you type. Turning it on or off " +
                "applies to apps you open afterwards, and to the engine host after a restart. The typing practice records " +
                "(practice.json) keep the stages you finished and the keys you often miss.",
                "設定ファイル（config.toml）は直接編集してもかまいません。保存してから入力欄をクリックし直すかアプリを切り替えると反映されます。" +
                "開発者ログにはキーコード・修飾キー・時刻だけが残り、入力した文字は残りません。オン・オフは以後に開いたアプリから、" +
                "エンジンホストは再起動後から反映されます。タイピング練習の記録（practice.json）には終えた段階とよく間違えるキーが残ります。"),
            Ui.Row(T("설정 파일", "Settings File", "設定ファイル"),
                Ui.Button(T("탐색기에서 보기", "Show in Explorer", "エクスプローラーで表示"), M.RevealConfigFile)),
            Ui.Row(T("개발자 기록", "Developer Log", "開発者ログ"), developer),
            Ui.Row(T("기록 파일", "Log File", "ログファイル"), logFile),
            Ui.Row(T("타자 연습 기록", "Typing Practice Records", "タイピング練習の記録"), practiceFile)));

        Add(Ui.Section(T("라이선스", "License", "ライセンス"), null,
            Ui.Row("cssgsg", new TextBlock { Text = "MIT" }),
            Ui.Row(T("오픈 소스 고지", "Open Source Notices", "オープンソースの告知"),
                Ui.Button(T("보기…", "View…", "表示…"), () => _ = ShowNoticesAsync()))));

        updater.Changed += PaintUpdate;
        Unloaded += (_, _) => updater.Changed -= PaintUpdate;
        PaintUpdate();
        Shown();
    }

    public override void Shown()
    {
        var listed = Shell.InInputList();
        inputList.Text = listed ? T("추가됨", "Added", "追加済み") : T("추가 안 됨", "Not added", "未追加");
        inputList.Foreground = listed ? Ui.Brush("SystemFillColorSuccessBrush") : Ui.Brush("SystemFillColorCautionBrush");
        addInput.Visibility = listed || !File.Exists(Paths.HostExe) ? Visibility.Collapsed : Visibility.Visible;
        startup.Text = Shell.HostStartsAtLogin() ? T("예", "Yes", "はい") : T("아니요", "No", "いいえ");
        restartHost.IsEnabled = File.Exists(Paths.HostExe);
        Quietly(() => developer.IsOn = Shell.DeveloperLog);
        logFile.IsEnabled = File.Exists(Paths.DeveloperLog);
        practiceFile.IsEnabled = File.Exists(Paths.PracticeRecord);
        _ = ShowHostAsync();
        updater.CheckIfDue();
    }

    async Task ShowHostAsync()
    {
        var engine = await HostClient.EngineVersionAsync();
        host.Text = engine switch
        {
            null => T("꺼짐", "Not running", "停止中"),
            "" => T("실행 중 · Mozc를 읽지 못함(가나만 입력)", "Running · Mozc couldn’t be loaded (kana only)", "実行中・Mozc を読み込めず（かなのみ）"),
            _ => T($"실행 중 · Mozc {engine}", $"Running · Mozc {engine}", $"実行中・Mozc {engine}"),
        };
        restartHost.Content = engine == null ? T("시작", "Start", "起動") : T("다시 시작", "Restart", "再起動");
    }

    async Task RestartHostAsync()
    {
        restartHost.IsEnabled = false;
        host.Text = T("다시 시작하는 중…", "Restarting…", "再起動中…");
        await HostClient.QuitAsync();
        Shell.StartHost();
        // 엔진(Mozc)을 읽을 때까지 잠깐 걸린다.
        for (var i = 0; i < 20 && await HostClient.EngineVersionAsync() == null; i++) await Task.Delay(150);
        restartHost.IsEnabled = true;
        await ShowHostAsync();
    }

    async Task AddInputAsync()
    {
        addInput.IsEnabled = false;
        await Shell.InstallForUserAsync();
        addInput.IsEnabled = true;
        Shown();
    }

    void PaintUpdate()
    {
        updateStatus.Children.Clear();
        notes.Children.Clear();
        notes.Visibility = Visibility.Collapsed;
        channel.IsEnabled = !updater.Busy;
        checkNow.IsEnabled = !updater.Busy;
        lastCheck.Text = updater.LastCheck is { } t
            ? T("마지막 확인: ", "Last checked: ", "最終確認: ") + t.ToString("g")
            : "";
        switch (updater.State)
        {
            case Updater.Phase.Idle:
                updateStatus.Children.Add(Secondary(T("확인 전", "Not checked", "未確認")));
                break;
            case Updater.Phase.Checking:
                updateStatus.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
                updateStatus.Children.Add(new TextBlock { Text = T("확인 중…", "Checking…", "確認中…") });
                break;
            case Updater.Phase.UpToDate:
                updateStatus.Children.Add(Colored(T("최신 버전입니다", "Up to date", "最新です"), "SystemFillColorSuccessBrush"));
                break;
            case Updater.Phase.Available when updater.Offer is { } offer:
                updateStatus.Children.Add(new TextBlock
                {
                    Text = T($"새 버전 {offer.Version}", $"Version {offer.Version} available", $"新しいバージョン {offer.Version}"),
                    FontWeight = FontWeights.SemiBold,
                    VerticalAlignment = VerticalAlignment.Center,
                });
                updateStatus.Children.Add(Ui.Button(T("설치", "Install", "インストール"), () => _ = updater.InstallAsync(), accent: true));
                if (!string.IsNullOrWhiteSpace(offer.Notes))
                {
                    notes.Children.Add(new ScrollViewer
                    {
                        MaxHeight = 160,
                        Content = new TextBlock { Text = offer.Notes.Trim(), TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true },
                    });
                }
                if (offer.Page != null) notes.Children.Add(Ui.Link(T("릴리스 페이지", "Release Page", "リリースページ"), offer.Page));
                notes.Visibility = notes.Children.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
                break;
            case Updater.Phase.Downloading:
                updateStatus.Children.Add(new ProgressBar { Value = updater.Progress * 100, Width = 140, VerticalAlignment = VerticalAlignment.Center });
                updateStatus.Children.Add(new TextBlock { Text = $"{(int)(updater.Progress * 100)}%" });
                break;
            case Updater.Phase.Installing:
                updateStatus.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
                updateStatus.Children.Add(new TextBlock { Text = T("설치 중… (관리자 권한을 허용하세요)", "Installing… (allow administrator access)", "インストール中…（管理者権限を許可してください）") });
                break;
            case Updater.Phase.Failed:
                updateStatus.Children.Add(Colored(updater.Problem ?? "", "SystemFillColorCautionBrush"));
                break;
        }
    }

    static TextBlock Secondary(string text) => new() { Text = text, Foreground = Ui.Brush("TextFillColorSecondaryBrush") };

    static TextBlock Colored(string text, string brush) => new() { Text = text, Foreground = Ui.Brush(brush), TextWrapping = TextWrapping.Wrap };

    async Task ShowNoticesAsync()
    {
        var parts = new List<string>();
        foreach (var file in Paths.Notices)
        {
            try
            {
                if (File.Exists(file)) parts.Add(await File.ReadAllTextAsync(file));
            }
            catch (Exception) { }
        }
        var text = parts.Count > 0
            ? string.Join("\n\n", parts)
            : T("고지 파일이 없습니다.", "The notices file is missing.", "告知ファイルがありません。");
        var box = new TextBlock
        {
            Text = text,
            FontFamily = new FontFamily("Cascadia Mono, Consolas"),
            FontSize = 12,
            IsTextSelectionEnabled = true,
            TextWrapping = TextWrapping.Wrap,
        };
        var dialog = Ui.Dialog(XamlRoot, T("오픈 소스 고지", "Open Source Notices", "オープンソースの告知"),
            new ScrollViewer { Content = box, MaxHeight = 520, MinWidth = 600 }, "", T("닫기", "Close", "閉じる"));
        dialog.Resources["ContentDialogMaxWidth"] = 900.0;
        await dialog.ShowAsync();
    }
}
