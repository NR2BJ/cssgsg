using System.Runtime.InteropServices;

namespace Cssgsg.Settings;

/// 러스트 설정 라이브러리(cssgsg_config.dll, config-ffi). 설정의 모양과 값 검사, 사용자 사전 파일 형식은 그쪽에만 있다.
/// 돌려받은 문자열은 같은 스레드에서 다시 부르기 전까지만 유효해서 곧바로 복사한다.
static partial class ConfigLib
{
    const string Dll = "cssgsg_config";

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    private static partial IntPtr cssgsg_config_json_windows(string? toml);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    private static partial IntPtr cssgsg_config_toml_windows(string json);

    [LibraryImport(Dll)]
    private static partial IntPtr cssgsg_config_key_name_windows(ushort scan, byte extended);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    private static partial IntPtr cssgsg_userdict_json(string path);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    private static partial byte cssgsg_userdict_save(string path, string json);

    [LibraryImport(Dll)]
    private static partial IntPtr cssgsg_config_error();

    static string LastError() => Marshal.PtrToStringUTF8(cssgsg_config_error()) ?? "";

    static (string? Value, string? Error) Take(IntPtr p) =>
        p == IntPtr.Zero ? (null, LastError()) : (Marshal.PtrToStringUTF8(p), null);

    /// 설정 파일 내용(null이면 기본 설정) → JSON. 틀린 파일이면 까닭.
    public static (string? Json, string? Error) JsonFromToml(string? toml) => Take(cssgsg_config_json_windows(toml));

    /// JSON → 설정 파일 내용(설명이 달리고 윈도우 기본값과 같은 설정은 주석). 받아 줄 수 없으면 까닭.
    public static (string? Toml, string? Error) TomlFromJson(string json) => Take(cssgsg_config_toml_windows(json));

    /// 스캔 코드의 키 이름("enter", "a"). 수식키는 "mod:shift_left" 꼴, 모르는 키는 null.
    public static string? KeyName(uint scan, bool extended)
    {
        if (scan > ushort.MaxValue) return null;
        var p = cssgsg_config_key_name_windows((ushort)scan, (byte)(extended ? 1 : 0));
        return p == IntPtr.Zero ? null : Marshal.PtrToStringUTF8(p);
    }

    public static (string? Json, string? Error) UserDictJson(string path) => Take(cssgsg_userdict_json(path));

    /// 사용자 사전을 쓴다. 못 쓰면 까닭.
    public static string? UserDictSave(string path, string json) =>
        cssgsg_userdict_save(path, json) != 0 ? null : LastError();
}

static partial class User32
{
    public const int SW_RESTORE = 9;

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool SetForegroundWindow(IntPtr hwnd);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool ShowWindow(IntPtr hwnd, int command);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool IsIconic(IntPtr hwnd);

    [LibraryImport("user32.dll")]
    public static partial uint GetDpiForWindow(IntPtr hwnd);
}
