import Carbon
import Foundation

/// 처음 실행 때 입력 소스를 등록하고 켠다.
///
/// pkg postinstall이 설치 직후 앱을 한 번 띄우므로, 첫 설치 뒤 입력 메뉴에 cssgsg가 바로 보인다
/// (시스템 설정에서 찾아 넣거나 다시 로그인하지 않아도 되게). 선택까지는 하지 않는다.
/// 한 번만 한다. 사용자가 나중에 입력 소스에서 뺐으면 업데이트해도 다시 켜지 않는다.
enum InputSourceSetup {
    private static let doneKey = "didEnableInputSource"

    static func enableOnce() {
        let defaults = UserDefaults.standard
        guard !defaults.bool(forKey: doneKey), let bundleID = Bundle.main.bundleIdentifier else { return }
        let registered = TISRegisterInputSource(Bundle.main.bundleURL as CFURL)
        let condition = [kTISPropertyInputSourceID as String: bundleID + ".en"] as CFDictionary
        guard let sources = TISCreateInputSourceList(condition, true)?.takeRetainedValue() as? [TISInputSource],
              let source = sources.first else {
            DeveloperLogger.shared.log("Setup", "input source not found", metadata: ["register": "\(registered)"])
            return
        }
        let enabled = TISEnableInputSource(source)
        if enabled == noErr {
            defaults.set(true, forKey: doneKey)
        }
        DeveloperLogger.shared.log("Setup", "enable input source", metadata: [
            "register": "\(registered)", "enable": "\(enabled)",
        ])
    }
}
