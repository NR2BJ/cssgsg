import Carbon
import Foundation

/// 입력 소스를 켠다(처음 실행) · 반쪽 상태를 되살린다(그 뒤).
///
/// 모드가 있는 입력기는 본체(TISTypeKeyboardInputMethodModeEnabled)와 모드(TISTypeKeyboardInputMode)가 따로 켜진다.
/// 모드만 켜면 입력 메뉴에도, 시스템 설정의 추가(+) 목록에도 안 보인다. 0.1.0이 이렇게 만들었다.
/// 그래서 NRIME(Tools/pkg/enable_input_source.swift)처럼 같은 번들 ID의 입력 소스를 모두 켠다.
///
/// - 처음 실행: 본체와 모드를 모두 켠다. pkg postinstall이 설치 직후 앱을 한 번 띄우므로 입력 메뉴에 바로 보인다.
/// - 그 뒤: 모드는 켜졌는데 본체가 꺼진 반쪽 상태만 고친다. 사용자가 입력 소스에서 뺐으면(둘 다 꺼짐) 건드리지 않는다.
/// - 선택은 하지 않는다.
enum InputSourceSetup {
    struct Source: Equatable {
        let id: String
        /// 입력기 본체인지(아니면 모드).
        let isMethod: Bool
        let enabled: Bool
        let enableCapable: Bool
    }

    private static let doneKey = "didEnableInputSource"

    /// 켤 입력 소스 ID. 본체를 먼저 켠다. 시스템을 건드리지 않는 순수 함수(tools/mac/update-smoke가 확인한다).
    static func plan(_ sources: [Source], firstRun: Bool) -> [String] {
        let off = sources.filter { !$0.enabled && $0.enableCapable }
        let methods = off.filter(\.isMethod).map(\.id)
        if firstRun {
            return methods + off.filter { !$0.isMethod }.map(\.id)
        }
        let modeOn = sources.contains { !$0.isMethod && $0.enabled }
        return modeOn ? methods : []
    }

    static func run() {
        guard let bundleID = Bundle.main.bundleIdentifier else { return }
        var found = installed(bundleID)
        var registered = "-"
        if found.isEmpty {
            registered = "\(TISRegisterInputSource(Bundle.main.bundleURL as CFURL))"
            found = installed(bundleID)
        }
        let defaults = UserDefaults.standard
        let firstRun = !defaults.bool(forKey: doneKey)
        var results: [String: String] = ["register": registered]
        for id in plan(found.map(\.info), firstRun: firstRun) {
            guard let source = found.first(where: { $0.info.id == id })?.source else { continue }
            results[id] = "\(TISEnableInputSource(source))"
        }
        if firstRun && !found.isEmpty {
            defaults.set(true, forKey: doneKey)
        }
        if results.count > 1 || registered != "-" {
            DeveloperLogger.shared.log("Setup", firstRun ? "first run" : "repair", metadata: results)
        }
    }

    /// 같은 번들 ID의 입력 소스(켜지 않은 것까지 모두).
    static func installed(_ bundleID: String) -> [(source: TISInputSource, info: Source)] {
        let condition = [kTISPropertyBundleID as String: bundleID] as CFDictionary
        let list = (TISCreateInputSourceList(condition, true)?.takeRetainedValue() as? [TISInputSource]) ?? []
        let methodTypes = [kTISTypeKeyboardInputMethodModeEnabled as String, kTISTypeKeyboardInputMethodWithoutModes as String]
        return list.compactMap { source in
            guard let id = string(source, kTISPropertyInputSourceID) else { return nil }
            let info = Source(
                id: id,
                isMethod: methodTypes.contains(string(source, kTISPropertyInputSourceType) ?? ""),
                enabled: bool(source, kTISPropertyInputSourceIsEnabled),
                enableCapable: bool(source, kTISPropertyInputSourceIsEnableCapable)
            )
            return (source, info)
        }
    }

    private static func string(_ source: TISInputSource, _ key: CFString) -> String? {
        guard let pointer = TISGetInputSourceProperty(source, key) else { return nil }
        return Unmanaged<CFString>.fromOpaque(pointer).takeUnretainedValue() as String
    }

    private static func bool(_ source: TISInputSource, _ key: CFString) -> Bool {
        guard let pointer = TISGetInputSourceProperty(source, key) else { return false }
        return CFBooleanGetValue(Unmanaged<CFBoolean>.fromOpaque(pointer).takeUnretainedValue())
    }
}
