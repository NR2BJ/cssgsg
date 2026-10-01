using System.Text;
using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using static Cssgsg.Settings.Lang;

namespace Cssgsg.Settings;

/// Mozc 사용자 사전(%LOCALAPPDATA%\cssgsg\mozc\user_dictionary.db, 맥 UserDictionary와 같다). 읽기·쓰기는 러스트 설정
/// 라이브러리(config-ffi/src/userdict.rs)가 한다. 첫 사전만 보이고 고친다(없으면 "User Dictionary"를 만든다).
/// 저장하면 엔진 호스트에 다시 읽으라고 한다(바로 적용).
sealed class UserDictionaryModel
{
    public sealed record Entry(string Key, string Value, string Comment, int Pos, string Locale = "")
    {
        /// 화면에서 고르기 위한 것. 파일에는 없다.
        public Guid Id { get; init; } = Guid.NewGuid();
    }

    public List<Entry> Entries { get; private set; } = [];
    /// 마지막으로 읽거나 쓰지 못한 까닭.
    public string? Problem { get; private set; }

    JsonObject storage = new() { ["dictionaries"] = new JsonArray() };
    DateTime? loadedStamp;

    static DateTime? Stamp => File.Exists(Paths.UserDictionary) ? File.GetLastWriteTimeUtc(Paths.UserDictionary) : null;

    public UserDictionaryModel() => Load();

    /// 파일이 바뀌었을 때만 다시 읽는다(창이 앞으로 올 때). 고르던 줄이 풀리지 않게.
    public bool ReloadIfChanged()
    {
        if (Stamp == loadedStamp) return false;
        Load();
        return true;
    }

    public void Load()
    {
        loadedStamp = Stamp;
        var (json, error) = ConfigLib.UserDictJson(Paths.UserDictionary);
        if (json == null)
        {
            Problem = error;
            return;
        }
        try
        {
            storage = JsonNode.Parse(json)!.AsObject();
            var first = (storage["dictionaries"] as JsonArray)?.FirstOrDefault() as JsonObject;
            Entries = (first?["entries"] as JsonArray)?.OfType<JsonObject>().Select(e => new Entry(
                e["key"]?.GetValue<string>() ?? "",
                e["value"]?.GetValue<string>() ?? "",
                e["comment"]?.GetValue<string>() ?? "",
                e["pos"]?.GetValue<int>() ?? 1,
                e["locale"]?.GetValue<string>() ?? "")).ToList() ?? [];
            Problem = null;
        }
        catch (Exception e)
        {
            Problem = e.Message;
        }
    }

    /// 읽기를 저장하는 모양으로(Mozc UserDictionaryUtil::NormalizeReading과 같은 쪽): 앞뒤 공백을 떼고,
    /// 반각 가타카나·전각 영숫자는 NFKC로 전각 가타카나·반각 영숫자로, 가타카나(ァ~ヶ)는 히라가나로 바꾼다.
    /// 장음 부호(ー)는 그대로 둔다.
    public static string NormalizedReading(string text)
    {
        var s = text.Trim().Normalize(NormalizationForm.FormKC);
        var b = new StringBuilder(s.Length);
        foreach (var c in s) b.Append(c is >= 'ァ' and <= 'ヶ' ? (char)(c - 0x60) : c);
        return b.ToString();
    }

    /// 넣거나 고칠 낱말이 쓸 만한지. 쓸 만하면 null.
    public string? ProblemWith(Entry entry)
    {
        string[] fields = [entry.Key, entry.Value, entry.Comment];
        if (entry.Key.Length == 0 || entry.Value.Length == 0)
            return T("읽기와 단어를 입력하세요.", "Enter a reading and a word.", "読みと単語を入力してください。");
        if (fields.Any(f => f.EnumerateRunes().Count() > 300))
            return T("300자까지 입력할 수 있습니다.", "Up to 300 characters.", "300 文字まで入力できます。");
        if (fields.Any(f => f.Contains('\t') || f.Contains('\n') || f.Contains('\r')))
            return T("탭이나 줄바꿈은 쓸 수 없습니다.", "Tabs and line breaks aren’t allowed.", "タブや改行は使えません。");
        if (Entries.Any(e => e.Id != entry.Id && e.Key == entry.Key && e.Value == entry.Value && e.Pos == entry.Pos))
            return T("같은 단어가 이미 있습니다.", "The same word is already there.", "同じ単語がすでにあります。");
        return null;
    }

