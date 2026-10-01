import SwiftUI
import WebKit

/// 타자 연습 페이지(practice/index.html, tools/practice/build.mjs가 엔진 데이터로 만든 것). 창을 키우거나 전체 화면으로 쓴다.
/// 기록(끝낸 단계, 자주 틀린 키, 고른 단계)은 설정 파일 옆 practice.json에 남는다. 같은 파일을 브라우저로 열면
/// 그 브라우저의 localStorage에 남는다.
struct PracticeTab: View {
    var body: some View {
        if PracticePage.shared.pageURL != nil {
            PracticeWebViewHost()
        } else {
            Text(tr("타자 연습 페이지를 찾을 수 없습니다.", "The typing practice page is missing.",
                    "タイピング練習ページが見つかりません。"))
                .foregroundStyle(.secondary)
        }
    }
}

/// 웹 뷰 하나를 앱이 뜰 때 만들어 계속 쓴다(배열 학습과 같다: 탭을 누를 때마다 다시 읽지 않고, 치던 판도 남는다).
/// 페이지는 `cssgsgPractice` 메시지로 기록을 읽고(`{op: "load"}` → 파일 내용 또는 null) 쓴다(`{op: "save", data}`).
/// 앱은 JSON인지만 보고 그대로 파일에 옮겨 적는다(내용은 페이지가 정한다).
final class PracticePage: NSObject, WKScriptMessageHandlerWithReply, WKNavigationDelegate {
    /// 시험은 처음 쓰기 전에 다른 페이지·기록 파일로 바꿔 끼운다.
    static var shared = PracticePage(
        pageURL: Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "practice"),
        recordURL: Cssgsg.practiceRecordURL)

    let pageURL: URL?
    let recordURL: URL
    let webView: WKWebView
    private let writer = DispatchQueue(label: "com.cssgsg.settings.practice-record")

    init(pageURL: URL?, recordURL: URL) {
        self.pageURL = pageURL
        self.recordURL = recordURL
        let configuration = WKWebViewConfiguration()
        webView = KeyOnlyWebView(frame: .zero, configuration: configuration)
        super.init()
        // 컨트롤러가 처리기를 붙잡으므로 약한 고리로 넘긴다.
        configuration.userContentController.addScriptMessageHandler(
            WeakPracticeHandler(self), contentWorld: .page, name: "cssgsgPractice")
        webView.navigationDelegate = self
        if let pageURL {
            webView.loadFileURL(pageURL, allowingReadAccessTo: pageURL.deletingLastPathComponent())
        }
    }

    func userContentController(
        _ controller: WKUserContentController, didReceive message: WKScriptMessage,
        replyHandler: @escaping @MainActor @Sendable (Any?, String?) -> Void
    ) {
        guard let body = message.body as? [String: Any], let op = body["op"] as? String else {
            replyHandler(nil, "message")
            return
        }
        switch op {
        case "load":
            replyHandler(try? String(contentsOf: recordURL, encoding: .utf8), nil)
        case "save":
            guard let text = body["data"] as? String,
                  (try? JSONSerialization.jsonObject(with: Data(text.utf8))) is [String: Any]
            else {
                replyHandler(nil, "not a JSON object")
                return
            }
            let url = recordURL
            writer.async {
                do {
                    try FileManager.default.createDirectory(
                        at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
                    try Data(text.utf8).write(to: url, options: .atomic)
                } catch {
                    NSLog("cssgsg: 타자 연습 기록을 쓰지 못했다: \(error)")
                }
            }
            replyHandler(true, nil)
        default:
            replyHandler(nil, "op")
        }
    }

    /// WebKit 프로세스가 죽으면 다시 읽는다(페이지가 기록 파일에서 이어 간다).
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        if let pageURL {
            webView.loadFileURL(pageURL, allowingReadAccessTo: pageURL.deletingLastPathComponent())
        }
    }

    /// 쓰기 대기열이 비기를 기다린다(시험용).
    func flush() { writer.sync {} }
}

private final class WeakPracticeHandler: NSObject, WKScriptMessageHandlerWithReply {
    weak var target: PracticePage?

    init(_ target: PracticePage) { self.target = target }

    func userContentController(
        _ controller: WKUserContentController, didReceive message: WKScriptMessage,
        replyHandler: @escaping @MainActor @Sendable (Any?, String?) -> Void
    ) {
        guard let target else {
            replyHandler(nil, "gone")
            return
        }
        target.userContentController(controller, didReceive: message, replyHandler: replyHandler)
    }
}

/// 타자 연습 웹 뷰. 키는 받되 입력기는 부르지 않는다(페이지가 키 자리로 판정하므로 입력기가 할 일이 없다).
/// 설정 창의 다른 뷰(SwiftUI)가 키를 받고 있으면 그 뷰의 입력 컨텍스트로 입력기가 켜져서 Shift 톡에 모드가 바뀐다
/// (0.7.0, 사용자가 본 것: 개발자 기록에서 그동안 입력기가 켜져 있었다). 그래서 탭이 보이는 동안 웹 뷰가 키를 붙잡는다.
final class KeyOnlyWebView: WKWebView {
    /// 입력 컨텍스트가 없으면 앱이 입력기를 켜지 않는다(Shift 톡·글자 모두 페이지로만 간다).
    /// 글을 고칠 수 없는 페이지면 WebKit도 nil을 주지만 기대지 않고 못 박아 둔다.
    override var inputContext: NSTextInputContext? { nil }

    private var keyWindowObserver: NSObjectProtocol?

    /// 탭을 고르면(창에 붙으면) 클릭 없이 바로 칠 수 있게 키 입력을 받는다. 창이 다시 앞으로 올 때도.
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if let keyWindowObserver { NotificationCenter.default.removeObserver(keyWindowObserver) }
        keyWindowObserver = nil
        guard let window else { return }
        takeKeys()
        // SwiftUI·탭 보기가 붙이는 동안 첫 응답자를 다시 정할 수 있어서 조금 뒤에 한 번 더
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in self?.takeKeys() }
        keyWindowObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didBecomeKeyNotification, object: window, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.takeKeys() }
        }
    }

    private func takeKeys() {
        DispatchQueue.main.async { [weak self] in
            guard let self, let window = self.window, window.firstResponder !== self else { return }
            window.makeFirstResponder(self)
        }
    }
}

struct PracticeWebViewHost: NSViewRepresentable {
    func makeNSView(context: Context) -> WKWebView { PracticePage.shared.webView }

    func updateNSView(_ view: WKWebView, context: Context) {}
}
