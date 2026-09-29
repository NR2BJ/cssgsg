import Cocoa

/// 앱 글자에 하는 일(IMKTextInput에서 쓰는 것만). 입력기는 IMK 클라이언트를 감싸 쓰고,
/// 셸 스모크 테스트는 가짜 문서로 같은 코드(TextApplier)를 돌린다.
protocol TextClient {
    func insertText(_ text: String, replacementRange: NSRange)
    /// text는 NSAttributedString(밑줄) 또는 NSString("" 지우기).
    func setMarkedText(_ text: Any, selectionRange: NSRange, replacementRange: NSRange)
    func markedRange() -> NSRange
    func selectedRange() -> NSRange
    /// 문서의 그 구간 글자. 앱이 주지 못하면 nil.
    func substring(_ range: NSRange) -> String?
}

/// 엔진 결과의 글자 부분(확정, 조합)을 앱에 넣는다. 규칙은 core/src/engine.rs 맨 위 설명과 같다.
/// - 확정은 insertText만. 그 앞뒤에 setMarkedText("")를 부르지 않는다(Chromium·JS 에디터에서 글자가 사라진다).
/// - 조합이 앞 글자를 덮어야 하면(한자 변환) setMarkedText의 replacementRange로 끌어온다.
///   앱이 따랐는지 뒤에 markedRange로 확인하지 않는다. Chromium·WebKit은 조합을 늦게 반영해 그 값이 옛것이라
///   확인이 늘 틀리고, 그걸 믿고 다시 시작하면 WebKit에서는 앞 글자가 지워진다. 무시하는 앱은 글자가 겹칠 뿐 지워지지는 않는다.
enum TextApplier {
    /// "바꿀 구간 없음"(조합 글자나 선택을 바꾼다).
    static let noReplacement = NSRange(location: NSNotFound, length: NSNotFound)
    /// 한자 변환에 넘기는 앞 글자 길이(UTF-16). 러스트 시뮬레이터의 CONTEXT_UNITS와 같다.
    static let contextUnits = 20
    /// 선택은 이만큼만 읽는다. 엔진은 64보다 길면 바꾸지 않으므로 하나 더 읽어 "길다"를 알린다.
    static let selectionUnits = 65

    /// 확정·조합 글자를 넣는다. 조합이 앞 글자를 덮어야 하면(`replaceBefore` > 0) `anchor`(기준 자리) 앞
    /// 그만큼과 기준 자리를 replacementRange로 준다. 기준 자리를 모르거나 앞 글자가 모자라 끌어올 수 없으면
    /// 앱에 아무것도 하지 않고 false: 부른 쪽이 앞 글자 없이 다시 시작한다.
    @discardableResult
    static func apply(_ out: EngineOutput, to doc: TextClient, allowCommit: Bool = true, anchor: NSRange? = nil) -> Bool {
        if let preedit = out.preedit, preedit.replaceBefore > 0,
           anchor.map({ $0.location < preedit.replaceBefore }) ?? true {
            return false
        }
        let committed = !out.commit.isEmpty && allowCommit
        if committed {
            doc.insertText(out.commit, replacementRange: noReplacement)
        }
        guard let preedit = out.preedit else { return true }
        if preedit.text.isEmpty {
            // 확정 없이 조합이 사라졌을 때만 지운다. insertText가 이미 조합 글자를 대신했으면 부르지 않는다.
            if !committed {
                doc.setMarkedText("" as NSString, selectionRange: NSRange(location: 0, length: 0), replacementRange: noReplacement)
            }
            return true
        }
        var replacement = noReplacement
        if preedit.replaceBefore > 0, let anchor {
            replacement = NSRange(location: anchor.location - preedit.replaceBefore, length: preedit.replaceBefore + anchor.length)
        }
        doc.setMarkedText(marked(preedit), selectionRange: NSRange(location: preedit.caret, length: 0), replacementRange: replacement)
        return true
    }

    /// 한자 변환의 기준 자리: 조합 중이면 조합 글자(markedRange), 아니면 커서·선택(selectedRange). 모르면 nil.
    /// 조합 중인데 markedRange를 모르면 selectedRange로 대신하지 않는다(조합 글자 안이나 뒤를 가리켜 엉뚱한 글자를 덮는다).
    static func hanjaAnchor(_ doc: TextClient, composing: Bool) -> NSRange? {
        let r = composing ? doc.markedRange() : doc.selectedRange()
        guard r.location != NSNotFound, r.length != NSNotFound, !composing || r.length > 0 else { return nil }
        return r
    }

