//! 참신세벌식(첫가끝 갈마들이) 조합기.
//!
//! 키 하나가 초성·중성·종성·기호 중 여러 역할을 가진다. 어떤 역할로 쓸지는 지금 음절 상태로 정한다.
//! 판정 규칙은 원작자 사용법과 오이(Online Hangeul IME)의 참신세벌식 동작을 따른다.
//!
//! - 오른손 모음 키(o=ㄹ/ㅗ, b=ㅋ/ㅑ, .=./ㅜ)는 초성만 있을 때만 모음이고, 그 밖에는 초성이나 기호다.
//! - 왼손 키(모음/받침)는 초성 뒤에서 모음, 모음 뒤에서 받침이다.
//!   오른손 ㅗ/ㅜ 뒤에서는 겹모음을 먼저 시도한다(ㄱ+o+f=과). 왼손 모음 뒤에서는 곧바로 받침이다(ㄱ+g+f=곧).
//! - 받침 뒤 왼손 키: 겹받침이 되면 겹받침, 안 되면 새 받침 낱자(한+f → 한ㄷ).
//! - 모음만 있는 낱자나 받침 낱자 뒤의 왼손 키는 모음이다(ㅏ+e → ㅏㅔ). 받침은 초성+중성 뒤에만 붙는다.
//! - 기호+받침 겸용 키(Shift+B, D의 숫자열)는 초성+중성 뒤(받침 없음)에서만 받침이다.
//! - Backspace는 키 입력 하나를 되돌린다(음절 단위가 아니다).

use super::layout::{Action, KoLayout, Roles};
use super::syllable;

/// 중성이 어떻게 들어왔는지. 겹모음 판정에 쓴다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Src {
    /// 왼손 모음 키(또는 초성 없는 모음).
    #[default]
    Left,
    /// 오른손 ㅗ/ㅜ(초성 바로 뒤). 왼손 모음과 겹모음을 만들 수 있다.
    Right,
    /// 이미 겹모음이 됐다.
    Compound,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Syllable {
    cho: Option<char>,
    jung: Option<char>,
    src: Src,
    jong: Option<char>,
}

impl Syllable {
    fn is_empty(&self) -> bool {
        self.cho.is_none() && self.jung.is_none() && self.jong.is_none()
    }

    fn render(&self) -> String {
        if let (Some(c), Some(v)) = (self.cho, self.jung) {
            if let Some(s) = syllable(c, v, self.jong) {
                return s.to_string();
            }
        }
        [self.cho, self.jung, self.jong].into_iter().flatten().collect()
    }
}

/// 조합기가 키 하나를 처리한 결과. `commit`은 이 키 때문에 확정된 앞 음절이다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KoResult {
    /// 조합 중인 음절이 바뀌었다.
    Composed { commit: String },
    /// 음절을 확정하고 기호를 낸다.
    Symbol { commit: String, sym: String },
    /// ❖: 음절을 확정하고 아무것도 내지 않는다.
    Stopped { commit: String },
}

/// 한 키가 음절에 하는 일.
#[derive(Debug)]
enum Op {
    ExtendCho(char),
    SetJung(char, Src),
    ExtendJung(char),
    SetJong(char),
    ExtendJong(char),
    NewCho(char),
    NewJung(char),
    NewJong(char),
    Sym(String),
    Stop,
}

#[derive(Debug, Default, Clone)]
pub struct KoComposer {
    cur: Syllable,
    /// 지금 음절의 키 입력별 이전 상태. Backspace가 하나씩 되돌린다.
    history: Vec<Syllable>,
}