    /// 넣거나(같은 Id가 없으면) 고친다. 저장하지 못하면 까닭.
    public string? Upsert(Entry entry)
    {
        var next = Entries.ToList();
        var i = next.FindIndex(e => e.Id == entry.Id);
        if (i >= 0) next[i] = entry;
        else next.Add(entry);
        return Write(next);
    }

    public string? Remove(IReadOnlySet<Guid> ids) => Write(Entries.Where(e => !ids.Contains(e.Id)).ToList());

    string? Write(List<Entry> entries)
    {
        var next = (JsonObject)storage.DeepClone();
        if (next["dictionaries"] is not JsonArray dictionaries)
        {
            dictionaries = [];
            next["dictionaries"] = dictionaries;
        }
        if (dictionaries.Count == 0)
            dictionaries.Add((JsonNode)new JsonObject { ["id"] = "", ["name"] = "User Dictionary", ["entries"] = new JsonArray() });
        var list = new JsonArray();
        foreach (var e in entries)
        {
            list.Add((JsonNode)new JsonObject
            {
                ["key"] = e.Key,
                ["value"] = e.Value,
                ["comment"] = e.Comment,
                ["pos"] = e.Pos,
                ["locale"] = e.Locale,
            });
        }
        dictionaries[0]!["entries"] = list;
        try
        {
            Directory.CreateDirectory(Paths.MozcProfile);
        }
        catch (Exception ex)
        {
            return Problem = ex.Message;
        }
        var error = ConfigLib.UserDictSave(Paths.UserDictionary, next.ToJsonString());
        if (error != null) return Problem = error;
        // 새 사전의 id는 저장할 때 매겨지므로 다시 읽는다. 줄 순서는 그대로라 화면의 Id를 이어 붙인다.
        var ids = entries.Select(e => e.Id).ToList();
        Load();
        if (Entries.Count == ids.Count) Entries = Entries.Select((e, n) => e with { Id = ids[n] }).ToList();
        _ = HostClient.ReloadAsync();
        return null;
    }
}

/// Mozc 품사(user_dictionary_storage.proto PosType). 고르는 목록은 NRIME와 같은 18개(맥 MozcPOS와 같다).
static class MozcPos
{
    public static readonly int[] Choices = [1, 4, 5, 6, 7, 8, 9, 2, 3, 14, 15, 16, 18, 19, 20, 21, 40, 44];

    static readonly Dictionary<int, string> Japanese = new()
    {
        [1] = "名詞", [2] = "短縮よみ", [3] = "サジェストのみ", [4] = "固有名詞", [5] = "人名", [6] = "姓", [7] = "名", [8] = "組織",
        [9] = "地名", [10] = "名詞サ変", [11] = "名詞形動", [12] = "数", [13] = "アルファベット", [14] = "記号", [15] = "顔文字",
        [16] = "副詞", [17] = "連体詞", [18] = "接続詞", [19] = "感動詞", [20] = "接頭語", [21] = "助数詞", [22] = "接尾一般",
        [23] = "接尾人名", [24] = "接尾地名", [25] = "動詞ワ行五段", [26] = "動詞カ行五段", [27] = "動詞サ行五段", [28] = "動詞タ行五段",
        [29] = "動詞ナ行五段", [30] = "動詞マ行五段", [31] = "動詞ラ行五段", [32] = "動詞ガ行五段", [33] = "動詞バ行五段",
        [34] = "動詞ハ行四段", [35] = "動詞一段", [36] = "動詞カ変", [37] = "動詞サ変", [38] = "動詞ザ変", [39] = "動詞ラ変",
        [40] = "形容詞", [41] = "終助詞", [42] = "句読点", [43] = "独立語", [44] = "抑制単語",
    };

    static readonly Dictionary<int, string> Korean = new()
    {
        [1] = "명사", [2] = "줄인 읽기", [3] = "추천에만", [4] = "고유명사", [5] = "사람 이름", [6] = "성", [7] = "이름", [8] = "조직",
        [9] = "지명", [14] = "기호", [15] = "이모티콘", [16] = "부사", [18] = "접속사", [19] = "감탄사", [20] = "접두어",
        [21] = "조수사", [40] = "형용사", [44] = "억제 단어",
    };

    static readonly Dictionary<int, string> English = new()
    {
        [1] = "Noun", [2] = "Abbreviation", [3] = "Suggestion only", [4] = "Proper noun", [5] = "Person name",
        [6] = "Family name", [7] = "Given name", [8] = "Organization", [9] = "Place name", [14] = "Symbol", [15] = "Emoticon",
        [16] = "Adverb", [18] = "Conjunction", [19] = "Interjection", [20] = "Prefix", [21] = "Counter", [40] = "Adjective",
        [44] = "Suppressed word",
    };

