import SwiftUI
import WebKit

/// 배열 학습 페이지(learn/index.html, 엔진 데이터로 만든 것). 바깥 링크는 기본 브라우저로 연다.
struct LearnTab: View {
    var body: some View {
        if LearnPage.shared.url != nil {
            LearnWebView()
        } else {
            Text(tr("학습 페이지를 찾을 수 없다", "The learning page is missing", "学習ページが見つかりません"))
                .foregroundStyle(.secondary)
        }
    }
}

/// 웹 뷰 하나를 앱이 뜰 때 만들어 미리 읽어 두고 계속 쓴다. 탭을 누를 때마다 새로 만들면 WebKit 프로세스를 띄우고
/// 페이지를 다시 읽느라 잠깐 비어 있었다(0.5.0). 고른 배열 탭과 스크롤도 그대로 남는다.
final class LearnPage: NSObject, WKNavigationDelegate, WKUIDelegate {
    static let shared = LearnPage()

    let url = Bundle.main.url(forResource: "index", withExtension: "html")
    let webView = WKWebView()

    private override init() {
        super.init()
        webView.navigationDelegate = self
        webView.uiDelegate = self
        if let url {
            webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        }
    }

    func webView(
        _ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
        decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
    ) {
        if let url = action.request.url, action.navigationType == .linkActivated, url.scheme?.hasPrefix("http") == true {
            NSWorkspace.shared.open(url)
            decisionHandler(.cancel)
        } else {
            decisionHandler(.allow)
        }
    }

    /// target="_blank" 링크
    func webView(
        _ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
        for action: WKNavigationAction, windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        if let url = action.request.url { NSWorkspace.shared.open(url) }
        return nil
    }
}

struct LearnWebView: NSViewRepresentable {
    func makeNSView(context: Context) -> WKWebView { LearnPage.shared.webView }

    func updateNSView(_ view: WKWebView, context: Context) {}
}
