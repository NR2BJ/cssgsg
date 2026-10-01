using System.Diagnostics;
using Microsoft.Win32;

namespace Cssgsg.Settings;

/// 탐색기·브라우저·윈도우 설정 열기와 입력기 상태 읽기.
static class Shell
{
    /// 탐색기에서 이 파일을 골라 보인다(없으면 폴더를 연다).
    public static void Reveal(string path)
    {
        try
        {
            if (File.Exists(path))
                Process.Start("explorer.exe", $"/select,\"{path}\"");
            else if (Directory.Exists(Path.GetDirectoryName(path)))
                Process.Start("explorer.exe", $"\"{Path.GetDirectoryName(path)}\"");
        }
        catch (Exception) { }
    }

    /// 기본 브라우저나 윈도우 설정(ms-settings:)으로 연다.
    public static void Open(string uri)
    {
        try { Process.Start(new ProcessStartInfo(uri) { UseShellExecute = true }); }
        catch (Exception) { }
    }

    /// win/tip/src/lib.rs의 CLSID_TEXT_SERVICE, GUID_PROFILE(내 입력 목록의 항목 이름, en-US).
    const string TipItem = "0409:{A9227DC2-56BC-4023-AE29-82A9AAA4EE14}{DCFBD969-D52F-4AFB-ABC0-271CC58FE18C}";

    /// 내 입력 목록(윈도우 설정 → 시간 및 언어 → 언어 및 지역)에 cssgsg가 있는지. 윈도우는 언어마다 키를 두고
    /// 그 언어의 입력기를 값 이름으로 적는다(HKCU\Control Panel\International\User Profile\en-US).
    public static bool InInputList()
    {
        try
        {
            using var profiles = Registry.CurrentUser.OpenSubKey(@"Control Panel\International\User Profile");
            if (profiles == null) return false;
            foreach (var language in profiles.GetSubKeyNames())
            {
                using var key = profiles.OpenSubKey(language);
                if (key?.GetValueNames().Any(n => n.Equals(TipItem, StringComparison.OrdinalIgnoreCase)) == true)
                    return true;
            }
        }
        catch (Exception) { }
        return false;
    }

    /// 로그인할 때 엔진 호스트를 띄우는지(HKCU Run의 cssgsg, 설치기의 --install-user가 넣는다).
    public static bool HostStartsAtLogin()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Run");
            return key?.GetValue("cssgsg") != null;
        }
        catch (Exception)
        {
            return false;
        }
    }

    const string CssgsgKey = @"Software\cssgsg";

    /// 개발자 기록(입력기·엔진 호스트, HKCU\Software\cssgsg DebugLog).
    public static bool DeveloperLog
    {
        get
        {
            try
            {
                using var key = Registry.CurrentUser.OpenSubKey(CssgsgKey);
                return key?.GetValue("DebugLog") is int v && v != 0;
            }
            catch (Exception)
            {
                return false;
            }
        }
        set
        {
            try
            {
                using var key = Registry.CurrentUser.CreateSubKey(CssgsgKey);
                key.SetValue("DebugLog", value ? 1 : 0, RegistryValueKind.DWord);
            }
            catch (Exception) { }
        }
    }

    /// 엔진 호스트를 이 사용자로 띄운다(이미 떠 있으면 새것은 바로 끝난다).
    public static bool StartHost(string arguments = "")
    {
        if (!File.Exists(Paths.HostExe)) return false;
        try
        {
            using var p = Process.Start(new ProcessStartInfo(Paths.HostExe, arguments) { UseShellExecute = false, CreateNoWindow = true });
            return p != null;
        }
        catch (Exception)
        {
            return false;
        }
    }

    /// 엔진 호스트의 사용자 쪽 설치(내 입력 목록에 넣기, 시작 프로그램, 띄우기)를 하고 끝날 때까지 기다린다.
    public static async Task<bool> InstallForUserAsync()
    {
        if (!File.Exists(Paths.HostExe)) return false;
        try
        {
            using var p = Process.Start(new ProcessStartInfo(Paths.HostExe, "--install-user") { UseShellExecute = false, CreateNoWindow = true });
            if (p == null) return false;
            await p.WaitForExitAsync();
            return p.ExitCode == 0;
        }
        catch (Exception)
        {
            return false;
        }
    }
}
