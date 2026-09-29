//! Mozc 사용자 사전 파일(`user_dictionary.db`) 읽기·쓰기. 설정 앱이 쓴다.
//!
//! 파일은 `mozc.user_dictionary.UserDictionaryStorage` 프로토콜 버퍼를 그대로 적은 것이다
//! (build/mozc/src/protocol/user_dictionary_storage.proto, Mozc UserDictionaryStorage::LoadInternal).
//! 필요한 필드만 직접 풀고 묶는다(프로토콜 버퍼 라이브러리를 들이지 않는다). 저장과 사전 단위의
//! 모르는 필드(예전 필드 번호 등)는 원래 바이트 그대로 남겨 다시 쓴다.
//!
//! - UserDictionaryStorage: version = 1 (int32), dictionaries = 2 (repeated UserDictionary)
//! - UserDictionary: id = 1 (uint64), name = 3, entries = 4 (repeated Entry)
//! - Entry: key = 1 (읽기), value = 2 (단어), comment = 4, pos = 5 (enum, 기본 NOUN = 1), locale = 12

use serde::{Deserialize, Serialize};

/// Mozc가 받아 주는 읽기·단어·메모 길이(UserDictionaryUtil의 한도와 같다).
pub const MAX_FIELD_LEN: usize = 300;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Storage {
    pub dictionaries: Vec<Dictionary>,
    /// 버전 필드(있으면 그대로 다시 쓴다).
    #[serde(skip)]
    version: Option<u64>,
    #[serde(skip)]
    unknown: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dictionary {
    /// u64라서 JSON에서는 문자열로 둔다. 새 사전이면 "" (저장할 때 새로 매긴다).
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub entries: Vec<Entry>,
    #[serde(skip)]
    unknown: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// 읽기(히라가나).
    pub key: String,
    /// 단어.
    pub value: String,
    #[serde(default)]
    pub comment: String,
    /// 품사(PosType 번호). 기본은 名詞(1).
    #[serde(default = "noun")]
    pub pos: u64,
    #[serde(default)]
    pub locale: String,
}

fn noun() -> u64 {
    1
}

// ---------------------------------------------------------------- 프로토콜 버퍼 선(wire) 형식

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

/// 필드 하나: 번호, 값, 원래 바이트(모르는 필드를 그대로 남길 때 쓴다).
type RawField<'a> = (u64, Field<'a>, &'a [u8]);

enum Field<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    /// 64비트·32비트 고정 길이(이 파일에는 없지만 모르는 필드로 남긴다).
    Fixed,
}

impl<'a> Reader<'a> {
    fn varint(&mut self) -> Result<u64, String> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let &b = self.buf.get(self.pos).ok_or("사전 파일이 중간에 끊겼다")?;
            self.pos += 1;
            value |= u64::from(b & 0x7F) << shift;
            if b & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err("사전 파일의 수가 너무 길다".into())
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end =
            self.pos.checked_add(n).filter(|&e| e <= self.buf.len()).ok_or("사전 파일이 중간에 끊겼다")?;
        let bytes = &self.buf[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    /// 다음 필드: (번호, 값, 원래 바이트).
    fn next(&mut self) -> Result<Option<RawField<'a>>, String> {
        if self.pos >= self.buf.len() {
            return Ok(None);
        }
        let start = self.pos;
        let tag = self.varint()?;
        let field = match tag & 7 {
            0 => Field::Varint(self.varint()?),
            1 => {
                self.take(8)?;
                Field::Fixed
            }
            2 => {
                let n = usize::try_from(self.varint()?).map_err(|_| "사전 파일의 길이가 너무 크다")?;
                Field::Bytes(self.take(n)?)
            }
            5 => {
                self.take(4)?;
                Field::Fixed
            }
            w => return Err(format!("사전 파일에 모르는 형식({w})이 있다")),
        };
        Ok(Some((tag >> 3, field, &self.buf[start..self.pos])))
    }
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn put_varint_field(out: &mut Vec<u8>, number: u64, v: u64) {
    put_varint(out, number << 3);
    put_varint(out, v);
}

fn put_bytes_field(out: &mut Vec<u8>, number: u64, bytes: &[u8]) {
    put_varint(out, (number << 3) | 2);
    put_varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

fn text(bytes: &[u8]) -> Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| "사전 파일의 글자가 UTF-8이 아니다".to_string())
}

// ---------------------------------------------------------------- 풀기·묶기

