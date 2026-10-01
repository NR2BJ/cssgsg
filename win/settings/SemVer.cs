namespace Cssgsg.Settings;

/// "1.0.9", "1.0.9-beta.2" 같은 버전(맥 SemanticVersion, NRIME에서 가져온 것과 같은 순서).
/// 같은 숫자면 정식이 앞판보다 높고(1.0.9 > 1.0.9-beta.2), 앞판 꼬리는 왼쪽부터 견주며 숫자는 숫자로 견준다.
sealed class SemVer : IComparable<SemVer>
{
    readonly int[] core;
    readonly string[] prerelease;

    SemVer(int[] core, string[] prerelease)
    {
        this.core = core;
        this.prerelease = prerelease;
    }

    /// 앞의 "v"를 떼고 빌드 꼬리("+001")는 버린다. 숫자가 없으면 null.
    public static SemVer? Parse(string raw)
    {
        var text = raw.Trim();
        if (text.StartsWith('v') || text.StartsWith('V')) text = text[1..];
        var plus = text.IndexOf('+');
        if (plus >= 0) text = text[..plus];
        var dash = text.IndexOf('-');
        var coreText = dash >= 0 ? text[..dash] : text;
        var pre = dash >= 0 ? text[(dash + 1)..] : "";
        var numbers = new List<int>();
        foreach (var part in coreText.Split('.'))
        {
            if (int.TryParse(part, out var n)) numbers.Add(n);
        }
        if (numbers.Count == 0) return null;
        return new SemVer([.. numbers], pre.Length == 0 ? [] : pre.Split('.'));
    }

    public int CompareTo(SemVer? other)
    {
        if (other == null) return 1;
        var count = Math.Max(core.Length, other.core.Length);
        for (var i = 0; i < count; i++)
        {
            var a = i < core.Length ? core[i] : 0;
            var b = i < other.core.Length ? other.core[i] : 0;
            if (a != b) return a.CompareTo(b);
        }
        return (prerelease.Length > 0, other.prerelease.Length > 0) switch
        {
            (false, false) => 0,
            (true, false) => -1,
            (false, true) => 1,
            _ => ComparePrerelease(prerelease, other.prerelease),
        };
    }

    static int ComparePrerelease(string[] a, string[] b)
    {
        var count = Math.Min(a.Length, b.Length);
        for (var i = 0; i < count; i++)
        {
            if (a[i] == b[i]) continue;
            var an = int.TryParse(a[i], out var x);
            var bn = int.TryParse(b[i], out var y);
            if (an && bn) return x.CompareTo(y);
            if (an) return -1;  // 숫자 꼬리가 낮다
            if (bn) return 1;
            return string.CompareOrdinal(a[i], b[i]);
        }
        return a.Length.CompareTo(b.Length);
    }

    /// `remote`가 `current`보다 새 버전인지. 읽을 수 없는 버전은 새 버전이 아니다(잘못된 태그로 받지 않게).
    public static bool IsNewer(string remote, string current)
    {
        var r = Parse(remote);
        var c = Parse(current);
        return r != null && c != null && r.CompareTo(c) > 0;
    }
}
