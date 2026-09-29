import AppKit
import SwiftUI

/// Mozc 사용자 사전(~/Library/Application Support/cssgsg/mozc/user_dictionary.db). 읽기·쓰기는 러스트 설정 라이브러리
/// (config-ffi/src/userdict.rs)가 한다. 첫 사전만 보이고 고친다(없으면 "User Dictionary"를 만든다, NRIME와 같다).
/// 저장하면 입력기에 알리고 입력기는 Mozc에 다시 읽으라고 한다(바로 적용).
struct UserDictStorage: Codable {
    var dictionaries: [UserDict]
}

struct UserDict: Codable {
    /// u64라서 문자열. 새 사전이면 ""(저장할 때 새로 매긴다).
    var id: String
    var name: String
    var entries: [UserDictEntry]
}

struct UserDictEntry: Codable, Identifiable, Equatable {
    /// 화면에서 고르기 위한 것. 파일에는 없다.
    var id = UUID()
    var key: String
    var value: String
    var comment: String
    var pos: Int
    var locale: String

    enum CodingKeys: String, CodingKey { case key, value, comment, pos, locale }

    init(key: String, value: String, comment: String, pos: Int, locale: String = "") {
        (self.key, self.value, self.comment, self.pos, self.locale) = (key, value, comment, pos, locale)
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        key = try c.decode(String.self, forKey: .key)
        value = try c.decode(String.self, forKey: .value)
        comment = try c.decodeIfPresent(String.self, forKey: .comment) ?? ""
        pos = try c.decodeIfPresent(Int.self, forKey: .pos) ?? 1
        locale = try c.decodeIfPresent(String.self, forKey: .locale) ?? ""
    }
}

/// Mozc 품사(user_dictionary_storage.proto PosType). 고르는 목록은 NRIME와 같은 18개, 이름은 파일에 있는 모든 품사.
enum MozcPOS {
    static let choices = [1, 4, 5, 6, 7, 8, 9, 2, 3, 14, 15, 16, 18, 19, 20, 21, 40, 44]

    private static let japanese: [Int: String] = [
        1: "名詞", 2: "短縮よみ", 3: "サジェストのみ", 4: "固有名詞", 5: "人名", 6: "姓", 7: "名", 8: "組織", 9: "地名",
        10: "名詞サ変", 11: "名詞形動", 12: "数", 13: "アルファベット", 14: "記号", 15: "顔文字", 16: "副詞", 17: "連体詞",
        18: "接続詞", 19: "感動詞", 20: "接頭語", 21: "助数詞", 22: "接尾一般", 23: "接尾人名", 24: "接尾地名",
        25: "動詞ワ行五段", 26: "動詞カ行五段", 27: "動詞サ行五段", 28: "動詞タ行五段", 29: "動詞ナ行五段", 30: "動詞マ行五段",
        31: "動詞ラ行五段", 32: "動詞ガ行五段", 33: "動詞バ行五段", 34: "動詞ハ行四段", 35: "動詞一段", 36: "動詞カ変",
        37: "動詞サ変", 38: "動詞ザ変", 39: "動詞ラ変", 40: "形容詞", 41: "終助詞", 42: "句読点", 43: "独立語", 44: "抑制単語",
    ]

    private static let korean: [Int: String] = [
        1: "명사", 2: "줄인 읽기", 3: "추천에만", 4: "고유명사", 5: "사람 이름", 6: "성", 7: "이름", 8: "조직", 9: "지명",
        14: "기호", 15: "이모티콘", 16: "부사", 18: "접속사", 19: "감탄사", 20: "접두어", 21: "조수사", 40: "형용사", 44: "억제 단어",
    ]

    private static let english: [Int: String] = [
        1: "Noun", 2: "Abbreviation", 3: "Suggestion only", 4: "Proper noun", 5: "Person name", 6: "Family name",
        7: "Given name", 8: "Organization", 9: "Place name", 14: "Symbol", 15: "Emoticon", 16: "Adverb", 18: "Conjunction",
        19: "Interjection", 20: "Prefix", 21: "Counter", 40: "Adjective", 44: "Suppressed word",
    ]