    public static string Name(int pos)
    {
        var ja = Japanese.GetValueOrDefault(pos, pos.ToString());
        return Active switch
        {
            UiLanguage.Ja => ja,
            UiLanguage.En => English.TryGetValue(pos, out var en) ? $"{en} {ja}" : ja,
            _ => Korean.TryGetValue(pos, out var ko) ? $"{ko} {ja}" : ja,
        };
    }
}

/// 일본어 탭의 개인 사전 칸: 검색, 목록(읽기·단어·품사·메모), 추가·편집·삭제.
sealed partial class UserDictionaryView : StackPanel
{
    readonly UserDictionaryModel dictionary;
    readonly TextBox search = new() { PlaceholderText = T("검색", "Search", "検索"), MinWidth = 240 };
    readonly ListView list = new() { SelectionMode = ListViewSelectionMode.Extended, Height = 260 };
    readonly TextBlock count = new() { VerticalAlignment = VerticalAlignment.Center };
    readonly Button edit, delete;
    readonly TextBlock problem = new() { TextWrapping = TextWrapping.Wrap };

    public UserDictionaryView(UserDictionaryModel dictionary)
    {
        this.dictionary = dictionary;
        Spacing = 8;
        Padding = new Thickness(0, 10, 0, 10);
        problem.Foreground = Ui.Brush("SystemFillColorCautionBrush");

        var add = Ui.Button(T("추가…", "Add…", "追加…"), () => _ = EditAsync(null));
        var top = new Grid { ColumnSpacing = 8 };
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        top.Children.Add(search);
        Grid.SetColumn(add, 1);
        top.Children.Add(add);
        Children.Add(top);

        Children.Add(Columns(T("읽기", "Reading", "読み"), T("단어", "Word", "単語"), T("품사", "Part of Speech", "品詞"),
            T("메모", "Note", "メモ"), header: true));
        list.BorderBrush = Ui.Brush("CardStrokeColorDefaultBrush");
        list.BorderThickness = new Thickness(1);
        list.CornerRadius = new CornerRadius(4);
        Children.Add(list);

        edit = Ui.Button(T("편집…", "Edit…", "編集…"), () => _ = EditAsync(Selected().FirstOrDefault()));
        delete = Ui.Button(T("삭제…", "Delete…", "削除…"), () => _ = DeleteAsync());
        var bottom = new Grid { ColumnSpacing = 8 };
        bottom.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        bottom.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        bottom.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        bottom.Children.Add(count);
        Grid.SetColumn(edit, 1);
        Grid.SetColumn(delete, 2);
        bottom.Children.Add(edit);
        bottom.Children.Add(delete);
        Children.Add(bottom);
        Children.Add(problem);

        search.TextChanged += (_, _) => Fill();
        list.SelectionChanged += (_, _) => Buttons();
        list.DoubleTapped += (_, _) => _ = EditAsync(Selected().FirstOrDefault());
        Fill();
    }

