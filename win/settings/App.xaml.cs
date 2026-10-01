using System.Diagnostics;
using Microsoft.UI.Xaml;

namespace Cssgsg.Settings;

/// cssgsg 설정 앱. 설치 폴더(Program Files\cssgsg\settings)에 있고 시작 메뉴의 "cssgsg 설정"이 연다.
/// 입력기 프로세스(앱마다 뜨는 DLL) 안에 창을 두지 않는다: 입력기가 자기 창의 입력을 받는 꼴이 되지 않게(맥과 같다).
/// 하나만 띄운다: 이미 떠 있으면 그 창을 앞으로 가져오고 끝난다.
public partial class App : Application
{
    Window? window;
    Mutex? single;

    public App()
    {
        InitializeComponent();
        // 처리하지 못한 예외는 %LOCALAPPDATA%\cssgsg\settings-error.log에 남긴다(입력한 글자는 들어가지 않는다).
        UnhandledException += (_, e) => LogError(e.Exception);
        AppDomain.CurrentDomain.UnhandledException += (_, e) => LogError(e.ExceptionObject as Exception);
    }

    static void LogError(Exception? e)
    {
        try
        {
            Directory.CreateDirectory(Paths.LocalDir);
            File.AppendAllText(Path.Combine(Paths.LocalDir, "settings-error.log"), $"{DateTime.Now:yyyy-MM-dd HH:mm:ss} {BuildInfo.Version} {e}\n");
        }
        catch (Exception) { }
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        var cli = Environment.GetCommandLineArgs();
        var i = Array.IndexOf(cli, "--tab");
        var tab = i >= 0 && i + 1 < cli.Length ? cli[i + 1] : null;
        single = new Mutex(true, @"Local\cssgsg-settings", out var first);
        if (!first)
        {
            BringOtherToFront(tab);
            Exit();
            return;
        }
        // 웹 뷰 데이터는 설치 폴더(쓸 수 없다) 대신 사용자 폴더에 둔다. 웹 뷰를 만들기 전에 정한다.
        Environment.SetEnvironmentVariable("WEBVIEW2_USER_DATA_FOLDER", Paths.WebViewData);
        window = new MainWindow(tab);
        window.Activate();
    }

    /// 떠 있는 설정 창을 앞으로 가져오고, 탭을 달라고 했으면 그 탭을 보이라고 한다.
    static void BringOtherToFront(string? tab)
    {
        foreach (var p in Process.GetProcessesByName("cssgsg-settings"))
        {
            using (p)
            {
                if (p.Id == Environment.ProcessId || p.MainWindowHandle == IntPtr.Zero) continue;
                if (User32.IsIconic(p.MainWindowHandle)) User32.ShowWindow(p.MainWindowHandle, User32.SW_RESTORE);
                User32.SetForegroundWindow(p.MainWindowHandle);
                var param = MainWindow.ShowTabParam(tab);
                if (param != IntPtr.Zero) User32.PostMessageW(p.MainWindowHandle, MainWindow.ShowTabMessage, param, IntPtr.Zero);
            }
        }
    }
}
