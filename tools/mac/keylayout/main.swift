// Graphite 자판(.keylayout)을 만든다. 인증 창(관리자 암호, 시스템 암호 시트)은 입력기가 넣은 글자를 버려서
// 입력기가 키를 넘기는데, 그때 macOS가 키를 글자로 바꾸는 자판을 이것으로 끼운다(입력기의 PasswordLayout, CONCEPT §13).
//
// - 글자 층(수식키 없음, Shift, Caps Lock, Shift+Caps Lock): 엔진 영어 모드가 내는 글자 그대로. 엔진이 넘기는 키(쿼티와 같은
//   글자, Return·화살표 같은 키)는 ABC가 내는 것.
// - ⌘·Control·Option이 낀 층: ABC 그대로. 단축키와 Option 특수 문자가 지금과 같다. 데드 키(Option+e 등)는 기다리지 않고
//   그 기호를 바로 낸다.
// - 독립 대조: layouts/en/official/Graphite.keylayout(Graphite 원작자 배포본)의 글자 층과 인쇄 가능한 글자를 비교한다.
//
// 쓰기: bash tools/mac/keylayout/run.sh [--check]
//   출력: mac/KeyboardLayout/cssgsg-Graphite.bundle (pkg가 /Library/Keyboard Layouts에 설치한다)
import Carbon
import Foundation

let bundleName = "cssgsg-Graphite.bundle"
let layoutName = "Graphite (cssgsg)"
let sourceID = "com.cssgsg.keylayout.graphite"
let bundleID = "com.cssgsg.keyboardlayout.graphite"
let layoutID = -27315

// MARK: - ABC

let abcData: Data = {
    let cond = [kTISPropertyInputSourceID as String: "com.apple.keylayout.ABC"] as CFDictionary
    guard let list = TISCreateInputSourceList(cond, true)?.takeRetainedValue() as? [TISInputSource],
          let source = list.first,
          let raw = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData)
    else { fatalError("ABC 배열(com.apple.keylayout.ABC)을 찾을 수 없다") }
    return Unmanaged<CFData>.fromOpaque(raw).takeUnretainedValue() as Data
}()
// ANSI 키보드로 만든다(사용자의 키보드는 모두 US 배열).
let keyboardType = UInt32(LMGetKbdType())

/// 수식키 조합(UCKeyTranslate의 modifierKeyState 비트와 같은 순서로 쓴다).
struct Combo: Hashable {
    var shift = false, caps = false, option = false, command = false, control = false

    static let all: [Combo] = (0..<32).map {
        Combo(shift: $0 & 1 != 0, caps: $0 & 2 != 0, option: $0 & 4 != 0, command: $0 & 8 != 0, control: $0 & 16 != 0)
    }

    var isLetterLayer: Bool { !option && !command && !control }

    var carbonState: UInt32 {
        var state: UInt32 = 0
        if command { state |= UInt32(cmdKey >> 8) }
        if shift { state |= UInt32(shiftKey >> 8) }
        if caps { state |= UInt32(alphaLock >> 8) }
        if option { state |= UInt32(optionKey >> 8) }
        if control { state |= UInt32(controlKey >> 8) }
        return state & 0xFF
    }

    /// keylayout의 modifier keys: 적은 것은 눌려 있어야 하고 적지 않은 것은 떼어져 있어야 한다.
    var keylayoutKeys: String {
        var names: [String] = []
        if shift { names.append("anyShift") }
        if caps { names.append("caps") }
        if option { names.append("anyOption") }
        if command { names.append("command") }
        if control { names.append("anyControl") }
        return names.joined(separator: " ")
    }
}

func abc(_ code: UInt16, _ combo: Combo) -> String {
    var dead: UInt32 = 0
    var length = 0
    var buffer = [UniChar](repeating: 0, count: 8)
    let status = abcData.withUnsafeBytes { bytes -> OSStatus in
        UCKeyTranslate(
            bytes.bindMemory(to: UCKeyboardLayout.self).baseAddress!, code, UInt16(kUCKeyActionDown), combo.carbonState,
            keyboardType, OptionBits(kUCKeyTranslateNoDeadKeysMask), &dead, buffer.count, &length, &buffer)
    }
    guard status == noErr else { return "" }
    return String(utf16CodeUnits: buffer, count: length)
}

// MARK: - 엔진(영어 모드)

