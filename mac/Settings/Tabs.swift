import SwiftUI

// MARK: - 일반

struct GeneralTab: View {
    @ObservedObject var model: SettingsModel

    /// 탭 키 이름(설정 파일)과 화면 이름. 기본값에 쓰는 Shift 둘을 먼저 보이고 나머지는 접어 둔다.
    private static let mainKeys = [("shift_right", "오른쪽 Shift"), ("shift_left", "왼쪽 Shift")]
    private static let otherKeys = [
        ("control_left", "왼쪽 Control"), ("control_right", "오른쪽 Control"),
        ("alt_left", "왼쪽 Option"), ("alt_right", "오른쪽 Option"),
        ("meta_left", "왼쪽 Command"), ("meta_right", "오른쪽 Command"),
    ]

    var body: some View {
        Form {
            Section {
                ForEach(Self.mainKeys, id: \.0) { key, label in tapPicker(key, label) }
                DisclosureGroup("다른 수식키") {
                    ForEach(Self.otherKeys, id: \.0) { key, label in tapPicker(key, label) }
                }
                if !model.config.taps.values.contains("toggle_english") {
                    Label("영어로 오가는 키가 없다. 메뉴 막대로는 모드를 바꿀 수 없으니 하나는 정해 두자.", systemImage: "exclamationmark.triangle")
                        .foregroundStyle(.orange)
                }
                LabeledContent("탭 인식 시간") {
                    MillisecondSlider(value: model.config.tapThresholdMs, range: 100...400, step: 10) { ms in
                        model.update { $0.tapThresholdMs = ms }
                    }
                }
            } header: {
                Text("언어 전환: 수식키 탭")
            } footer: {
                Text("수식키를 혼자 짧게 눌렀다 떼면 모드를 바꾼다. 다른 키와 같이 누르거나 탭 인식 시간보다 오래 누르면 탭이 아니다.")
            }
            Section("모드 표시") {
                Toggle("모드를 바꿀 때 커서 근처에 G · ㅊ · 月을 잠깐 보인다", isOn: model.binding(\.mac.hud))
                Picker("자리", selection: model.binding(\.mac.hudPosition)) {
                    Text("커서 위 (커서 자리를 모르는 앱에서는 안 보임)").tag("caret")
                    Text("마우스 옆").tag("mouse")
                }
                .disabled(!model.config.mac.hud)
            }
            Section {
                LabeledContent("Shift+Enter 줄바꿈 대기") {
                    MillisecondSlider(value: model.config.mac.newlineReplayMs, range: 20...500, step: 10) { ms in
                        model.update { $0.mac.newlineReplayMs = ms }
                    }
                }
            } header: {
                Text("고급")
            } footer: {
                Text("Codex처럼 Enter를 전송으로 받는 앱에서 조합 중 Shift+Enter를 누르면, 확정한 뒤 이만큼 기다렸다 줄을 바꾼다. 너무 짧으면 줄바꿈이 먹힌다.")
            }
        }
        .formStyle(.grouped)
    }

    private func tapPicker(_ key: String, _ label: String) -> some View {
        Picker(label, selection: model.tapBinding(key)) {
            Text("없음").tag("none")
            Text("영어 ↔ 방금 쓰던 언어").tag("toggle_english")
            Text("한국어 ↔ 일본어").tag("toggle_non_english")
        }
    }
}

/// 밀리초 슬라이더. 끄는 동안은 숫자만 바꾸고, 놓을 때 한 번 쓴다(파일을 여러 번 쓰지 않게).
struct MillisecondSlider: View {
    let value: Int
    let range: ClosedRange<Double>
    let step: Double
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
            Text("\(Int((draft ?? Double(value)).rounded()))ms")
                .monospacedDigit()
                .frame(minWidth: 52, alignment: .trailing)
        }
    }
}

// MARK: - 한국어

struct KoreanTab: View {
    @ObservedObject var model: SettingsModel
    @State private var confirmClear = false

    var body: some View {
        Form {
            Section {
                Picker("배열", selection: model.binding(\.koLayout)) {
                    Text("참신세벌식 v18 (기본형)").tag("chamshin-v18")
                    Text("참신세벌식 D v19").tag("chamshin-d-v19")
                }
                .pickerStyle(.radioGroup)
            } header: {
                Text("배열")
            } footer: {
                Text("D는 숫자 줄에도 글자가 있다. 두 배열의 자리는 배열 학습 탭에 있다.")
            }
            Section {
                LabeledContent("고른 한자 기억") {
                    HStack {
                        Text("\(model.hanjaLearningCount)개").monospacedDigit()
                        Button("지우기…") { confirmClear = true }
                            .disabled(model.hanjaLearningCount == 0)
                    }
                }
            } header: {
                Text("한자")
            } footer: {
                Text("조합 중인 글자에서 ⌥Option+Return을 누르면 한자로 바꾼다(자음 하나면 기호). 고른 한자는 기억해서 다음에 먼저 보인다.")
            }
        }
        .formStyle(.grouped)
        .confirmationDialog("고른 한자 기억을 모두 지울까?", isPresented: $confirmClear) {
            Button("지우기", role: .destructive) { model.clearHanjaLearning() }
        }
    }
}

