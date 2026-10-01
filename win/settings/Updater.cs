using System.ComponentModel;
using System.Diagnostics;
using System.Net;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text.Json.Nodes;
using Microsoft.Win32;

namespace Cssgsg.Settings;

/// 앱 업데이트(맥 Updater의 윈도우판). GitHub 릴리스 중 태그가 win-v로 시작하고 cssgsg-setup.exe가 붙은 것을 본다
/// (같은 저장소에 맥 릴리스와 Mozc 엔진 릴리스가 섞여 있어서 목록을 받아 거른다. 윈도우 릴리스는 "최신"으로 두지 않는다).
/// - 정식 채널은 앞판(prerelease)을 권하지 않고, 베타 채널은 권한다.
/// - 확인은 정보 탭을 열 때(1분이 지났으면), "지금 확인", 채널을 바꿀 때. 바뀐 게 없으면 GitHub는 304만 준다(ETag).
/// - 받은 설치기는 GitHub가 계산한 SHA-256(digest)과 같을 때만 돌린다(관리자 권한으로 돈다). 해시가 없으면 돌리지 않는다.
/// - 설치기는 조용히(/SILENT) 돌고 끝나면 이 앱을 정보 탭으로 다시 띄운다(/RELAUNCH=1). 이 앱은 설치기를 띄우고 끝낸다.
sealed class Updater
{
    public enum Phase { Idle, Checking, UpToDate, Available, Downloading, Installing, Failed }

    public sealed record Release(string Version, string? Notes, string? Page, string Url, long Size, string? Digest);

    public Phase State { get; private set; } = Phase.Idle;
    public Release? Offer { get; private set; }
    public double Progress { get; private set; }
    public string? Problem { get; private set; }
    public DateTimeOffset? LastCheck { get; private set; }

    /// 상태가 바뀌었다(UI 스레드에서 부른다).
    public event Action? Changed;

    const string Api = "https://api.github.com/repos/NR2BJ/cssgsg/releases?per_page=100";
    const string TagPrefix = "win-v";
    const string Asset = "cssgsg-setup.exe";
    const string Key = @"Software\cssgsg\Update";
    static readonly TimeSpan CheckInterval = TimeSpan.FromMinutes(1);

    static readonly HttpClient Http = MakeClient();

    static HttpClient MakeClient()
    {
        var client = new HttpClient { Timeout = TimeSpan.FromSeconds(20) };
        // 이메일 같은 개인 정보는 넣지 않는다.
        client.DefaultRequestHeaders.UserAgent.ParseAdd($"cssgsg-settings/{BuildInfo.Version} (+https://github.com/NR2BJ/cssgsg)");
        client.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json");
        return client;
    }

    public bool Beta
    {
        get => ReadString("Channel") == "beta";
        set
        {
            if (value == Beta || Busy) return;
            WriteString("Channel", value ? "beta" : "stable");
            State = Phase.Idle;
            Offer = null;
            Changed?.Invoke();
            _ = CheckAsync(userInitiated: true);
        }
    }

    public bool Busy => State is Phase.Checking or Phase.Downloading or Phase.Installing;

    public Updater()
    {
        LastCheck = ReadTime();
    }

    // ---- 확인 ----

    /// 정보 탭을 열 때: 1분이 지났으면 확인하고, 아니면 지난번 응답으로 상태를 보인다.
    public void CheckIfDue()
    {
        if (Busy) return;
        if (LastCheck is { } last && DateTimeOffset.Now - last < CheckInterval)
        {
            if (State == Phase.Idle) Evaluate(Cached());
            return;
        }
        _ = CheckAsync(userInitiated: false);
    }