    /// 한자 변환에 줄 앱 글자.
    /// - before: 기준 자리 앞 contextUnits만큼. 앱이 읽어 주지 않으면 nil(엔진이 방금 친 한글을 쓴다).
    ///   기준 자리를 모르거나 문서 맨 앞이면 ""(끌어올 앞 글자 없음).
    /// - selected: 선택한 글(조합 중이 아닐 때). 못 읽으면 "".
    static func hanjaContext(_ doc: TextClient, anchor: NSRange?, composing: Bool) -> (before: String?, selected: String) {
        guard let anchor else { return ("", "") }
        var selected = ""
        if !composing, anchor.length > 0 {
            let range = NSRange(location: anchor.location, length: min(anchor.length, selectionUnits))
            if let s = doc.substring(range), s.utf16.count == range.length { selected = s }
        }
        let n = min(anchor.location, contextUnits)
        guard n > 0 else { return ("", selected) }
        guard let s = doc.substring(NSRange(location: anchor.location - n, length: n)), s.utf16.count == n else {
            return (nil, selected)
        }
        return (s, selected)
    }

    /// 한자 변환을 시작한 과정(개발자 기록용, 글자 내용 없이 길이만).
    struct HanjaTrace {
        var anchorKnown = false
        /// 앱이 준 앞 글자 길이. nil이면 앱이 읽어 주지 않아 엔진이 방금 친 한글을 썼다.
        var beforeLength: Int?
        var selectedLength = 0
        var replaceBefore = 0
        /// 선택이 있는데 읽지 못해 바꾸지 않았다(앞 글자만 바꾸면 선택이 사라진다).
        var selectionUnreadable = false
        /// 끌어올 수 없어(기준 자리보다 앞 글자가 모자람) 조합 음절만으로 다시 시작했다.
        var restarted = false
    }

    /// 한자 키: 앱 글자를 읽어 변환을 시작하고 글자를 넣는다. 돌려준 결과의 글자는 이미 넣었다.
    /// 후보창·모드 표시는 부른 쪽이 한다.
    static func beginHanja(_ context: HanjaContext, engine: CoreEngine, doc: TextClient) -> (EngineOutput, HanjaTrace) {
        let composing = context == .composing
        let anchor = hanjaAnchor(doc, composing: composing)
        let (before, selected) = hanjaContext(doc, anchor: anchor, composing: composing)
        var trace = HanjaTrace(anchorKnown: anchor != nil, beforeLength: before?.utf16.count, selectedLength: selected.utf16.count)
        if !composing, let anchor, anchor.length > 0, selected.isEmpty {
            // 키는 먹는다: 앱에 넘기면 선택이 줄바꿈으로 바뀐다.
            trace.selectionUnreadable = true
            var out = EngineOutput()
            out.consumed = true
            return (out, trace)
        }
        let first = engine.hanjaBegin(before: before, selected: selected)
        trace.replaceBefore = first.preedit?.replaceBefore ?? 0
        if apply(first, to: doc, anchor: anchor) {
            return (first, trace)
        }
        // 아직 앱에 아무것도 넣지 않았다. 앞 글자 없이(조합 음절만) 다시 시작한다.
        trace.restarted = true
        var retry = engine.hanjaBegin(before: "", selected: "")
        apply(retry, to: doc)
        retry.consumed = true
        if case .unchanged = retry.candidates {
            retry.candidates = first.candidates
        }
        return (retry, trace)
    }

    /// 조합 중 글자에 밑줄. 변환 중이면 문절마다 나누고 포커스된 문절은 굵게.
    static func marked(_ p: PreeditUpdate) -> NSAttributedString {
        let s = NSMutableAttributedString(string: p.text)
        let full = NSRange(location: 0, length: s.length)
        if p.segments.isEmpty {
            s.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: full)
        }
        for (i, seg) in p.segments.enumerated() where NSMaxRange(seg.range) <= s.length {
            let style: NSUnderlineStyle = seg.focused ? .thick : .single
            s.addAttributes([.underlineStyle: style.rawValue, .markedClauseSegment: i], range: seg.range)
        }
        return s
    }
}
