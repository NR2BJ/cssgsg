import SwiftUI
import WebKit

/// 배열 학습 페이지(learn/index.html, 엔진 데이터로 만든 것). 바깥 링크는 기본 브라우저로 연다.
struct LearnTab: View {
    var body: some View {
        if let url = Bundle.main.url(forResource: "index", withExtension: "html") {
            LearnWebView(url: url)
        } else {
            Text("학습 페이지를 찾을 수 없다").foregroundStyle(.secondary)
        }
    }
}

struct LearnWebView: NSViewRepresentable {
    let url: URL

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> WKWebView {
        let view = WKWebView()
        view.navigationDelegate = context.coordinator
        view.uiDelegate = context.coordinator
        view.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        return view
    }

    func updateNSView(_ view: WKWebView, context: Context) {}

    final class Coordinator: NSObject, WKNavigationDelegate, WKUIDelegate {
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
}