    /// 사람이 누른 확인이면 네트워크 실패도 보인다. 자동 확인의 실패는 조용히 넘긴다.
    public async Task CheckAsync(bool userInitiated)
    {
        if (Busy) return;
        var beta = Beta;
        Set(Phase.Checking);
        try
        {
            using var request = new HttpRequestMessage(HttpMethod.Get, Api);
            var etag = ReadString(Channel(beta) + ".ETag");
            if (etag != null && Cached() != null) request.Headers.IfNoneMatch.Add(EntityTagHeaderValue.Parse(etag));
            using var response = await Http.SendAsync(request);
            if (beta != Beta) return;  // 그새 채널을 바꿨다
            LastCheck = DateTimeOffset.Now;
            WriteString("LastCheck", LastCheck.Value.ToUnixTimeSeconds().ToString());
            switch (response.StatusCode)
            {
                case HttpStatusCode.NotModified:
                    Evaluate(Cached());
                    break;
                case HttpStatusCode.OK:
                    var body = await response.Content.ReadAsStringAsync();
                    var releases = Decode(body);
                    if (releases == null)
                    {
                        Fail(Lang.T("릴리스 정보를 읽지 못했습니다", "Couldn’t read the release info", "リリース情報を読めませんでした"));
                        return;
                    }
                    // 본문을 저장한 뒤에만 ETag를 남긴다(304가 늘 저장한 응답을 가리키게).
                    Directory.CreateDirectory(Paths.Updates);
                    File.WriteAllText(CacheFile(beta), body);
                    if (response.Headers.ETag != null) WriteString(Channel(beta) + ".ETag", response.Headers.ETag.ToString());
                    Evaluate(releases);
                    break;
                default:
                    if (userInitiated)
                        Fail(Lang.T($"GitHub가 {(int)response.StatusCode}로 응답했습니다", $"GitHub responded {(int)response.StatusCode}",
                            $"GitHub が {(int)response.StatusCode} を返しました"));
                    else
                        Set(Phase.Idle);
                    break;
            }
        }
        catch (Exception)
        {
            if (userInitiated)
                Fail(Lang.T("서버에 연결하지 못했습니다", "Couldn’t reach the server", "サーバーに接続できませんでした"));
            else
                Set(Phase.Idle);
        }
    }

    static string Channel(bool beta) => beta ? "beta" : "stable";
    static string CacheFile(bool beta) => Path.Combine(Paths.Updates, $"releases-{Channel(beta)}.json");