impl Storage {
    pub fn decode(buf: &[u8]) -> Result<Storage, String> {
        let mut r = Reader { buf, pos: 0 };
        let mut s = Storage::default();
        while let Some((number, field, raw)) = r.next()? {
            match (number, field) {
                (1, Field::Varint(v)) => s.version = Some(v),
                (2, Field::Bytes(b)) => s.dictionaries.push(Dictionary::decode(b)?),
                _ => s.unknown.extend_from_slice(raw),
            }
        }
        Ok(s)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        if let Some(v) = self.version {
            put_varint_field(&mut out, 1, v);
        }
        for d in &self.dictionaries {
            put_bytes_field(&mut out, 2, &d.encode());
        }
        out.extend_from_slice(&self.unknown);
        out
    }
}

impl Dictionary {
    fn decode(buf: &[u8]) -> Result<Dictionary, String> {
        let mut r = Reader { buf, pos: 0 };
        let mut d = Dictionary::default();
        while let Some((number, field, raw)) = r.next()? {
            match (number, field) {
                (1, Field::Varint(v)) => d.id = v.to_string(),
                (3, Field::Bytes(b)) => d.name = text(b)?,
                (4, Field::Bytes(b)) => d.entries.push(Entry::decode(b)?),
                _ => d.unknown.extend_from_slice(raw),
            }
        }
        Ok(d)
    }

    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        if let Ok(id) = self.id.parse::<u64>() {
            put_varint_field(&mut out, 1, id);
        }
        put_bytes_field(&mut out, 3, self.name.as_bytes());
        for e in &self.entries {
            put_bytes_field(&mut out, 4, &e.encode());
        }
        out.extend_from_slice(&self.unknown);
        out
    }
}

