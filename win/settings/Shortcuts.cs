using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using VirtualKey = Windows.System.VirtualKey;

namespace Cssgsg.Settings;

/// 단축키 글자열(설정 파일, core/src/shortcut.rs) ↔ 화면 글자(맥 ShortcutText의 윈도우판: Ctrl·Alt·Win, Enter·Backspace).
/// - 탭: "tap:shift_right" → "오른쪽 Shift 탭"
/// - 조합: "control_left+space" → "왼쪽 Ctrl + Space". 손으로 적은 좌우 없는 이름("control")은 "Ctrl"(어느 쪽이든).
/// - "": 없음
static class ShortcutText
{
    /// 수식키 종류. 파일에 적는 순서와 같다.
    static readonly (string Name, string Label)[] Families =
        [("control", "Ctrl"), ("alt", "Alt"), ("shift", "Shift"), ("meta", "Win")];

    /// 수식키 없이 단축키로 두면 그 키를 칠 수 없게 되는 키(코어 Shortcut::validate와 같다).
    static readonly HashSet<string> NeedsModifier =
    [
        "space", "enter", "tab", "backspace", "escape", "minus", "equal", "bracket_left", "bracket_right", "backslash",
        "semicolon", "quote", "grave", "comma", "period", "slash",
    ];

    public static string Label(string value)
    {
        if (value.Length == 0 || value == "none") return Lang.T("없음", "None", "なし");
        if (value.StartsWith("tap:")) return TapLabel(value[4..]);
        var parts = value.Split('+');
        return string.Join(" + ", parts[..^1].Select(ModifierLabel).Append(KeyLabel(parts[^1])));
    }

    /// "control_left" → "왼쪽 Ctrl", "control" → "Ctrl".
    public static string ModifierLabel(string name)
    {
        var parts = name.Split('_');
        var key = Families.FirstOrDefault(f => f.Name == parts[0]).Label ?? parts[0];
        if (parts.Length != 2) return key;
        return parts[1] == "right"
            ? Lang.T($"오른쪽 {key}", $"Right {key}", $"右{key}")
            : Lang.T($"왼쪽 {key}", $"Left {key}", $"左{key}");
    }

    static string TapLabel(string name)
    {
        var m = ModifierLabel(name);
        return Lang.T($"{m} 탭", $"{m} tap", $"{m} タップ");
    }

    public static string KeyLabel(string name) => name switch
    {
        "enter" => "Enter",
        "keypad_enter" => Lang.T("숫자패드 Enter", "Numpad Enter", "テンキー Enter"),
        "escape" => "Esc",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "insert" => "Insert",
        "tab" => "Tab",
        "space" => "Space",
        "minus" => "-",
        "equal" => "=",
        "bracket_left" => "[",
        "bracket_right" => "]",
        "backslash" => "\\",
        "semicolon" => ";",
        "quote" => "'",
        "grave" => "`",
        "comma" => ",",
        "period" => ".",
        "slash" => "/",
        "home" => "Home",
        "end" => "End",
        "page_up" => "Page Up",
        "page_down" => "Page Down",
        "left" => "←",
        "right" => "→",
        "up" => "↑",
        "down" => "↓",
        _ => name.ToUpperInvariant(),
    };

    public static string ActionName(string field) => field switch
    {
        "toggle_english" => Lang.T("영어 ↔ 비영어", "English ↔ Non-English", "英語 ↔ 英語以外"),
        "toggle_non_english" => Lang.T("한국어 ↔ 일본어", "Korean ↔ Japanese", "韓国語 ↔ 日本語"),
        _ => Lang.T("한자 변환", "Hanja Conversion", "ハンジャ変換（韓国語の漢字）"),
    };

    /// 이 키만으로는 단축키가 될 수 없는지(그 글자를 칠 수 없게 된다).
    public static bool NeedsAModifier(string key) => NeedsModifier.Contains(key) || key.Length == 1;

    /// 수식키 이름을 파일 순서(control, alt, shift, meta; 왼쪽 먼저)로 늘어놓는다.
    public static IEnumerable<string> InFileOrder(IEnumerable<string> modifiers) =>
        modifiers.OrderBy(m => Array.FindIndex(Families, f => m.StartsWith(f.Name))).ThenBy(m => m.EndsWith("_right"));
}

/// 단축키 녹화(맥 ShortcutRecording과 같은 방식). 녹화하는 동안 창에 온 키를 가로채서 쓰고 창에는 넘기지 않는다.
/// - 수식키 + 다른 키: 다른 키를 누르는 순간 조합으로 기록한다. 수식키는 좌우를 가려 적는다("control_left+space").
/// - 수식키 하나만 눌렀다 떼면(사이에 다른 키·수식키 없이) 0.15초 뒤 탭으로 기록한다.
/// - 수식키 없이 Esc: 취소.
/// - Alt·Windows 키는 받지 않는다: Alt 조합은 앱 메뉴가 먼저 가져가서 입력기에 오지 않고 Alt 탭은 뗄 때 앱 메뉴를 연다
///   (Shortcuts::windows 주석). Windows 키는 탭이면 시작 메뉴, 조합이면 윈도우 단축키다.
/// 키 자리는 스캔 코드로 읽는다(입력기와 같다: 키보드 배치와 상관없다).
sealed class ShortcutRecorder
{
    readonly UIElement root;
    readonly DispatcherQueueTimer tapTimer;
    Action<string?>? done;
    Action<string>? rejected;
    readonly HashSet<string> held = [];
    string? tapCandidate;
    string? pendingTap;