    static func name(_ pos: Int) -> String {
        let ja = japanese[pos] ?? "\(pos)"
        switch UILanguage.active {
        case .ja: return ja
        case .ko: return korean[pos].map { "\($0) \(ja)" } ?? ja
        case .en: return english[pos].map { "\($0) \(ja)" } ?? ja
        }
    }
}

@MainActor
final class UserDictionaryModel: ObservableObject {
    @Published private(set) var entries: [UserDictEntry] = []
    /// 마지막으로 읽거나 쓰지 못한 까닭.
    @Published private(set) var problem: String?

    private var storage = UserDictStorage(dictionaries: [])
    private var loadedDate: Date?

    static var url: URL { Cssgsg.mozcProfileURL.appendingPathComponent("user_dictionary.db") }

    init() {
        load()
    }

    private static var modificationDate: Date? {
        (try? FileManager.default.attributesOfItem(atPath: url.path))?[.modificationDate] as? Date
    }

    /// 파일이 바뀌었을 때만 다시 읽는다(창이 앞으로 올 때). 고르던 줄이 풀리지 않게.
    func reloadIfChanged() {
        if Self.modificationDate != loadedDate { load() }
    }

    func load() {
        loadedDate = Self.modificationDate
        guard let pointer = Self.url.path.withCString({ cssgsg_userdict_json($0) }) else {
            problem = ConfigProblem.lastError.message
            return
        }
        do {
            storage = try JSONDecoder().decode(UserDictStorage.self, from: Data(String(cString: pointer).utf8))
            entries = storage.dictionaries.first?.entries ?? []
            problem = nil
        } catch {
            problem = "\(error)"
        }
    }

    /// 읽기를 저장하는 모양으로(Mozc UserDictionaryUtil::NormalizeReading과 같은 쪽): 앞뒤 공백을 떼고,
    /// 반각 가타카나·전각 영숫자는 NFKC로 전각 가타카나·반각 영숫자로, 가타카나(ァ~ヶ)는 히라가나로 바꾼다.
    /// Foundation의 가타카나→히라가나 변환은 쓰지 않는다: 장음 부호를 모음으로 바꾼다(ヴぁー → ゔぁあ).
    static func normalizedReading(_ text: String) -> String {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines).precomposedStringWithCompatibilityMapping
        var out = String.UnicodeScalarView()
        for scalar in trimmed.unicodeScalars {
            if (0x30A1...0x30F6).contains(scalar.value), let hiragana = Unicode.Scalar(scalar.value - 0x60) {
                out.append(hiragana)
            } else {
                out.append(scalar)
            }
        }
        return String(out)
    }

    /// 넣거나 고칠 낱말이 쓸 만한지. 쓸 만하면 nil.
    func problem(with entry: UserDictEntry) -> String? {
        let fields = [entry.key, entry.value, entry.comment]
        if entry.key.isEmpty || entry.value.isEmpty {
            return tr("읽기와 단어를 쓴다.", "Enter a reading and a word.", "読みと単語を入力します。")
        }
        if fields.contains(where: { $0.count > 300 }) {
            return tr("300자까지 쓸 수 있다.", "Up to 300 characters.", "300 文字までです。")
        }
        if fields.contains(where: { $0.contains(where: { $0 == "\t" || $0.isNewline }) }) {
            return tr("탭이나 줄바꿈은 쓸 수 없다.", "Tabs and line breaks aren’t allowed.", "タブや改行は使えません。")
        }
        if entries.contains(where: { $0.id != entry.id && $0.key == entry.key && $0.value == entry.value && $0.pos == entry.pos }) {
            return tr("같은 낱말이 이미 있다.", "The same word is already there.", "同じ単語がすでにあります。")
        }
        return nil
    }

    /// 넣거나(같은 id가 없으면) 고친다. 저장하지 못하면 까닭.
    func upsert(_ entry: UserDictEntry) -> String? {
        var next = entries
        if let i = next.firstIndex(where: { $0.id == entry.id }) {
            next[i] = entry
        } else {
            next.append(entry)
        }
        return write(next)
    }

    func remove(_ ids: Set<UserDictEntry.ID>) -> String? {
        write(entries.filter { !ids.contains($0.id) })
    }

    private func write(_ newEntries: [UserDictEntry]) -> String? {
        var next = storage
        if next.dictionaries.isEmpty {
            next.dictionaries.append(UserDict(id: "", name: "User Dictionary", entries: []))
        }
        next.dictionaries[0].entries = newEntries
        do {
            try FileManager.default.createDirectory(at: Cssgsg.mozcProfileURL, withIntermediateDirectories: true)
            let json = String(decoding: try JSONEncoder().encode(next), as: UTF8.self)
            let ok = Self.url.path.withCString { path in json.withCString { cssgsg_userdict_save(path, $0) } }
            guard ok != 0 else { throw ConfigProblem.lastError }
        } catch let failure as ConfigProblem {
            problem = failure.message
            return failure.message
        } catch {
            problem = error.localizedDescription
            return error.localizedDescription
        }
        // 새 사전의 id는 저장할 때 매겨지므로 다시 읽는다. 줄 순서는 그대로라 화면의 id를 이어 붙인다.
        let ids = newEntries.map(\.id)
        load()
        if entries.count == ids.count {
            for i in entries.indices { entries[i].id = ids[i] }
        }
        Cssgsg.Notice.userDictionaryChanged.post()
        return nil
    }
}