// MARK: - 일본어

struct JapaneseTab: View {
    @ObservedObject var model: SettingsModel
    @State private var confirmClear = false

    var body: some View {
        Form {
            Section("기호") {
                Picker("구두점", selection: model.binding(\.ja.punctuation)) {
                    Text("、。「」 일본식").tag("japanese")
                    Text("，．［］ 전각 서양식").tag("full_width_western")
                    Text(",.[] 반각").tag("half_width_western")
                }
                Toggle("/ 자리를 ・로", isOn: model.binding(\.ja.slashNakaguro))
                Toggle("읽기가 없을 때 Space를 전각 스페이스로", isOn: model.binding(\.ja.fullWidthSpace))
            }
            Section("가타카나 (Caps Lock)") {
                Toggle("치는 대로 바로 확정한다 (마지막 글자만 잠깐 밑줄)", isOn: model.binding(\.ja.katakanaDirect))
                Toggle("일본어 모드를 나가면 Caps Lock을 끈다", isOn: model.binding(\.ja.capsKatakanaAutoOff))
            }
            Section {
                LabeledContent("변환 학습 (Mozc)") {
                    Button("지우기…") { confirmClear = true }
                }
            } header: {
                Text("변환")
            } footer: {
                Text("Mozc가 기억한 변환(문절 나누기, 고른 후보)을 지운다. 지우면 입력기가 다시 시작된다(다음 키 입력 때 저절로 뜬다).")
            }
        }
        .formStyle(.grouped)
        .confirmationDialog("Mozc 변환 학습을 지우고 입력기를 다시 시작할까?", isPresented: $confirmClear) {
            Button("지우기", role: .destructive) { model.clearMozcLearning() }
        }
    }
}

// MARK: - 정보

struct AboutTab: View {
    @ObservedObject var model: SettingsModel
    @State private var showNotices = false

    private var appVersion: String { Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "?" }

    /// 옆에 설치된 입력기의 버전.
    private var imeVersion: String {
        let url = Bundle.main.bundleURL.deletingLastPathComponent().appendingPathComponent("cssgsg.app")
        return Bundle(url: url)?.infoDictionary?["CFBundleShortVersionString"] as? String ?? "찾을 수 없음"
    }

    var body: some View {
        Form {
            Section {
                LabeledContent("입력기", value: imeVersion)
                LabeledContent("설정 앱", value: appVersion)
                LabeledContent("입력 소스") {
                    if model.inputSourceAdded {
                        Label("추가됨", systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                    } else {
                        HStack {
                            Text("추가 안 됨").foregroundStyle(.orange)
                            Button("시스템 설정 열기") { Cssgsg.openKeyboardSettings() }
                        }
                    }
                }
                Link("GitHub: NR2BJ/cssgsg", destination: URL(string: "https://github.com/NR2BJ/cssgsg")!)
                Link("바뀐 점 (릴리스)", destination: URL(string: "https://github.com/NR2BJ/cssgsg/releases")!)
            } header: {
                Text("cssgsg")
            } footer: {
                Text("업데이트는 메뉴 막대의 cssgsg 메뉴에서 확인한다. 입력 소스는 시스템 설정 → 키보드 → 입력 소스 편집 → + → 영어 → cssgsg로 추가한다.")
            }
            Section {
                LabeledContent("설정 파일") {
                    Button("Finder에서 보기") { model.revealConfigFile() }
                }
                Toggle("개발자 기록", isOn: Binding(get: { model.developerMode }, set: { model.setDeveloperMode($0) }))
                LabeledContent("기록 파일") {
                    Button("Finder에서 보기") { NSWorkspace.shared.activateFileViewerSelecting([Cssgsg.developerLogURL]) }
                        .disabled(!FileManager.default.fileExists(atPath: Cssgsg.developerLogURL.path))
                }
            } header: {
                Text("파일")
            } footer: {
                Text("설정 파일(config.toml)을 직접 고쳐도 된다. 개발자 기록에는 키 코드·수식키·시각만 남고 글자 내용은 남지 않는다.")
            }
            Section("라이선스") {
                LabeledContent("cssgsg", value: "MIT")
                Button("다른 소프트웨어·데이터 고지문…") { showNotices = true }
            }
        }
        .formStyle(.grouped)
        .sheet(isPresented: $showNotices) { NoticesView() }
    }
}

struct NoticesView: View {
    @Environment(\.dismiss) private var dismiss

    private var text: String {
        guard let url = Bundle.main.url(forResource: "THIRD_PARTY_NOTICES", withExtension: "txt") else { return "고지문 파일이 없다" }
        return (try? String(contentsOf: url, encoding: .utf8)) ?? "고지문을 읽지 못했다"
    }

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                Text(text)
                    .font(.system(.caption, design: .monospaced))
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding()
            }
            Divider()
            HStack {
                Spacer()
                Button("닫기") { dismiss() }.keyboardShortcut(.defaultAction)
            }
            .padding(10)
        }
        .frame(width: 640, height: 520)
    }
}