impl KoComposer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_composing(&self) -> bool {
        !self.cur.is_empty()
    }

    /// 화면에 보일 조합 중 글자.
    pub fn preedit(&self) -> String {
        self.cur.render()
    }

    /// 지금 음절을 확정해서 돌려주고 비운다.
    pub fn flush(&mut self) -> String {
        let text = self.cur.render();
        self.clear();
        text
    }

    /// 확정 없이 비운다(포커스가 바뀔 때).
    pub fn clear(&mut self) {
        self.cur = Syllable::default();
        self.history.clear();
    }

    /// 키 입력 하나를 되돌린다. 되돌릴 게 없으면 false(앱이 Backspace를 처리한다).
    pub fn backspace(&mut self) -> bool {
        match self.history.pop() {
            Some(prev) => {
                self.cur = prev;
                true
            }
            None => false,
        }
    }

    pub fn input(&mut self, layout: &KoLayout, roles: &Roles) -> KoResult {
        let op = self.decide(layout, roles);
        self.apply(op)
    }

    fn decide(&self, l: &KoLayout, r: &Roles) -> Op {
        let s = self.cur;
        if r.action == Some(Action::Stop) {
            return Op::Stop;
        }

        // 오른손 모음 키: 초성만 있을 때만 모음, 그 밖에는 초성이나 기호.
        if let Some(v) = r.jung {
            if r.cho.is_some() || r.sym.is_some() {
                if s.cho.is_some() && s.jung.is_none() {
                    return Op::SetJung(v, Src::Right);
                }
                if let Some(c) = r.cho {
                    return self.cho_op(l, c);
                }
                return Op::Sym(r.sym.clone().unwrap_or_default());
            }
        }

        if let Some(c) = r.cho {
            return self.cho_op(l, c);
        }

        // 기호+받침 겸용 키: 초성+중성 뒤(받침 없음)에서만 받침.
        if let (Some(sym), Some(j)) = (&r.sym, r.jong) {
            if s.cho.is_some() && s.jung.is_some() && s.jong.is_none() {
                return Op::SetJong(j);
            }
            return Op::Sym(sym.clone());
        }

        if r.jung.is_some() || r.jong.is_some() {
            return self.left_op(l, r.jung, r.jong);
        }

        Op::Sym(r.sym.clone().unwrap_or_default())
    }

    fn cho_op(&self, l: &KoLayout, c: char) -> Op {
        let s = self.cur;
        if let (Some(prev), None) = (s.cho, s.jung) {
            if let Some(cc) = l.combine_cho(prev, c) {
                return Op::ExtendCho(cc);
            }
        }
        Op::NewCho(c)
    }

    /// 왼손 키(모음 그리고/또는 받침).
    fn left_op(&self, l: &KoLayout, jung: Option<char>, jong: Option<char>) -> Op {
        let s = self.cur;
        let new_vowel_or_jong = || match (jung, jong) {
            (Some(v), _) => Op::NewJung(v),
            (None, Some(j)) => Op::NewJong(j),
            (None, None) => unreachable!("left_op은 모음이나 받침이 있는 키만 받는다"),
        };
        match (s.cho, s.jung, s.jong) {
            // 빈 상태: 모음만 있는 낱자를 시작한다.
            (None, None, None) => new_vowel_or_jong(),
            // 초성만: 왼손 모음.
            (Some(_), None, _) => match jung {
                Some(v) => Op::SetJung(v, Src::Left),
                None => new_vowel_or_jong(),
            },
            // 초성+중성: 오른손 ㅗ/ㅜ 뒤면 겹모음을 먼저, 아니면 받침.
            (Some(_), Some(v0), None) => {
                if s.src == Src::Right {
                    if let Some(cv) = jung.and_then(|v| l.combine_jung(v0, v)) {
                        return Op::ExtendJung(cv);
                    }
                }
                match jong {
                    Some(j) => Op::SetJong(j),
                    None => new_vowel_or_jong(),
                }
            }
            // 초성+중성+종성: 겹받침, 안 되면 새 받침 낱자.
            (Some(_), Some(_), Some(j0)) => match jong {
                Some(j) => match l.combine_jong(j0, j) {
                    Some(cj) => Op::ExtendJong(cj),
                    None => Op::NewJong(j),
                },
                None => new_vowel_or_jong(),
            },
            // 모음만 있는 낱자: 왼손끼리 겹모음, 안 되면 새 모음.
            (None, Some(v0), _) => {
                if let Some(cv) = jung.and_then(|v| l.combine_jung(v0, v)) {
                    return Op::ExtendJung(cv);
                }
                new_vowel_or_jong()
            }
            // 받침 낱자: 새 모음.
            (None, None, Some(_)) => new_vowel_or_jong(),
        }
    }

    fn apply(&mut self, op: Op) -> KoResult {
        let mut commit = String::new();
        match op {
            Op::ExtendCho(c) => {
                self.history.push(self.cur);
                self.cur.cho = Some(c);
            }
            Op::SetJung(v, src) => {
                self.history.push(self.cur);
                self.cur.jung = Some(v);
                self.cur.src = src;
            }
            Op::ExtendJung(v) => {
                self.history.push(self.cur);
                self.cur.jung = Some(v);
                self.cur.src = Src::Compound;
            }
            Op::SetJong(j) | Op::ExtendJong(j) => {
                self.history.push(self.cur);
                self.cur.jong = Some(j);
            }
            Op::NewCho(c) => {
                commit = self.flush();
                self.start(Syllable { cho: Some(c), ..Default::default() });
            }
            Op::NewJung(v) => {
                commit = self.flush();
                self.start(Syllable { jung: Some(v), ..Default::default() });
            }
            Op::NewJong(j) => {
                commit = self.flush();
                self.start(Syllable { jong: Some(j), ..Default::default() });
            }
            Op::Sym(sym) => {
                return KoResult::Symbol { commit: self.flush(), sym };
            }
            Op::Stop => {
                return KoResult::Stopped { commit: self.flush() };
            }
        }
        KoResult::Composed { commit }
    }

    fn start(&mut self, s: Syllable) {
        self.history.clear();
        self.history.push(Syllable::default());
        self.cur = s;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;

    /// 쿼티 글자로 적은 키열을 넣고 (확정분 + 조합 중) 글자를 돌려준다. 대문자는 Shift.
    /// 배열에 없는 키는 쿼티 글자 그대로 나간다(앱이 받는 것처럼).
    fn type_keys(layout_id: &str, keys: &str) -> String {
        let l = KoLayout::builtin(layout_id).unwrap();
        let mut c = KoComposer::new();
        let mut out = String::new();
        for ch in keys.chars() {
            if ch == '<' {
                if !c.backspace() {
                    out.pop();
                }
                continue;
            }
            if ch == ' ' {
                out += &c.flush();
                out.push(' ');
                continue;
            }
            let shift = ch.is_ascii_uppercase() || "~!@#$%^&*()_+{}|:\"<>?".contains(ch);
            let base = if ch.is_ascii_uppercase() { ch.to_ascii_lowercase() } else { ch };
            let key = Key::from_qwerty(base)
                .or_else(|| (0..0x40).map(Key).find(|k| k.qwerty_char(true) == Some(ch)))
                .unwrap_or_else(|| panic!("키 없음: {ch:?}"));
            match l.roles(key, shift) {
                Some(r) => match c.input(&l, r) {
                    KoResult::Composed { commit } | KoResult::Stopped { commit } => out += &commit,
                    KoResult::Symbol { commit, sym } => {
                        out += &commit;
                        out += &sym;
                    }
                },
                None => {
                    out += &c.flush();
                    out.push(key.qwerty_char(shift).unwrap());
                }
            }
        }
        out + &c.preedit()
    }

    fn v18(keys: &str) -> String {
        type_keys("chamshin-v18", keys)
    }

    /// 오이(Online Hangeul IME, 최신 조합표 패치본)에서 확인한 결과.
    #[test]
    fn matches_ohi_oracle() {
        let cases = [
            ("kf", "가"),
            ("kof", "과"),
            ("hfs", "한"),
            ("jdkv", "이거"),
            ("k.v", "궈"),
            ("u.vB", "뭨"),
            ("jkf", "까"),
            ("kkf", "까"),
            ("njf", "짜"),
            ("hfsd", "핞"),
            ("kfwe", "갉"),
            ("bf", "카"),
            ("kb", "갸"),
            ("kfB", "갘"),
            ("kf.", "가."),
            ("kfea", "갘"),
            ("kfda", "갎"),
            ("fe", "ㅏㅔ"),
            ("fj", "ㅏㅇ"),
            ("hfsf", "한ㄷ"),
            ("hfsff", "한ㄷㅏ"),
            ("hfsfd", "한ㄷㅣ"),
            ("gf", "ㅘ"),
            ("kgf", "곧"),
            ("kgfe", "곧ㄱ"),
            ("kob", "고ㅋ"),
            ("bb", "캬"),
            ("jb", "야"),
            ("jNb", "ㅇㅋ"),
            ("kB", "ㄱ△"),
            ("kfeB", "각△"),
            ("k..", "구."),
            (".k", ".ㄱ"),
            ("ob", "랴"),
            ("oo", "로"),
            ("kfo", "가ㄹ"),
            ("jjf", "ㅇ아"),
            ("kdf", "긷"),
            ("df", "ㅑ"),
            ("cd", "ㅢ"),
            ("kcd", "긍"),
            ("k.d", "귀"),
            ("k.f", "굳"),
            ("kogd", "곺ㅇ"),
            ("fef", "ㅏㅔㅏ"),
            ("kfQ", "가「"),
            ("kf;", "가ㅂ"),
            ("kf/", "가ㅍ"),
            ("kfL", "가/"),
            ("kfP", "가;"),
            ("kfN", "가"),
            ("kfNf", "가ㅏ"),
            ("N", ""),
            ("Nk", "ㄱ"),
            ("B", "△"),
            ("fB", "ㅏ△"),
            ("fgB", "ㅏㅗ△"),
            ("kfewB", "갉△"),
            ("hfswe", "한ㄹㅔ"),
            ("hfswf", "한ㄹㅏ"),
            ("gfd", "ㅘㅣ"),
            ("gs", "ㅙ"),
            ("fkf", "ㅏ가"),
            ("k.o", "구ㄹ"),
            ("ko.", "고."),
            ("kos", "괘"),
            ("k.e", "궤"),
            ("kfs.k", "간.ㄱ"),
            ("jfd", "앙"),
            ("jfdf", "앙ㄷ"),
            ("kfxz", "값"),
            ("kfzx", "값"),
            ("kfeq", "갃"),
            ("kfaw", "갊"),
            ("kfwa", "갊"),
            ("bff", "칻"),
            ("kfdw", "갏"),
            ("kfdc", "강ㅎ"),
        ];
        for (keys, want) in cases {
            assert_eq!(v18(keys), want, "키열 {keys:?}");
        }
    }

    #[test]
    fn everyday_words() {
        assert_eq!(v18("jfsmtdhfleja"), "안녕하세요");
        assert_eq!(v18("kfalfhfzmdif"), "감사합니다");
        assert_eq!(v18("kofnf"), "과자");
        assert_eq!(v18("jvzxif"), "없다");
        assert_eq!(v18("jdewif"), "읽다");
        assert_eq!(v18("jlfd"), "쌍");
        assert_eq!(v18("jos jw j.d"), "왜 의 위");
        assert_eq!(v18("bNbNb"), "ㅋㅋㅋ");
    }

    #[test]
    fn backspace_undoes_one_key() {
        assert_eq!(v18("kof<"), "고");
        assert_eq!(v18("kof<<"), "ㄱ");
        assert_eq!(v18("kfwe<"), "갈");
        assert_eq!(v18("jkf<"), "ㄲ");
        assert_eq!(v18("jkf<<"), "ㅇ");
        assert_eq!(v18("jkf<<<"), "");
        // 음절이 확정된 뒤의 Backspace는 앱으로 간다(여기서는 확정분에서 한 글자 지움).
        assert_eq!(v18("hfsj<"), "한");
        assert_eq!(v18("hfsj<<"), "");
    }

    #[test]
    fn d_variant_number_row_finals() {
        let d = |k| type_keys("chamshin-d-v19", k);
        assert_eq!(d("kf3"), "갇");
        assert_eq!(d("kf2"), "갖");
        assert_eq!(d("kf4"), "갚");
        assert_eq!(d("kfe3"), "각3");
        assert_eq!(d("/a3"), "푣");
        assert_eq!(d("/aN3"), "표3");
        assert_eq!(d("kff"), "갘");
        assert_eq!(d("kgf"), "곸");
        assert_eq!(d("kf4w"), "갎");
        assert_eq!(d("3"), "3");
    }
}
