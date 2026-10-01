import SwiftUI
import WebKit

/// 타자 연습 페이지(practice/index.html, tools/practice/build.mjs가 엔진 데이터로 만든 것). 창을 키우거나 전체 화면으로 쓴다.
/// 기록(끝낸 단계, 자주 틀린 키, 고른 단계)은 설정 파일 옆 practice.json에 남는다. 같은 파일을 브라우저로 열면
/// 그 브라우저의 localStorage에 남는다.
struct PracticeTab: View {
    var body: some View {
        if PracticePage.shared.pageURL != nil {
            PracticeWebView()
                // 탭을 고르면 바로 칠 수 있게 키 입력을 웹 뷰로 보낸다(페이지에는 입력칸이 없어 입력기가 끼지 않는다).
                .onAppear { PracticePage.shared.focus() }
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
    static let shared = PracticePage(
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
        webView = WKWebView(frame: .zero, configuration: configuration)
        super.init()
        // 컨트롤러가 처리기를 붙잡으므로 약한 고리로 넘긴다.
        configuration.userContentController.addScriptMessageHandler(
            WeakPracticeHandler(self), contentWorld: .page, name: "cssgsgPractice")
        webView.navigationDelegate = self
        if let pageURL {
            webView.loadFileURL(pageURL, allowingReadAccessTo: pageURL.deletingLastPathComponent())
        }
    }

    /// 키 입력을 웹 뷰로. 탭이 막 나타났을 때는 아직 창에 붙기 전이라 한 박자 늦춘다.
    func focus() {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [webView] in
            webView.window?.makeFirstResponder(webView)
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

struct PracticeWebView: NSViewRepresentable {
    func makeNSView(context: Context) -> WKWebView { PracticePage.shared.webView }

    func updateNSView(_ view: WKWebView, context: Context) {}
}
