using System.Globalization;
using Microsoft.Win32;

namespace Cssgsg.Settings;

/// 설정 앱의 화면 언어. HKCU\Software\cssgsg의 AppLanguage에 둔다(맥은 설정 앱의 기본값 저장소).
/// 고른 적이 없으면 윈도우 표시 언어를 따른다(한국어·일본어, 그 밖은 영어).
enum UiLanguage { Ko, En, Ja }

static class Lang
{
    const string Key = @"Software\cssgsg";
    const string Value = "AppLanguage";

    public static UiLanguage Active { get; set; } = Stored();

    /// 화면 문구를 지금 언어로 고른다. 세 벌을 쓰는 자리에 같이 둬서 빠진 번역이 없게 한다(맥 tr과 같다).
    public static string T(string ko, string en, string ja) => Active switch
    {
        UiLanguage.En => en,
        UiLanguage.Ja => ja,
        _ => ko,
    };

    public static readonly UiLanguage[] All = [UiLanguage.Ko, UiLanguage.En, UiLanguage.Ja];

    /// 언어 이름은 늘 그 언어로 쓴다(고르는 사람이 알아보게).
    public static string Name(UiLanguage l) => l switch
    {
        UiLanguage.En => "English",
        UiLanguage.Ja => "日本語",
        _ => "한국어",
    };

    static string Code(UiLanguage l) => l switch
    {
        UiLanguage.En => "en",
        UiLanguage.Ja => "ja",
        _ => "ko",
    };

    /// XAML의 Language(한자 글꼴 고르기 등).
    public static string Tag => Active switch
    {
        UiLanguage.En => "en-US",
        UiLanguage.Ja => "ja-JP",
        _ => "ko-KR",
    };

    static UiLanguage Stored()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(Key);
            switch (key?.GetValue(Value) as string)
            {
                case "ko": return UiLanguage.Ko;
                case "en": return UiLanguage.En;
                case "ja": return UiLanguage.Ja;
            }
        }
        catch (Exception) { }
        return CultureInfo.CurrentUICulture.TwoLetterISOLanguageName switch
        {
            "ko" => UiLanguage.Ko,
            "ja" => UiLanguage.Ja,
            _ => UiLanguage.En,
        };
    }

    public static void Store(UiLanguage l)
    {
        Active = l;
        try
        {
            using var key = Registry.CurrentUser.CreateSubKey(Key);
            key.SetValue(Value, Code(l));
        }
        catch (Exception) { }
    }
}
