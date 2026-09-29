//! 가나→한자 변환기 인터페이스.
//!
//! 실제 변환은 Mozc가 한다(맥: 입력기 프로세스 안의 엔진 스레드, 윈도우: 엔진 호스트).
//! 코어는 읽기를 넘기고 결과 화면(문절·후보)을 받아 표시만 한다. 셸이 이 트레이트를 구현해서 넣는다.
//! 엔진이 아직 준비되지 않았으면 `start`가 `None`을 돌려주고, 입력 스레드는 기다리지 않는다.

/// 변환 중 명령. Mozc 세션 명령과 대응한다. 페이지 넘김·격자 이동은 엔진이 목표 번호를 계산해 `Select`로 보낸다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvCmd {
    Next,
    Prev,
    FocusLeft,
    FocusRight,
    Shrink,
    Expand,
    /// 포커스된 문절의 후보 중 n번째(0부터, 전체 목록 기준)를 고른다.
    Select(usize),
}

/// 변환 결과 화면.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConvView {
    /// 문절별 지금 선택된 글자.
    pub segments: Vec<String>,
    /// 포커스된 문절.
    pub focused: usize,
    /// 포커스된 문절의 후보 전체. 페이지는 화면(후보창)이 나눈다.
    pub candidates: Vec<String>,
    /// 지금 고른 후보(전체 목록 기준).
    pub selected: Option<usize>,
}

pub trait Converter {
    /// 읽기로 변환을 시작한다. 변환 엔진이 준비되지 않았으면 `None`.
    fn start(&mut self, reading: &str) -> Option<ConvView>;
    /// 변환 중 명령. 실패하면 `None`(화면은 그대로 둔다).
    fn command(&mut self, cmd: ConvCmd) -> Option<ConvView>;
    /// 지금 결과를 확정하고 세션을 끝낸다.
    fn commit(&mut self) -> String;
    /// 변환을 취소한다(읽기로 돌아간다).
    fn cancel(&mut self);
}

/// Mozc를 붙이기 전까지 쓰는 변환기: 후보가 [히라가나, 가타카나] 둘뿐이다.
#[derive(Debug, Default)]
pub struct EchoConverter {
    candidates: Vec<String>,
    selected: usize,
}

impl EchoConverter {
    fn view(&self) -> ConvView {
        ConvView {
            segments: vec![self.candidates[self.selected].clone()],
            focused: 0,
            candidates: self.candidates.clone(),
            selected: Some(self.selected),
        }
    }
}

impl Converter for EchoConverter {
    fn start(&mut self, reading: &str) -> Option<ConvView> {
        let kata = crate::kana::to_katakana(reading);
        self.candidates = vec![reading.to_string()];
        if kata != reading {
            self.candidates.push(kata);
        }
        self.selected = 0;
        Some(self.view())
    }

    fn command(&mut self, cmd: ConvCmd) -> Option<ConvView> {
        if self.candidates.is_empty() {
            return None;
        }
        let n = self.candidates.len();
        match cmd {
            ConvCmd::Next => self.selected = (self.selected + 1) % n,
            ConvCmd::Prev => self.selected = (self.selected + n - 1) % n,
            ConvCmd::Select(i) if i < n => self.selected = i,
            ConvCmd::Select(_) => return None,
            _ => {}
        }
        Some(self.view())
    }

    fn commit(&mut self) -> String {
        let text = self.candidates.get(self.selected).cloned().unwrap_or_default();
        self.cancel();
        text
    }

    fn cancel(&mut self) {
        self.candidates.clear();
        self.selected = 0;
    }
}
