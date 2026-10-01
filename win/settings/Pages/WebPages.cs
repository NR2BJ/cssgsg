using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Web.WebView2.Core;
using static Cssgsg.Settings.Lang;

namespace Cssgsg.Settings.Pages;

/// 배열 학습·타자 연습 페이지(맥과 같은 learn/index.html, practice/index.html). 웹 뷰는 앱이 뜰 때 하나씩 만들어 계속 쓴다
/// (탭을 누를 때마다 새로 만들면 다시 읽느라 잠깐 비고, 치던 판도 없어진다. 맥 0.5.0과 같다). 바깥 링크는 기본 브라우저로 연다.
/// 웹 뷰 데이터(캐시)는 설치 폴더가 아니라 %LOCALAPPDATA%\cssgsg\webview2에 둔다(Program Files에는 쓸 수 없다).
partial class WebPage : Grid
{
    protected readonly WebView2 View = new();
    readonly string file;
    readonly TextBlock message = new() { TextWrapping = TextWrapping.Wrap, Margin = new Thickness(28), Visibility = Visibility.Collapsed };

    protected WebPage(string relative)
    {
        file = Path.Combine(Paths.AppDir, relative);
        Children.Add(View);
        Children.Add(message);
        _ = StartAsync();
    }

    async Task StartAsync()
    {
        if (!File.Exists(file))
        {
            Show(T("페이지를 찾을 수 없습니다.", "The page is missing.", "ページが見つかりません。"));
            return;
        }
        try
        {
            await View.EnsureCoreWebView2Async();
        }
        catch (Exception)
        {
            // WebView2 런타임은 윈도우 11에 들어 있다. 없으면(일부 윈도우 10) 받아서 깔아야 한다.
            Show(T("페이지를 보이려면 Microsoft Edge WebView2 런타임이 필요합니다: https://go.microsoft.com/fwlink/p/?LinkId=2124703",
                "Showing this page needs the Microsoft Edge WebView2 Runtime: https://go.microsoft.com/fwlink/p/?LinkId=2124703",
                "このページの表示には Microsoft Edge WebView2 ランタイムが必要です: https://go.microsoft.com/fwlink/p/?LinkId=2124703"));
            return;
        }
        var core = View.CoreWebView2;
        core.Settings.IsStatusBarEnabled = false;
        core.Settings.AreDevToolsEnabled = false;
        core.Settings.IsZoomControlEnabled = true;
        var home = new Uri(file).AbsoluteUri;
        core.NewWindowRequested += (_, e) =>
        {
            e.Handled = true;
            if (e.Uri.StartsWith("http")) Shell.Open(e.Uri);
        };
        core.NavigationStarting += (_, e) =>
        {
            if (e.Uri.StartsWith("http"))
            {
                e.Cancel = true;
                Shell.Open(e.Uri);
            }
        };
        await Prepare(core);
        // 탭이 보이는 동안 다 읽혔으면 바로 키를 받게 한다.
        core.NavigationCompleted += (_, _) => TakeFocus();
        core.Navigate(home);
    }

    /// 페이지를 읽기 전에 할 일(타자 연습: 기록 다리).
    protected virtual Task Prepare(CoreWebView2 core) => Task.CompletedTask;

    void Show(string text)
    {
        message.Text = text;
        message.Visibility = Visibility.Visible;
        View.Visibility = Visibility.Collapsed;
    }

    bool showing;

    /// 탭이 보일 때(창이 다시 앞으로 올 때도) 키를 바로 받게 한다(클릭 없이 칠 수 있게). 탭 목록을 누른 뒤에는 목록이
    /// 포커스를 다시 가져가서, 맥처럼 조금 뒤에 한 번 더 준다. 타자 연습 페이지는 포커스가 없으면 "여기를 누르면 이어서
    /// 칩니다"를 띄운다(처음 사용자 확인: 탭을 처음 열면 그 안내가 떠 있었다).
    public void Shown()
    {
        showing = true;
        TakeFocus();
        DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, TakeFocus);
        var later = DispatcherQueue.CreateTimer();
        later.Interval = TimeSpan.FromMilliseconds(300);
        later.IsRepeating = false;
        later.Tick += (_, _) => TakeFocus();
        later.Start();
    }

    public void Hidden() => showing = false;

    void TakeFocus()
    {
        if (showing && View.CoreWebView2 != null) View.Focus(FocusState.Programmatic);
    }
}