    List<JsonObject>? Cached()
    {
        try
        {
            var file = CacheFile(Beta);
            return File.Exists(file) ? Decode(File.ReadAllText(file)) : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    static List<JsonObject>? Decode(string body)
    {
        try
        {
            return JsonNode.Parse(body) is JsonArray list ? [.. list.OfType<JsonObject>()] : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    void Evaluate(List<JsonObject>? releases)
    {
        if (releases == null)
        {
            Set(Phase.Idle);
            return;
        }
        Offer = Pick(releases, BuildInfo.Version, Beta);
        Set(Offer != null ? Phase.Available : Phase.UpToDate);
    }

    /// 권할 릴리스 중 가장 높은 버전: 초안이 아니고, 윈도우 태그에 설치기가 붙었고, 지금보다 높다. 정식은 앞판 빼고.
    public static Release? Pick(IEnumerable<JsonObject> releases, string current, bool beta)
    {
        Release? best = null;
        SemVer? bestVersion = null;
        foreach (var r in releases)
        {
            var tag = r["tag_name"]?.GetValue<string>() ?? "";
            if (!tag.StartsWith(TagPrefix) || r["draft"]?.GetValue<bool>() == true) continue;
            if (!beta && r["prerelease"]?.GetValue<bool>() == true) continue;
            var version = tag[TagPrefix.Length..];
            var asset = (r["assets"] as JsonArray)?.OfType<JsonObject>()
                .FirstOrDefault(a => a["name"]?.GetValue<string>() == Asset);
            var url = asset?["browser_download_url"]?.GetValue<string>();
            if (url == null || !SemVer.IsNewer(version, current)) continue;
            var v = SemVer.Parse(version)!;
            if (bestVersion != null && v.CompareTo(bestVersion) <= 0) continue;
            bestVersion = v;
            best = new Release(version, r["body"]?.GetValue<string>(), r["html_url"]?.GetValue<string>(), url,
                asset!["size"]?.GetValue<long>() ?? 0, asset["digest"]?.GetValue<string>());
        }
        return best;
    }

    // ---- 설치 ----

    public async Task InstallAsync()
    {
        if (State != Phase.Available || Offer is not { } release) return;
        Progress = 0;
        Set(Phase.Downloading);
        var file = Path.Combine(Paths.Updates, $"cssgsg-setup-{Safe(release.Version)}.exe");
        try
        {
            Directory.CreateDirectory(Paths.Updates);
            using var response = await Http.GetAsync(release.Url, HttpCompletionOption.ResponseHeadersRead);
            response.EnsureSuccessStatusCode();
            var total = response.Content.Headers.ContentLength ?? release.Size;
            await using (var source = await response.Content.ReadAsStreamAsync())
            await using (var target = File.Create(file))
            {
                var buffer = new byte[1 << 16];
                long done = 0;
                int n;
                while ((n = await source.ReadAsync(buffer)) > 0)
                {
                    await target.WriteAsync(buffer.AsMemory(0, n));
                    done += n;
                    if (total > 0)
                    {
                        Progress = (double)done / total;
                        Changed?.Invoke();
                    }
                }
            }
        }
        catch (Exception)
        {
            Fail(Lang.T("내려받지 못했습니다", "Download failed", "ダウンロードに失敗しました"));
            return;
        }
        // 관리자 권한으로 돌릴 파일이다. 해시가 없거나 다르면 돌리지 않는다.
        if (!MatchesDigest(file, release.Digest))
        {
            try { File.Delete(file); } catch (Exception) { }
            Fail(Lang.T("받은 파일의 해시가 맞지 않습니다", "The download’s checksum doesn’t match", "ダウンロードのハッシュが一致しません"));
            return;
        }
        Set(Phase.Installing);
        try
        {
            // 설치기는 관리자 권한을 청한다(UAC). 끝나면 이 앱을 정보 탭으로 다시 띄운다.
            Process.Start(new ProcessStartInfo(file, "/SILENT /SUPPRESSMSGBOXES /NORESTART /RELAUNCH=1") { UseShellExecute = true });
            Microsoft.UI.Xaml.Application.Current.Exit();
        }
        catch (Win32Exception)
        {
            Fail(Lang.T("설치를 취소했거나 설치하지 못했습니다", "Installation was cancelled or failed", "インストールが取り消されたか、失敗しました"));
        }
    }

    static bool MatchesDigest(string file, string? digest)
    {
        if (digest == null || !digest.StartsWith("sha256:", StringComparison.OrdinalIgnoreCase)) return false;
        try
        {
            using var stream = File.OpenRead(file);
            var hex = Convert.ToHexStringLower(SHA256.HashData(stream));
            return hex == digest["sha256:".Length..].ToLowerInvariant();
        }
        catch (Exception)
        {
            return false;
        }
    }

    /// 파일 이름에 넣어도 되는 글자(ASCII 영숫자, ".", "-")만 남긴다.
    static string Safe(string version) => new([.. version.Where(c => char.IsAsciiLetterOrDigit(c) || c is '.' or '-')]);

    void Set(Phase phase)
    {
        State = phase;
        if (phase != Phase.Failed) Problem = null;
        Changed?.Invoke();
    }

    void Fail(string reason)
    {
        Problem = reason;
        Set(Phase.Failed);
    }

    // ---- 저장(HKCU\Software\cssgsg\Update) ----

    static string? ReadString(string name)
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(Key);
            return key?.GetValue(name) as string;
        }
        catch (Exception)
        {
            return null;
        }
    }

    static void WriteString(string name, string value)
    {
        try
        {
            using var key = Registry.CurrentUser.CreateSubKey(Key);
            key.SetValue(name, value);
        }
        catch (Exception) { }
    }

    static DateTimeOffset? ReadTime() =>
        long.TryParse(ReadString("LastCheck"), out var t) ? DateTimeOffset.FromUnixTimeSeconds(t).ToLocalTime() : null;
}