// MARK: - 화면 (일본어 탭 안)

struct UserDictionarySection: View {
    @ObservedObject var dictionary: UserDictionaryModel
    @State private var search = ""
    @State private var selection = Set<UserDictEntry.ID>()
    @State private var editing: UserDictEntry?
    @State private var confirmDelete: Set<UserDictEntry.ID>?
    @State private var problem: String?

    private var shown: [UserDictEntry] {
        let q = search.trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return dictionary.entries }
        let reading = UserDictionaryModel.normalizedReading(q)
        return dictionary.entries.filter {
            $0.key.contains(reading) || $0.value.localizedCaseInsensitiveContains(q) || $0.comment.localizedCaseInsensitiveContains(q)
        }
    }

    var body: some View {
        Section {
            HStack {
                TextField(tr("찾기", "Search", "検索"), text: $search)
                    .textFieldStyle(.roundedBorder)
                Button(tr("추가…", "Add…", "追加…")) {
                    editing = UserDictEntry(key: "", value: "", comment: "", pos: 1)
                }
            }
            Table(shown, selection: $selection) {
                TableColumn(tr("읽기", "Reading", "読み"), value: \.key)
                TableColumn(tr("단어", "Word", "単語"), value: \.value)
                TableColumn(tr("품사", "Part of speech", "品詞")) { Text(MozcPOS.name($0.pos)) }
                TableColumn(tr("메모", "Note", "メモ"), value: \.comment)
            }
            .contextMenu(forSelectionType: UserDictEntry.ID.self) { ids in
                if ids.count == 1, let id = ids.first, let entry = dictionary.entries.first(where: { $0.id == id }) {
                    Button(tr("고치기…", "Edit…", "編集…")) { editing = entry }
                }
                if !ids.isEmpty {
                    Button(tr("지우기…", "Delete…", "削除…"), role: .destructive) { confirmDelete = ids }
                }
            } primaryAction: { ids in
                if let id = ids.first, let entry = dictionary.entries.first(where: { $0.id == id }) { editing = entry }
            }
            .frame(height: 240)
            HStack {
                Text(tr("\(dictionary.entries.count)개", "\(dictionary.entries.count) words", "\(dictionary.entries.count) 語"))
                    .monospacedDigit()
                    .foregroundStyle(.secondary)
                Spacer()
                Button(tr("고치기…", "Edit…", "編集…")) {
                    editing = dictionary.entries.first { selection.contains($0.id) }
                }
                .disabled(selection.count != 1)
                Button(tr("지우기…", "Delete…", "削除…")) { confirmDelete = selection }
                    .disabled(selection.isEmpty)
            }
            if let message = problem ?? dictionary.problem {
                Label(message, systemImage: "exclamationmark.triangle").foregroundStyle(.orange)
            }
        } header: {
            Text(tr("개인 사전", "User dictionary", "ユーザー辞書"))
        } footer: {
            Text(tr("""
                변환 후보에 낱말을 더한다(Mozc 사용자 사전). 읽기는 히라가나로 쓴다(가타카나는 히라가나로 바꿔 넣는다). \
                고치면 바로 적용된다. 두 번 누르면 고친다.
                """, """
                Adds words to the conversion candidates (Mozc user dictionary). Write readings in hiragana (katakana is converted). \
                Changes apply immediately. Double-click to edit.
                """, """
                変換候補に単語を加えます（Mozc のユーザー辞書）。読みはひらがなで入力します（カタカナはひらがなに直します）。\
                変更はすぐに反映されます。ダブルクリックで編集。
                """))
        }
        .sheet(item: $editing) { entry in
            UserDictEntryEditor(dictionary: dictionary, original: entry) { saved in
                selection = [saved.id]
                problem = nil
            }
        }
        .confirmationDialog(
            tr("낱말 \(confirmDelete?.count ?? 0)개를 지울까?", "Delete \(confirmDelete?.count ?? 0) word(s)?",
               "\(confirmDelete?.count ?? 0) 語を削除しますか？"),
            isPresented: Binding(get: { confirmDelete != nil }, set: { if !$0 { confirmDelete = nil } })
        ) {
            Button(tr("지우기", "Delete", "削除"), role: .destructive) {
                if let ids = confirmDelete {
                    problem = dictionary.remove(ids)
                    selection.subtract(ids)
                }
                confirmDelete = nil
            }
        }
    }
}

