import SwiftUI

/// 창 위에 뜨는 경고 줄.
struct Banner: View {
    let text: String

    var body: some View {
        Label(text, systemImage: "exclamationmark.triangle.fill")
            .font(.callout)
            .foregroundStyle(.primary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(10)
            .background(Color.orange.opacity(0.18))
            .textSelection(.enabled)
    }
}

/// 수 슬라이더. 끄는 동안은 숫자만 바꾸고, 놓을 때 한 번 쓴다(파일을 여러 번 쓰지 않게).
struct ValueSlider: View {
    let value: Int
    let range: ClosedRange<Double>
    let step: Double
    var unit = "ms"
    /// 0을 가운데 둔 조정값이면 +/-를 붙인다.
    var signed = false
    let commit: (Int) -> Void
    @State private var draft: Double?

    var body: some View {
        HStack {
            Slider(value: Binding(get: { draft ?? Double(value) }, set: { draft = $0 }), in: range, step: step) { editing in
                if !editing, let draft {
                    commit(Int(draft.rounded()))
                    self.draft = nil
                }
            }
            .frame(minWidth: 160)
            Text(label(Int((draft ?? Double(value)).rounded())))
                .monospacedDigit()
                .frame(minWidth: 52, alignment: .trailing)
        }
    }

    private func label(_ v: Int) -> String {
        signed && v > 0 ? "+\(v)\(unit)" : "\(v)\(unit)"
    }
}

/// 키 모양 글자(설명서).
struct KeyCap: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(.callout, design: .rounded).weight(.medium))
            .padding(.horizontal, 6)
            .padding(.vertical, 1)
            .background(RoundedRectangle(cornerRadius: 4).fill(Color.secondary.opacity(0.15)))
            .overlay(RoundedRectangle(cornerRadius: 4).stroke(Color.secondary.opacity(0.35), lineWidth: 0.5))
            .fixedSize()
    }
}
