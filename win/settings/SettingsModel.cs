using System.Text;
using System.Text.Json.Nodes;

namespace Cssgsg.Settings;

/// 설정 화면의 상태(맥 SettingsModel과 같다). 파일이 원본이다: 바꿀 때마다 파일을 다시 읽고(직접 고친 것을 덮지 않게)
/// 그 위에 바꿔 쓴다. 엔진 호스트가 파일이 바뀐 것을 보고 입력기들에 나눠 준다(입력칸·앱을 바꿀 때 적용된다).
///
/// 설정은 코어 Config의 JSON 그대로 들고 고친다(JsonObject). 맥 표([mac])처럼 이 앱이 보이지 않는 설정도 그대로 지킨다.
sealed class SettingsModel
{
    public JsonObject Config { get; private set; }
    /// 설정 파일을 읽지 못한 까닭(직접 고치다 틀림). 그동안 입력기는 앞의 올바른 설정(처음이면 기본값)으로 돈다.
    public string? FileProblem { get; private set; }
    /// 마지막으로 쓰지 못한 까닭.
    public string? WriteProblem { get; private set; }

    /// 파일에서 다시 읽어 바뀌었거나 고쳐 썼다(화면을 맞춘다).
    public event Action? Changed;

    public static readonly JsonObject Defaults = LoadDefaults();

    static JsonObject LoadDefaults()
    {
        var (json, error) = ConfigLib.JsonFromToml(null);
        if (json == null) throw new InvalidOperationException($"설정 라이브러리가 기본 설정을 주지 못했다: {error}");
        return JsonNode.Parse(json)!.AsObject();
    }

    public SettingsModel()
    {
        Config = (JsonObject)Defaults.DeepClone();
        Reload();
    }

    /// 파일을 다시 읽는다(창이 앞으로 올 때, 바꾸기 전).
    public void Reload()
    {
        string? text = null;
        try
        {
            if (File.Exists(Paths.Config)) text = File.ReadAllText(Paths.Config, Encoding.UTF8);
        }
        catch (Exception e)
        {
            SetState(Config, e.Message, WriteProblem);
            return;
        }
        var (json, error) = ConfigLib.JsonFromToml(text);
        if (json != null)
            SetState(JsonNode.Parse(json)!.AsObject(), null, WriteProblem);
        else
            SetState((JsonObject)Defaults.DeepClone(), error, WriteProblem);
    }

    void SetState(JsonObject config, string? fileProblem, string? writeProblem)
    {
        var changed = !JsonNode.DeepEquals(config, Config) || fileProblem != FileProblem || writeProblem != WriteProblem;
        Config = config;
        FileProblem = fileProblem;
        WriteProblem = writeProblem;
        if (changed) Changed?.Invoke();
    }

    // ---- 값 읽기 ----

    static JsonNode? At(JsonObject root, string path)
    {
        JsonNode? node = root;
        foreach (var part in path.Split('.')) node = node?[part];
        return node;
    }

    public string Str(string path) => At(Config, path)?.GetValue<string>() ?? "";
    public int Int(string path) => At(Config, path)?.GetValue<int>() ?? 0;
    public bool Bool(string path) => At(Config, path)?.GetValue<bool>() ?? false;
    public static string DefaultStr(string path) => At(Defaults, path)?.GetValue<string>() ?? "";

    static void Put(JsonObject root, string path, JsonNode? value)
    {
        var parts = path.Split('.');
        var node = root;
        for (var i = 0; i < parts.Length - 1; i++)
        {
            if (node[parts[i]] is not JsonObject child)
            {
                child = new JsonObject();
                node[parts[i]] = child;
            }
            node = child;
        }
        node[parts[^1]] = value;
    }

    public void Set(string path, string value) => Update(c => Put(c, path, JsonValue.Create(value)));
    public void Set(string path, int value) => Update(c => Put(c, path, JsonValue.Create(value)));
    public void Set(string path, bool value) => Update(c => Put(c, path, JsonValue.Create(value)));

    // ---- 바꾸기 ----

    public static readonly string[] ShortcutFields = ["toggle_english", "toggle_non_english", "hanja"];

    /// 단축키 하나를 바꾼다. 받아 줄 수 없으면(다른 곳에 이미 쓰는 단축키 등) 까닭을 돌려주고 바꾸지 않는다.
    public string? SetShortcut(string field, string value)
    {
        Reload();
        if (value.Length > 0)
        {
            foreach (var other in ShortcutFields)
            {
                if (other != field && Str($"shortcuts.{other}") == value)
                {
                    var name = ShortcutText.ActionName(other);
                    return Lang.T($"이미 ‘{name}’에 쓰고 있는 단축키입니다.", $"Already used for “{name}”.",
                        $"すでに「{name}」で使っているショートカットです。");
                }
            }
        }
        var next = (JsonObject)Config.DeepClone();
        Put(next, $"shortcuts.{field}", JsonValue.Create(value));
        // 코어가 받아 주는지 먼저 본다. 설정 앱이 미리 거르지 못한 경우의 까닭은 코어의 말이다.
        var (_, error) = ConfigLib.TomlFromJson(next.ToJsonString());
        if (error != null) return error;
        Update(c => Put(c, $"shortcuts.{field}", JsonValue.Create(value)));
        return null;
    }

    public void Update(Action<JsonObject> change)
    {
        Reload();
        var next = (JsonObject)Config.DeepClone();
        change(next);
        if (JsonNode.DeepEquals(next, Config) && FileProblem == null) return;
        var (text, error) = ConfigLib.TomlFromJson(next.ToJsonString());
        if (text == null)
        {
            SetState(Config, FileProblem, error);
            return;
        }
        try
        {
            Write(text);
            SetState(next, null, null);
        }
        catch (Exception e)
        {
            SetState(Config, FileProblem, e.Message);
        }
    }

    void Write(string text)
    {
        Directory.CreateDirectory(Paths.UserDir);
        var path = Paths.Config;
        if (FileProblem != null && File.Exists(path))
        {
            // 틀린 파일은 지우지 않고 옆에 남긴다.
            File.Copy(path, path + ".bak", overwrite: true);
        }
        // 임시 파일에 쓰고 바꿔 넣는다(쓰다 끊겨도 앞 파일이 남는다). BOM 없는 UTF-8(코어의 TOML 읽기와 같다).
        var temp = path + ".tmp";
        File.WriteAllText(temp, text, new UTF8Encoding(false));
        File.Move(temp, path, overwrite: true);
    }

    /// 설정 파일을 탐색기에서 보인다. 없으면 지금 설정(기본값)으로 만든다.
    public void RevealConfigFile()
    {
        if (!File.Exists(Paths.Config))
        {
            var (text, _) = ConfigLib.TomlFromJson(Config.ToJsonString());
            if (text != null)
            {
                try { Write(text); } catch (Exception) { }
            }
        }
        Shell.Reveal(Paths.Config);
    }

    // ---- 한자 기억 ----

    public static int HanjaLearningCount()
    {
        try
        {
            if (!File.Exists(Paths.HanjaLearning)) return 0;
            return File.ReadLines(Paths.HanjaLearning, Encoding.UTF8).Count(l => l.Length > 0 && !l.StartsWith('#'));
        }
        catch (Exception)
        {
            return 0;
        }
    }
}