/// 낱말 하나 넣기·고치기.
struct UserDictEntryEditor: View {
    @ObservedObject var dictionary: UserDictionaryModel
    let original: UserDictEntry
    let saved: (UserDictEntry) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var draft: UserDictEntry
    @State private var problem: String?

    init(dictionary: UserDictionaryModel, original: UserDictEntry, saved: @escaping (UserDictEntry) -> Void) {
        self.dictionary = dictionary
        self.original = original
        self.saved = saved
        _draft = State(initialValue: original)
    }

    private var isNew: Bool { !dictionary.entries.contains { $0.id == original.id } }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(isNew ? tr("낱말 넣기", "Add a word", "単語を追加") : tr("낱말 고치기", "Edit word", "単語を編集"))
                .font(.headline)
            Form {
                TextField(tr("읽기", "Reading", "読み"), text: $draft.key, prompt: Text("くもつ"))
                TextField(tr("단어", "Word", "単語"), text: $draft.value, prompt: Text("雲津"))
                Picker(tr("품사", "Part of speech", "品詞"), selection: $draft.pos) {
                    ForEach(MozcPOS.choices + (MozcPOS.choices.contains(original.pos) ? [] : [original.pos]), id: \.self) { pos in
                        Text(MozcPOS.name(pos)).tag(pos)
                    }
                }
                TextField(tr("메모", "Note", "メモ"), text: $draft.comment)
            }
            if let problem {
                Label(problem, systemImage: "exclamationmark.triangle").foregroundStyle(.orange)
            }
            HStack {
                Spacer()
                Button(tr("취소", "Cancel", "キャンセル")) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button(tr("저장", "Save", "保存")) { save() }
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(width: 420)
    }

    private func save() {
        var entry = draft
        entry.key = UserDictionaryModel.normalizedReading(entry.key)
        entry.value = entry.value.trimmingCharacters(in: .whitespacesAndNewlines)
        entry.comment = entry.comment.trimmingCharacters(in: .whitespacesAndNewlines)
        if let reason = dictionary.problem(with: entry) ?? dictionary.upsert(entry) {
            problem = reason
            return
        }
        saved(entry)
        dismiss()
    }
}