sealed partial class LearnPage() : WebPage(Path.Combine("learn", "index.html"));

/// 타자 연습. 기록(끝낸 단계, 자주 틀린 키, 고른 단계)은 설정 파일 옆 practice.json에 남는다(맥과 같은 파일·내용).
/// 페이지는 맥 WebKit의 `window.webkit.messageHandlers.cssgsgPractice.postMessage({op})`(답이 Promise로 온다)를 부른다:
/// 같은 모양을 WebView2 메시지(chrome.webview)로 만들어 넣는다. 앱은 JSON 객체인지만 보고 그대로 파일에 옮겨 적는다.
sealed partial class PracticePage() : WebPage(Path.Combine("practice", "index.html"))
{
    const string Bridge = """
        (() => {
          const pending = new Map();
          let next = 0;
          window.chrome.webview.addEventListener("message", (e) => {
            const m = e.data;
            if (!m || typeof m.id !== "number" || !pending.has(m.id)) return;
            const p = pending.get(m.id);
            pending.delete(m.id);
            if (m.error) p.reject(new Error(m.error)); else p.resolve(m.result);
          });
          const handler = {
            postMessage(body) {
              return new Promise((resolve, reject) => {
                const id = ++next;
                pending.set(id, { resolve, reject });
                window.chrome.webview.postMessage({ id, body });
              });
            },
          };
          window.webkit = { messageHandlers: { cssgsgPractice: handler } };
        })();
        """;

    readonly SemaphoreSlim writing = new(1, 1);

    protected override async Task Prepare(CoreWebView2 core)
    {
        await core.AddScriptToExecuteOnDocumentCreatedAsync(Bridge);
        core.WebMessageReceived += (sender, e) => _ = Answer(core, e);
    }

    async Task Answer(CoreWebView2 core, CoreWebView2WebMessageReceivedEventArgs e)
    {
        if (!e.Source.StartsWith("file:")) return;
        JsonNode? message;
        try { message = JsonNode.Parse(e.WebMessageAsJson); }
        catch (Exception) { return; }
        var id = message?["id"]?.GetValue<int>() ?? 0;
        var op = message?["body"]?["op"]?.GetValue<string>();
        var reply = new JsonObject { ["id"] = id };
        switch (op)
        {
            case "load":
                string? text = null;
                try
                {
                    if (File.Exists(Paths.PracticeRecord)) text = await File.ReadAllTextAsync(Paths.PracticeRecord);
                }
                catch (Exception) { }
                reply["result"] = text;
                break;
            case "save":
                var data = message?["body"]?["data"]?.GetValue<string>();
                bool isObject;
                try { isObject = data != null && JsonNode.Parse(data) is JsonObject; }
                catch (Exception) { isObject = false; }
                if (!isObject)
                {
                    reply["error"] = "not a JSON object";
                    break;
                }
                reply["result"] = true;
                _ = Save(data!);
                break;
            default:
                reply["error"] = "op";
                break;
        }
        core.PostWebMessageAsJson(reply.ToJsonString());
    }

    async Task Save(string data)
    {
        await writing.WaitAsync();
        try
        {
            Directory.CreateDirectory(Paths.UserDir);
            var temp = Paths.PracticeRecord + ".tmp";
            await File.WriteAllTextAsync(temp, data);
            File.Move(temp, Paths.PracticeRecord, overwrite: true);
        }
        catch (Exception) { }
        finally
        {
            writing.Release();
        }
    }
}