/// 글자 층: 엔진이 확정하면 그 글자, 넘기면 ABC 글자. 먹고 아무것도 내지 않는 키는 빈 글자(그 키는 아무것도 치지 않는다).
func engineLayer(_ combo: Combo) -> (outputs: [String], eaten: [UInt16]) {
    guard let engine = cssgsg_engine_new(nil) else { fatalError("엔진을 만들지 못했다") }
    defer { cssgsg_engine_free(engine) }
    _ = cssgsg_engine_set_mode(engine, Int32(CSSGSG_MODE_EN))
    var outputs: [String] = []
    var eaten: [UInt16] = []
    var time = 1.0
    for code in UInt16(0)..<128 {
        var mods: UInt32 = 0
        if combo.shift { mods |= UInt32(CSSGSG_MOD_SHIFT_L) }
        if combo.caps { mods |= UInt32(CSSGSG_MOD_CAPS) }
        var event = CssgsgKeyEvent(key: cssgsg_key_from_mac_keycode(code), down: 1, is_repeat: 0, mods: mods, time: time)
        guard let out = cssgsg_engine_handle_key(engine, &event, nil)?.pointee else { fatalError("엔진 오류") }
        let commit = String(cString: out.commit)
        if out.consumed != 0 {
            if commit.isEmpty { eaten.append(code) }
            outputs.append(commit)
        } else {
            outputs.append(abc(code, combo))
        }
        event.down = 0
        event.time = time + 0.05
        _ = cssgsg_engine_handle_key(engine, &event, nil)
        time += 1
    }
    return (outputs, eaten)
}

// MARK: - 만들기

var maps: [[String]] = []
var mapIndexFor: [Combo: Int] = [:]
var eatenKeys: Set<UInt16> = []
for combo in Combo.all {
    let outputs: [String]
    if combo.isLetterLayer {
        let layer = engineLayer(combo)
        outputs = layer.outputs
        eatenKeys.formUnion(layer.eaten)
    } else {
        outputs = (UInt16(0)..<128).map { abc($0, combo) }
    }
    if let i = maps.firstIndex(of: outputs) {
        mapIndexFor[combo] = i
    } else {
        mapIndexFor[combo] = maps.count
        maps.append(outputs)
    }
}
precondition(mapIndexFor[Combo()] == 0, "수식키 없는 층이 0번이어야 한다")
let maxout = maps.flatMap { $0 }.map { $0.utf16.count }.max() ?? 1

func escape(_ text: String) -> String {
    var out = ""
    for scalar in text.unicodeScalars {
        switch scalar {
        case "&": out += "&amp;"
        case "<": out += "&lt;"
        case ">": out += "&gt;"
        case "\"": out += "&quot;"
        case "'": out += "&apos;"
        default:
            if scalar.value < 0x20 || scalar.value == 0x7F {
                out += String(format: "&#x%04X;", scalar.value)
            } else {
                out.unicodeScalars.append(scalar)
            }
        }
    }
    return out
}

var xml = """
<?xml version="1.1" encoding="UTF-8"?>
<!DOCTYPE keyboard SYSTEM "file://localhost/System/Library/DTDs/KeyboardLayout.dtd">
<!-- cssgsg가 엔진 데이터로 만든 Graphite 자판(tools/mac/keylayout). 고치지 말고 다시 만든다.
     글자 층은 cssgsg 영어 모드와 같고, ⌘·Control·Option 층은 ABC와 같다. -->
<keyboard group="126" id="\(layoutID)" name="\(layoutName)" maxout="\(maxout)">
    <layouts>
        <layout first="0" last="255" mapSet="ANSI" modifiers="Modifiers"/>
    </layouts>
    <modifierMap id="Modifiers" defaultIndex="0">

"""
for index in maps.indices {
    xml += "        <keyMapSelect mapIndex=\"\(index)\">\n"
    for combo in Combo.all where mapIndexFor[combo] == index {
        xml += "            <modifier keys=\"\(combo.keylayoutKeys)\"/>\n"
    }
    xml += "        </keyMapSelect>\n"
}
xml += "    </modifierMap>\n    <keyMapSet id=\"ANSI\">\n"
for (index, outputs) in maps.enumerated() {
    xml += "        <keyMap index=\"\(index)\">\n"
    for (code, output) in outputs.enumerated() where !output.isEmpty {
        xml += "            <key code=\"\(code)\" output=\"\(escape(output))\"/>\n"
    }
    xml += "        </keyMap>\n"
}
xml += "    </keyMapSet>\n</keyboard>\n"

let infoPlist = """
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>\(bundleID)</string>
    <key>CFBundleName</key>
    <string>\(layoutName)</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>KLInfo_\(layoutName)</key>
    <dict>
        <key>TISInputSourceID</key>
        <string>\(sourceID)</string>
        <key>TISIntendedLanguage</key>
        <string>en</string>
    </dict>
</dict>
</plist>

"""

// MARK: - 독립 대조(원작자 배포본)

