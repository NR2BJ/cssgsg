using System.IO.Pipes;
using System.Security.Principal;
using System.Text;
using System.Text.Json.Nodes;

namespace Cssgsg.Settings;

/// 엔진 호스트(cssgsg-host.exe)에 묻는다. 입력기와 같은 파이프·메시지(win/ipc: 메시지 모드, JSON 한 줄)를 쓴다.
/// 호스트가 한자 기억과 Mozc 학습 파일을 쥐고 있어서, 지우기와 사용자 사전 다시 읽기는 호스트에 시킨다.
static class HostClient
{
    public enum Status { Done, NoHost, Refused, Timeout, Failed }

    public readonly record struct Reply(Status Status, JsonNode? Body, string? Message)
    {
        public bool Ok => Status == Status.Done;
    }

    /// win/ipc/src/lib.rs PROTOCOL.
    const int Protocol = 1;

    static string PipeName => $"cssgsg-engine-{WindowsIdentity.GetCurrent().User?.Value}";

    /// 요청 하나를 보내고 답 하나를 받는다(다른 스레드에서). 호스트가 없거나 늦으면 그 까닭.
    public static Task<Reply> CallAsync(string request, int timeoutMs = 3000) => Task.Run(() => Call(request, timeoutMs));

    static Reply Call(string request, int timeoutMs)
    {
        using var pipe = new NamedPipeClientStream(".", PipeName, PipeDirection.InOut, PipeOptions.Asynchronous,
            TokenImpersonationLevel.Identification);
        try
        {
            pipe.Connect(Math.Min(timeoutMs, 1000));
            pipe.ReadMode = PipeTransmissionMode.Message;
        }
        catch (Exception)
        {
            return new(Status.NoHost, null, null);
        }
        // 이 사용자의 호스트인지(다른 사용자가 같은 이름의 파이프를 먼저 만들었으면 쓰지 않는다).
        try
        {
            var owner = pipe.GetAccessControl().GetOwner(typeof(SecurityIdentifier));
            if (owner == null || !owner.Equals(WindowsIdentity.GetCurrent().User))
                return new(Status.Refused, null, null);
        }
        catch (Exception)
        {
            return new(Status.Refused, null, null);
        }
        try
        {
            var bytes = Encoding.UTF8.GetBytes(request);
            using var cancel = new CancellationTokenSource(timeoutMs);
            pipe.WriteAsync(bytes, cancel.Token).AsTask().Wait(cancel.Token);
            var reply = ReadMessage(pipe, cancel.Token);
            return Parse(reply);
        }
        catch (OperationCanceledException)
        {
            return new(Status.Timeout, null, null);
        }
        catch (AggregateException e) when (e.InnerException is OperationCanceledException)
        {
            return new(Status.Timeout, null, null);
        }
        catch (Exception e)
        {
            return new(Status.Failed, null, e.Message);
        }
    }

    static string ReadMessage(NamedPipeClientStream pipe, CancellationToken cancel)
    {
        var all = new MemoryStream();
        var buffer = new byte[64 * 1024];
        do
        {
            var n = pipe.ReadAsync(buffer, cancel).AsTask().GetAwaiter().GetResult();
            if (n == 0) break;
            all.Write(buffer, 0, n);
        } while (!pipe.IsMessageComplete);
        return Encoding.UTF8.GetString(all.ToArray());
    }

    /// serde의 바깥 꼬리표 모양: "Done", {"Hello":{…}}, {"Error":{"message":…}}.
    static Reply Parse(string text)
    {
        var node = JsonNode.Parse(text);
        if (node is JsonObject obj && obj.Count == 1)
        {
            var (name, body) = obj.First();
            return name == "Error"
                ? new(Status.Failed, body, body?["message"]?.GetValue<string>())
                : new(Status.Done, obj, null);
        }
        return node?.GetValueKind() == System.Text.Json.JsonValueKind.String && node.GetValue<string>() is "Done"
            ? new(Status.Done, node, null)
            : new(Status.Failed, node, text);
    }

    /// 호스트가 떠 있으면 Mozc 버전(엔진을 읽지 못했으면 ""), 없으면 null.
    public static async Task<string?> EngineVersionAsync()
    {
        var reply = await CallAsync($"{{\"Hello\":{{\"protocol\":{Protocol}}}}}", 1500);
        if (!reply.Ok) return null;
        return reply.Body?["Hello"]?["engine"]?.GetValue<string>() ?? "";
    }

    public static Task<Reply> ReloadAsync() => CallAsync("\"Reload\"");
    public static Task<Reply> ClearHanjaLearningAsync() => CallAsync("\"ClearHanjaLearning\"");
    /// 엔진을 내리고 다시 읽어서 조금 걸린다.
    public static Task<Reply> ClearMozcLearningAsync() => CallAsync("\"ClearMozcLearning\"", 15000);

    /// 호스트를 끝낸다(학습을 마무리하고). 파이프가 없어질 때까지 3초까지 기다린다.
    public static async Task<bool> QuitAsync()
    {
        var reply = await CallAsync("\"Quit\"", 2000);
        if (reply.Status == Status.NoHost) return true;
        var began = Environment.TickCount64;
        while (Environment.TickCount64 - began < 3000)
        {
            if (await EngineVersionAsync() == null) return true;
            await Task.Delay(50);
        }
        return false;
    }
}