impl Entry {
    fn decode(buf: &[u8]) -> Result<Entry, String> {
        let mut r = Reader { buf, pos: 0 };
        let mut e = Entry {
            key: String::new(),
            value: String::new(),
            comment: String::new(),
            pos: noun(),
            locale: String::new(),
        };
        while let Some((number, field, _)) = r.next()? {
            match (number, field) {
                (1, Field::Bytes(b)) => e.key = text(b)?,
                (2, Field::Bytes(b)) => e.value = text(b)?,
                (4, Field::Bytes(b)) => e.comment = text(b)?,
                (5, Field::Varint(v)) => e.pos = v,
                (12, Field::Bytes(b)) => e.locale = text(b)?,
                // 예전 필드(3 pos 옛 형식, 10 removed, 11 auto_registered)는 버린다.
                _ => {}
            }
        }
        Ok(e)
    }

    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_bytes_field(&mut out, 1, self.key.as_bytes());
        put_bytes_field(&mut out, 2, self.value.as_bytes());
        if !self.comment.is_empty() {
            put_bytes_field(&mut out, 4, self.comment.as_bytes());
        }
        put_varint_field(&mut out, 5, self.pos);
        if !self.locale.is_empty() {
            put_bytes_field(&mut out, 12, self.locale.as_bytes());
        }
        out
    }

    /// Mozc가 받아 줄 항목인지(비었거나 너무 길거나 탭·줄바꿈이 들었으면 안 된다).
    pub fn validate(&self) -> Result<(), String> {
        for (name, v) in [("읽기", &self.key), ("단어", &self.value), ("메모", &self.comment)] {
            if v.chars().count() > MAX_FIELD_LEN {
                return Err(format!("{name}가 너무 길다({MAX_FIELD_LEN}자까지)"));
            }
            if v.contains(['\t', '\n', '\r']) {
                return Err(format!("{name}에 탭이나 줄바꿈이 있다"));
            }
        }
        if self.key.is_empty() || self.value.is_empty() {
            return Err("읽기와 단어를 모두 적어야 한다".into());
        }
        if self.pos > 44 {
            return Err(format!("모르는 품사 번호 {}", self.pos));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- 파일

/// 사전 파일을 읽는다. 없으면 빈 저장소.
pub fn load(path: &str) -> Result<Storage, String> {
    match std::fs::read(path) {
        Ok(bytes) => Storage::decode(&bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Storage::default()),
        Err(e) => Err(format!("사전 파일을 읽지 못했다: {e}")),
    }
}

/// 사전들을 파일에 쓴다. 지금 파일의 사전과 id가 같으면 그 사전의 모르는 필드와 저장소의 필드를 그대로 둔다.
/// id가 비었으면 새 번호를 매긴다. 한꺼번에 바꿔 쓴다(임시 파일 → 이름 바꾸기).
pub fn save(path: &str, dictionaries: Vec<Dictionary>) -> Result<(), String> {
    for d in &dictionaries {
        for e in &d.entries {
            e.validate().map_err(|err| format!("{} → {}: {err}", e.key, e.value))?;
        }
    }
    let Storage { dictionaries: old, version, unknown } = load(path)?;
    let mut storage = Storage { dictionaries: Vec::new(), version, unknown };
    for mut d in dictionaries {
        match old.iter().find(|o| !d.id.is_empty() && o.id == d.id) {
            Some(o) => d.unknown = o.unknown.clone(),
            None if d.id.parse::<u64>().is_err() => d.id = new_id(&storage.dictionaries, &old).to_string(),
            None => {}
        }
        storage.dictionaries.push(d);
    }
    let path = std::path::Path::new(path);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("사전 폴더를 만들지 못했다: {e}"))?;
    }
    let tmp = path.with_extension("db.cssgsg-tmp");
    std::fs::write(&tmp, storage.encode()).map_err(|e| format!("사전 파일을 쓰지 못했다: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("사전 파일을 바꾸지 못했다: {e}"))
}

/// 겹치지 않는 새 사전 번호(0이 아닌 값).
fn new_id(new: &[Dictionary], old: &[Dictionary]) -> u64 {
    let used: Vec<&str> = new.iter().chain(old).map(|d| d.id.as_str()).collect();
    let seed =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
    let mut id = seed | 1;
    while used.contains(&id.to_string().as_str()) {
        id = id.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1;
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, value: &str, pos: u64) -> Entry {
        Entry { key: key.into(), value: value.into(), comment: String::new(), pos, locale: String::new() }
    }

    #[test]
    fn encodes_the_proto_wire_format() {
        // 손으로 만든 바이트: storage{ version:1, dictionaries:[{ id:7, name:"a", entries:[{key:"か", value:"火", pos:1}] }] }
        let entry_bytes = [&[0x0A, 3][..], "か".as_bytes(), &[0x12, 3], "火".as_bytes(), &[0x28, 1]].concat();
        let dict_bytes =
            [&[0x08, 7, 0x1A, 1, b'a', 0x22, entry_bytes.len() as u8][..], &entry_bytes].concat();
        let file = [&[0x08, 1, 0x12, dict_bytes.len() as u8][..], &dict_bytes].concat();
        let s = Storage::decode(&file).unwrap();
        assert_eq!(s.dictionaries[0].id, "7");
        assert_eq!(s.dictionaries[0].entries, vec![entry("か", "火", 1)]);
        assert_eq!(s.encode(), file, "같은 바이트로 되돌아간다");
    }

    #[test]
    fn keeps_unknown_fields_and_round_trips_files() {
        // 사전에 모르는 필드 6(예전 syncable = true)와 저장소에 필드 10(예전 storage_type = 3)
        let d = Dictionary {
            id: "42".into(),
            name: "사전".into(),
            entries: vec![entry("にほん", "日本", 1)],
            unknown: vec![0x30, 1],
        };
        let s = Storage { dictionaries: vec![d], version: None, unknown: vec![0x50, 3] };
        let back = Storage::decode(&s.encode()).unwrap();
        assert_eq!(back, s);

        let dir = std::env::temp_dir().join(format!("cssgsg-userdict-{}", std::process::id()));
        let path = dir.join("user_dictionary.db");
        let path = path.to_str().unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(path, s.encode()).unwrap();
        // 설정 앱은 JSON으로 받은 사전을 고쳐서 돌려준다(모르는 필드는 모른다).
        let mut edited: Vec<Dictionary> = load(path)
            .unwrap()
            .dictionaries
            .into_iter()
            .map(|d| Dictionary { unknown: vec![], ..d })
            .collect();
        edited[0].entries.push(entry("かな", "仮名", 1));
        edited.push(Dictionary {
            id: String::new(),
            name: "새 사전".into(),
            entries: vec![],
            unknown: vec![],
        });
        save(path, edited).unwrap();
        let saved = load(path).unwrap();
        assert_eq!(saved.dictionaries[0].unknown, vec![0x30, 1], "사전의 모르는 필드는 남는다");
        assert_eq!(saved.unknown, vec![0x50, 3], "저장소의 모르는 필드도 남는다");
        assert_eq!(saved.dictionaries[0].entries.len(), 2);
        assert!(saved.dictionaries[1].id.parse::<u64>().unwrap() > 0, "새 사전에는 번호를 매긴다");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_bad_entries_and_files() {
        assert!(entry("", "日本", 1).validate().is_err());
        assert!(entry("にほん", "日本\t", 1).validate().is_err());
        assert!(entry("にほん", "日本", 99).validate().is_err());
        assert!(entry(&"あ".repeat(301), "日本", 1).validate().is_err());
        assert!(entry("にほん", "日本", 5).validate().is_ok());
        assert!(Storage::decode(&[0x12, 10, 1]).is_err(), "끊긴 파일");
        assert_eq!(load("/nonexistent/cssgsg/user_dictionary.db").unwrap(), Storage::default());
    }
}