    static Grid Columns(string a, string b, string c, string d, bool header)
    {
        var g = new Grid { ColumnSpacing = 12, Padding = header ? new Thickness(14, 0, 14, 0) : new Thickness(0) };
        foreach (var w in new[] { 2.0, 2.0, 2.2, 2.0 })
            g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(w, GridUnitType.Star) });
        string[] texts = [a, b, c, d];
        for (var i = 0; i < 4; i++)
        {
            var t = new TextBlock { Text = texts[i], TextTrimming = TextTrimming.CharacterEllipsis };
            if (header)
            {
                t.Style = Ui.Style("CaptionTextBlockStyle");
                t.Foreground = Ui.Brush("TextFillColorSecondaryBrush");
            }
            else if (i >= 2)
            {
                t.Foreground = Ui.Brush("TextFillColorSecondaryBrush");
            }
            Grid.SetColumn(t, i);
            g.Children.Add(t);
        }
        return g;
    }

    IEnumerable<UserDictionaryModel.Entry> Shown()
    {
        var q = search.Text.Trim();
        if (q.Length == 0) return dictionary.Entries;
        var reading = UserDictionaryModel.NormalizedReading(q);
        return dictionary.Entries.Where(e => e.Key.Contains(reading) ||
            e.Value.Contains(q, StringComparison.CurrentCultureIgnoreCase) ||
            e.Comment.Contains(q, StringComparison.CurrentCultureIgnoreCase));
    }

    /// 목록을 다시 채운다. `keep`에 든 줄은 다시 고른다.
    public void Fill(IReadOnlySet<Guid>? keep = null)
    {
        keep ??= Selected().Select(e => e.Id).ToHashSet();
        list.Items.Clear();
        foreach (var e in Shown())
        {
            var row = Columns(e.Key, e.Value, MozcPos.Name(e.Pos), e.Comment, header: false);
            var item = new ListViewItem { Content = row, Tag = e.Id };
            AutomationProperties.SetName(item, $"{e.Key} {e.Value}");
            list.Items.Add(item);
        }
        foreach (var item in list.Items.OfType<ListViewItem>())
        {
            if (item.Tag is Guid id && keep.Contains(id)) list.SelectedItems.Add(item);
        }
        var n = dictionary.Entries.Count;
        count.Text = T($"{n}개", $"{n} words", $"{n} 語");
        problem.Text = dictionary.Problem ?? "";
        problem.Visibility = dictionary.Problem == null ? Visibility.Collapsed : Visibility.Visible;
        Buttons();
    }

    void Buttons()
    {
        var n = list.SelectedItems.Count;
        edit.IsEnabled = n == 1;
        delete.IsEnabled = n > 0;
    }

    List<UserDictionaryModel.Entry> Selected()
    {
        var ids = list.SelectedItems.OfType<ListViewItem>().Select(i => i.Tag).OfType<Guid>().ToHashSet();
        return dictionary.Entries.Where(e => ids.Contains(e.Id)).ToList();
    }

    async Task DeleteAsync()
    {
        var ids = Selected().Select(e => e.Id).ToHashSet();
        if (ids.Count == 0) return;
        if (!await Ui.Confirm(XamlRoot, T($"단어 {ids.Count}개를 삭제하시겠습니까?", $"Delete {ids.Count} word(s)?", $"{ids.Count} 語を削除しますか？"),
                T("삭제", "Delete", "削除")))
            return;
        dictionary.Remove(ids);
        Fill(new HashSet<Guid>());
    }

    /// 낱말 하나 넣기·고치기(맥 UserDictEntryEditor).
    async Task EditAsync(UserDictionaryModel.Entry? original)
    {
        var isNew = original == null;
        original ??= new UserDictionaryModel.Entry("", "", "", 1);
        var reading = new TextBox { Header = T("읽기", "Reading", "読み"), PlaceholderText = "くもつ", Text = original.Key };
        var word = new TextBox { Header = T("단어", "Word", "単語"), PlaceholderText = "雲津", Text = original.Value };
        var pos = new ComboBox { Header = T("품사", "Part of Speech", "品詞"), HorizontalAlignment = HorizontalAlignment.Stretch };
        var choices = MozcPos.Choices.Contains(original.Pos) ? MozcPos.Choices : [.. MozcPos.Choices, original.Pos];
        foreach (var p in choices) pos.Items.Add(MozcPos.Name(p));
        pos.SelectedIndex = Array.IndexOf(choices, original.Pos);
        var note = new TextBox { Header = T("메모", "Note", "メモ"), Text = original.Comment };
        var error = new TextBlock { Foreground = Ui.Brush("SystemFillColorCautionBrush"), TextWrapping = TextWrapping.Wrap };
        var form = new StackPanel { Spacing = 12, MinWidth = 360, Children = { reading, word, pos, note, error } };
        var dialog = Ui.Dialog(XamlRoot, isNew ? T("단어 추가", "Add Word", "単語を追加") : T("단어 편집", "Edit Word", "単語を編集"),
            form, T("저장", "Save", "保存"), T("취소", "Cancel", "キャンセル"));
        dialog.DefaultButton = ContentDialogButton.Primary;
        UserDictionaryModel.Entry? saved = null;
        dialog.PrimaryButtonClick += (_, args) =>
        {
            var entry = original with
            {
                Key = UserDictionaryModel.NormalizedReading(reading.Text),
                Value = word.Text.Trim(),
                Comment = note.Text.Trim(),
                Pos = choices[Math.Max(0, pos.SelectedIndex)],
            };
            var reason = dictionary.ProblemWith(entry) ?? dictionary.Upsert(entry);
            if (reason != null)
            {
                error.Text = reason;
                args.Cancel = true;
                return;
            }
            saved = entry;
        };
        await dialog.ShowAsync();
        if (saved != null) Fill(new HashSet<Guid> { saved.Id });
    }
}
