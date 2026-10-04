import AppKit

/// 비밀번호 칸에 입력기가 Graphite 글자를 직접 넣을지(사용자 결정 "비밀번호도 Graphite", CONCEPT §13).
///
/// 입력기가 넣은 글자를 버리는 칸이 있어서 아무 데나 넣지 않는다. 그런 칸에서 키를 먹으면 그 글자가 사라진다.
/// - 인증 창(SecurityAgent·loginwindow): NRIME에서 넣은 글자가 사라졌다. 키를 넘긴다(macOS가 ABC로 바꾼 쿼티).
/// - 브라우저·Electron 앱의 비밀번호 칸은 입력기를 그대로 쓴다: WebKit·Chromium은 로마자 입력 소스만, Firefox는
///   ASCII 입력 소스만 허용하는데 cssgsg는 영어 입력 소스라 남아서 키를 받는다(개발자 기록: Firefox 비밀번호 칸의 키).
///   여기서만 넣는다. 다른 앱의 칸은 확인하기 전까지 지금처럼 넘긴다.
/// - 그 앱의 칸인지는 클라이언트와 맨 앞 앱이 같은지로 본다. 브라우저가 띄운 시스템 암호 시트
///   (LocalAuthenticationRemoteService)는 다른 프로세스라 넣지 않는다.
///
/// OS 배열을 Graphite로 바꾸는 TISSetInputMethodKeyboardLayoutOverride는 macOS 26부터 noErr만 돌려주고 무시한다
/// (다른 입력기 두 곳이 입력기 안에서 읽어 보고 확인했다). 그래서 인증 창은 아직 쿼티다.
enum PasswordFields {
    /// Safari 계열(WebKit). 다른 WebKit 앱은 확인하기 전이라 넣지 않는다.
    static let webKitBrowsers: Set<String> = ["com.apple.Safari", "com.apple.SafariTechnologyPreview"]

    /// 판정. 비밀번호 칸(secure)일 때만 부른다.
    static func insertsGraphite(client: String?, frontmost: String?, authentication: Bool,
                                frontmostIsWebEngine: () -> Bool) -> Bool {
        guard let client, client == frontmost, !authentication else { return false }
        return frontmostIsWebEngine()
    }

    /// 지금 맨 앞 앱으로 판정한다.
    static func insertsGraphite(client: String?, authentication: Bool) -> Bool {
        let app = NSWorkspace.shared.frontmostApplication
        return insertsGraphite(client: client, frontmost: app?.bundleIdentifier, authentication: authentication) {
            isWebEngine(app)
        }
    }

    /// 번들 경로별 판정(키마다 파일을 뒤지지 않게). 메인 스레드에서만 쓴다.
    private static var cache: [String: Bool] = [:]

    /// WebKit(Safari), Chromium(Chrome·Edge·Electron), Gecko(Firefox 계열).
    static func isWebEngine(_ app: NSRunningApplication?) -> Bool {
        guard let app, let id = app.bundleIdentifier, let path = app.bundleURL?.path else { return false }
        if let cached = cache[path] { return cached }
        let found = webKitBrowsers.contains(id) || ChromiumDetector.isChromiumBundle(atPath: path)
            || isGeckoBundle(atPath: path)
        cache[path] = found
        return found
    }

    /// Firefox 계열: 번들에 XUL(libxul)이 있다. org.mozilla.* 말고도 Zen·LibreWolf 같은 파생 브라우저가 있다.
    static func isGeckoBundle(atPath path: String) -> Bool {
        FileManager.default.fileExists(atPath: path + "/Contents/MacOS/XUL")
    }
}
