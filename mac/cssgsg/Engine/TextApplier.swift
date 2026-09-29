import Cocoa

/// 앱 글자에 하는 일(IMKTextInput에서 쓰는 것만). 입력기는 IMK 클라이언트를 감싸 쓰고,
/// 셸 스모크 테스트는 가짜 문서로 같은 코드(TextApplier)를 돌린다.
protocol TextClient {
    func insertText(_ text: String, replacementRange: NSRange)
    /// text는 NSAttributedString(밑줄) 또는 NSString("" 지우기).
    func setMarkedText(_ text: Any, selectionRange: NSRange, replacementRange: NSRange)
}

/// 엔진 결과의 글자 부분(확정, 조합)을 앱에 넣는다. 규칙은 core/src/engine.rs 맨 위 설명과 같다.
/// - 확정은 insertText만. 그 앞뒤에 setMarkedText("")를 부르지 않는다(Chromium·JS 에디터에서 글자가 사라진다).
/// - 바꾸는 것은 늘 조합 글자(없으면 선택)다. replacementRange로 앞 글자를 덮지 않는다:
///   0.3.0 한자 변환이 그렇게 했다가 Discord(Chromium)가 무시해서 글자가 겹쳤다.
enum TextApplier {
    /// "바꿀 구간 없음"(조합 글자나 선택을 바꾼다).
    static let noReplacement = NSRange(location: NSNotFound, length: NSNotFound)

    static func apply(_ out: EngineOutput, to doc: TextClient, allowCommit: Bool = true) {
        let committed = !out.commit.isEmpty && allowCommit
        if committed {
            doc.insertText(out.commit, replacementRange: noReplacement)
        }
        guard let preedit = out.preedit else { return }
        if !preedit.text.isEmpty {
            doc.setMarkedText(marked(preedit), selectionRange: NSRange(location: preedit.caret, length: 0), replacementRange: noReplacement)
        } else if !committed {
            // 확정 없이 조합이 사라졌을 때만 지운다. insertText가 이미 조합 글자를 대신했으면 부르지 않는다.
            doc.setMarkedText("" as NSString, selectionRange: NSRange(location: 0, length: 0), replacementRange: noReplacement)
        }
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