    public bool Active => done != null;
    /// 녹화를 시작하거나 끝냈다(단추 글자를 바꾼다).
    public event Action? ActiveChanged;

    public ShortcutRecorder(UIElement root)
    {
        this.root = root;
        tapTimer = DispatcherQueue.GetForCurrentThread().CreateTimer();
        tapTimer.Interval = TimeSpan.FromMilliseconds(150);
        tapTimer.IsRepeating = false;
        tapTimer.Tick += (_, _) =>
        {
            if (pendingTap != null) Finish("tap:" + pendingTap);
        };
        root.AddHandler(UIElement.PreviewKeyDownEvent, new KeyEventHandler(OnKeyDown), true);
        root.AddHandler(UIElement.PreviewKeyUpEvent, new KeyEventHandler(OnKeyUp), true);
    }

    public void Start(Action<string?> done, Action<string> rejected)
    {
        Stop();
        this.done = done;
        this.rejected = rejected;
        held.Clear();
        tapCandidate = null;
        ActiveChanged?.Invoke();
    }

    public void Stop()
    {
        tapTimer.Stop();
        pendingTap = null;
        var was = Active;
        done = null;
        rejected = null;
        if (was) ActiveChanged?.Invoke();
    }

    void Finish(string? value)
    {
        var d = done;
        Stop();
        d?.Invoke(value);
    }

    void Reject(string reason)
    {
        tapTimer.Stop();
        pendingTap = null;
        rejected?.Invoke(reason);
    }

    static string AltWinReason => Lang.T(
        "Alt와 Windows 키는 쓸 수 없습니다. Alt는 앱 메뉴가, Windows 키는 시작 메뉴와 윈도우 단축키가 먼저 가져갑니다.",
        "Alt and the Windows key can’t be used: Alt goes to the app’s menu, and the Windows key to the Start menu and Windows shortcuts.",
        "Alt と Windows キーは使えません。Alt はアプリのメニューが、Windows キーはスタートメニューと Windows のショートカットが先に受け取ります。");

    void OnKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (!Active) return;
        e.Handled = true;
        tapTimer.Stop();
        pendingTap = null;
        var name = ConfigLib.KeyName(e.KeyStatus.ScanCode, e.KeyStatus.IsExtendedKey);
        if (name == null) return;  // 모르는 키(한/영 등)
        if (name.StartsWith("mod:"))
        {
            var modifier = name[4..];
            if (!held.Contains(modifier))
            {
                tapCandidate = held.Count == 0 ? modifier : null;
                held.Add(modifier);
            }
            return;
        }
        tapCandidate = null;
        if (e.Key == VirtualKey.Escape && held.Count == 0)
        {
            Finish(null);  // 취소
            return;
        }
        if (e.KeyStatus.WasKeyDown) return;  // 자동 반복
        if (held.Count == 0 && ShortcutText.NeedsAModifier(name))
        {
            var key = ShortcutText.KeyLabel(name);
            Reject(Lang.T(
                $"{key}만으로는 단축키를 만들 수 없습니다(그 키를 칠 수 없게 됩니다). 수식키와 함께 누르세요.",
                $"{key} alone can’t be a shortcut (you couldn’t type it anymore). Hold a modifier with it.",
                $"{key} だけではショートカットにできません（その文字が打てなくなります）。修飾キーと一緒に押してください。"));
            return;
        }
        if (held.Any(m => m.StartsWith("alt") || m.StartsWith("meta")))
        {
            Reject(AltWinReason);
            return;
        }
        Finish(string.Join("+", ShortcutText.InFileOrder(held).Append(name)));
    }

    void OnKeyUp(object sender, KeyRoutedEventArgs e)
    {
        if (!Active) return;
        e.Handled = true;
        var name = ConfigLib.KeyName(e.KeyStatus.ScanCode, e.KeyStatus.IsExtendedKey);
        if (name == null || !name.StartsWith("mod:")) return;
        var modifier = name[4..];
        held.Remove(modifier);
        if (held.Count != 0 || tapCandidate != modifier)
        {
            tapCandidate = null;
            return;
        }
        tapCandidate = null;
        if (modifier.StartsWith("alt") || modifier.StartsWith("meta"))
        {
            Reject(AltWinReason);
            return;
        }
        pendingTap = modifier;
        tapTimer.Start();
    }
}