/// 원작자 keylayout의 ANSI 글자 층(첫 layout의 mapSet, keyMap 0·1)에서 인쇄 가능한 글자.
func officialLayer(_ path: String, index: Int) -> [Int: String] {
    guard let text = try? String(contentsOfFile: path, encoding: .utf8) else { fatalError("\(path)를 읽지 못했다") }
    func firstMatch(_ pattern: String, in s: String) -> [String]? {
        let re = try! NSRegularExpression(pattern: pattern, options: [.dotMatchesLineSeparators])
        guard let m = re.firstMatch(in: s, range: NSRange(s.startIndex..., in: s)) else { return nil }
        return (0..<m.numberOfRanges).map { String(s[Range(m.range(at: $0), in: s)!]) }
    }
    guard let mapSet = firstMatch(#"<layout first="0" last="\d+" mapSet="([^"]+)""#, in: text)?[1],
          let block = firstMatch(#"<keyMapSet id="\#(mapSet)">(.*?)</keyMapSet>"#, in: text)?[1],
          let map = firstMatch(#"<keyMap index="\#(index)"[^>]*>(.*?)</keyMap>"#, in: block)?[1]
    else { fatalError("원작자 keylayout 구조를 읽지 못했다") }
    // 데드 키를 쓰는 배열이라 키 대부분이 action으로 적혀 있다: action의 기본 상태(none) 출력을 쓴다.
    var actions: [String: String] = [:]
    let actionRe = try! NSRegularExpression(
        pattern: #"<action id="([^"]+)">\s*<when state="none" output="([^"]*)""#, options: [.dotMatchesLineSeparators])
    for m in actionRe.matches(in: text, range: NSRange(text.startIndex..., in: text)) {
        actions[String(text[Range(m.range(at: 1), in: text)!])] = String(text[Range(m.range(at: 2), in: text)!])
    }
    let re = try! NSRegularExpression(pattern: #"<key code="(\d+)" (output|action)="([^"]*)"/>"#)
    var result: [Int: String] = [:]
    for m in re.matches(in: map, range: NSRange(map.startIndex..., in: map)) {
        let code = Int(map[Range(m.range(at: 1), in: map)!])!
        let raw = String(map[Range(m.range(at: 3), in: map)!])
        guard var value = map[Range(m.range(at: 2), in: map)!] == "output" ? raw : actions[raw] else { continue }
        for (entity, char) in [("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&apos;", "'"), ("&#x0027;", "'"),
                               ("&#x0022;", "\""), ("&#x003C;", "<"), ("&#x003E;", ">"), ("&#x0026;", "&"), ("&amp;", "&")] {
            value = value.replacingOccurrences(of: entity, with: char)
        }
        if value.unicodeScalars.count == 1, let s = value.unicodeScalars.first, (0x21...0x7E).contains(s.value) {
            result[code] = value
        }
    }
    return result
}

let root = URL(fileURLWithPath: CommandLine.arguments[1])
let check = CommandLine.arguments.contains("--check")
var differences: [String] = []
var compared = 0
for (index, combo) in [(0, Combo()), (1, Combo(shift: true))] {
    let official = officialLayer(root.appendingPathComponent("layouts/en/official/Graphite.keylayout").path, index: index)
    let ours = maps[mapIndexFor[combo]!]
    for (code, want) in official.sorted(by: { $0.key < $1.key }) where code < 0x33 {
        compared += 1
        if ours[code] != want {
            differences.append("키코드 \(code)\(combo.shift ? " Shift" : ""): 원작자 \(want), 우리 \(ours[code].isEmpty ? "(없음)" : ours[code])")
        }
    }
}

// MARK: - 쓰기

let bundle = root.appendingPathComponent("mac/KeyboardLayout/\(bundleName)")
let files: [(String, String)] = [
    ("Contents/Info.plist", infoPlist),
    ("Contents/Resources/\(layoutName).keylayout", xml),
]
var stale: [String] = []
for (relative, content) in files {
    let url = bundle.appendingPathComponent(relative)
    if check {
        if (try? String(contentsOf: url, encoding: .utf8)) != content { stale.append(relative) }
    } else {
        try! FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try! content.write(to: url, atomically: true, encoding: .utf8)
    }
}

print("층 \(maps.count)개(수식키 조합 32개), 최대 글자 수 \(maxout), 키보드 종류 \(keyboardType)")
if !eatenKeys.isEmpty { print("엔진이 먹고 아무것도 내지 않는 키: \(eatenKeys.sorted())") }
print("원작자 배포본과 글자 층 대조: \(compared)개 중 다름 \(differences.count)개")
differences.forEach { print("  \($0)") }
if check {
    if stale.isEmpty { print("\(bundleName) 최신") } else { print("\(bundleName)이 엔진과 다르다(다시 만든다): \(stale)"); exit(1) }
} else {
    print("→ \(bundle.path)")
}
exit(differences.isEmpty ? 0 : 1)
