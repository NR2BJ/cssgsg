namespace Cssgsg.Settings;

/// 파일 자리. 설치 폴더(Program Files\cssgsg)의 settings 폴더에 이 앱이 있고, 입력기·엔진 호스트·Mozc는 그 위 폴더에 있다.
/// 설정·한자 기억·타자 연습 기록은 %APPDATA%\cssgsg(맥 ~/Library/Application Support/cssgsg),
/// Mozc 학습·사용자 사전과 개발자 기록은 %LOCALAPPDATA%\cssgsg에 있다(엔진 호스트와 입력기가 쓰는 자리와 같다).
static class Paths
{
    public static string AppDir => AppContext.BaseDirectory;
    public static string InstallDir => Path.GetFullPath(Path.Combine(AppDir, ".."));

    public static string UserDir =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "cssgsg");
    public static string Config => Path.Combine(UserDir, "config.toml");
    public static string HanjaLearning => Path.Combine(UserDir, "hanja-learning.tsv");
    public static string PracticeRecord => Path.Combine(UserDir, "practice.json");

    public static string LocalDir =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "cssgsg");
    public static string MozcProfile => Path.Combine(LocalDir, "mozc");
    public static string UserDictionary => Path.Combine(MozcProfile, "user_dictionary.db");
    public static string DeveloperLog => Path.Combine(LocalDir, "tip-debug.log");
    public static string WebViewData => Path.Combine(LocalDir, "webview2");
    public static string Updates => Path.Combine(LocalDir, "updates");

    public static string HostExe => Path.Combine(InstallDir, "cssgsg-host.exe");
    public static string MozcVersion => Path.Combine(InstallDir, "mozc", "MOZC_VERSION");

    /// 고지문: 설치 폴더의 THIRD_PARTY_NOTICES.txt(엔진·배열·사전)와 이 앱 폴더의 윈도우 것(.NET, Windows App SDK, WebView2).
    public static IEnumerable<string> Notices =>
    [
        Path.Combine(InstallDir, "THIRD_PARTY_NOTICES.txt"),
        Path.Combine(AppDir, "THIRD_PARTY_NOTICES-windows.txt"),
    ];
}
