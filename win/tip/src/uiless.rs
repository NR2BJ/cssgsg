//! 후보창을 직접 그리는 앱(UILess 게임: TSF를 TF_TMAE_UIELEMENTENABLEDONLY로 켠다)에 후보 목록을 넘긴다.
//!
//! 후보가 뜨면 ITfUIElementMgr::BeginUIElement로 알리고, 앱이 "내가 그린다"(show = FALSE)고 답하면 우리 후보창은 띄우지 않는다.
//! 후보가 바뀌면 UpdateUIElement, 닫히면 EndUIElement. 앱은 [`CandidateList`](ITfCandidateListUIElement)로 목록·고른 것·페이지를
//! 읽어 자기 모양으로 그린다. 오버워치는 MS 일본어 입력기의 후보를 이렇게 받아 게임 색으로 그린다(2026-10-02 사용자). 0.2.3까지
//! cssgsg는 이것이 없어 우리 창이 모니터 오른쪽 아래에 떴다.
//!
//! 게임 스레드에서만 쓴다(보통 앱은 지금처럼 우리 창). 앱이 고르게 하는 ITfCandidateListUIElementBehavior는 아직 없다.

use std::cell::{Cell, RefCell};

use cssgsg_core::engine::{CAND_GRID_PAGE, CAND_LIST_PAGE, Candidates};
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::UI::TextServices::{
    ITfCandidateListUIElement, ITfCandidateListUIElement_Impl, ITfDocumentMgr, ITfThreadMgr,
    ITfUIElement_Impl, ITfUIElementMgr, TF_CLUIE_COUNT, TF_CLUIE_CURRENTPAGE, TF_CLUIE_DOCUMENTMGR,
    TF_CLUIE_PAGEINDEX, TF_CLUIE_SELECTION, TF_CLUIE_STRING,
};
use windows::core::{BOOL, BSTR, ComObject, GUID, Interface, Result, implement};

use crate::debug_log;

/// cssgsg 후보 목록 UI 요소.
const GUID_CANDIDATE_LIST: GUID = GUID::from_u128(0x8E1F5A3C_2B7D_4C69_9A41_6D0C3E2B5F17);

/// 앱에 보일 후보 목록 한 판.
#[derive(Clone, Debug, Default, PartialEq)]
struct ListData {
    /// 후보(뜻이 있으면 붙여 "國 나라 국").
    items: Vec<String>,
    selected: u32,
    /// 한 페이지의 후보 수(목록 9, 격자 30: 우리 후보창과 같게 나눈다).
    page_size: usize,
}

impl ListData {
    fn from(c: &Candidates) -> Self {
        let items = c
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| match c.notes.get(i).filter(|n| !n.is_empty()) {
                Some(note) => format!("{item} {note}"),
                None => item.clone(),
            })
            .collect();
        Self {
            items,
            selected: c.selected.unwrap_or(0) as u32,
            page_size: if c.grid { CAND_GRID_PAGE } else { CAND_LIST_PAGE },
        }
    }

    /// 페이지마다 첫 후보의 번호.
    fn page_starts(&self) -> Vec<u32> {
        (0..self.items.len().max(1)).step_by(self.page_size.max(1)).map(|i| i as u32).collect()
    }

    fn current_page(&self) -> u32 {
        self.selected / self.page_size.max(1) as u32
    }
}

/// 앱에 보이는 후보 목록. 앱이 아무 때나 읽으니 후보가 바뀔 때마다 통째로 바꿔 둔다.
#[implement(ITfCandidateListUIElement, Agile = false)]
pub struct CandidateList {
    thread_mgr: ITfThreadMgr,
    data: RefCell<ListData>,
    /// 우리 UI를 보이고 있는지(앱이 Show로 정한다).
    shown: Cell<bool>,
}

impl ITfUIElement_Impl for CandidateList_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("cssgsg candidates"))
    }

    fn GetGUID(&self) -> Result<GUID> {
        Ok(GUID_CANDIDATE_LIST)
    }

    fn Show(&self, show: BOOL) -> Result<()> {
        self.shown.set(show.as_bool());
        Ok(())
    }

    fn IsShown(&self) -> Result<BOOL> {
        Ok(self.shown.get().into())
    }
}

impl ITfCandidateListUIElement_Impl for CandidateList_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        // 후보가 바뀔 때마다 통째로 바꾸므로 늘 전부 바뀌었다고 한다.
        Ok(TF_CLUIE_DOCUMENTMGR
            | TF_CLUIE_COUNT
            | TF_CLUIE_SELECTION
            | TF_CLUIE_STRING
            | TF_CLUIE_PAGEINDEX
            | TF_CLUIE_CURRENTPAGE)
    }

    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        unsafe { self.thread_mgr.GetFocus() }
    }

    fn GetCount(&self) -> Result<u32> {
        Ok(self.data.borrow().items.len() as u32)
    }

    fn GetSelection(&self) -> Result<u32> {
        Ok(self.data.borrow().selected)
    }

    fn GetString(&self, index: u32) -> Result<BSTR> {
        self.data
            .borrow()
            .items
            .get(index as usize)
            .map(|s| BSTR::from(s.as_str()))
            .ok_or_else(|| E_INVALIDARG.into())
    }

    /// 페이지마다 첫 후보의 번호. `index`가 null이면 페이지 수만.
    fn GetPageIndex(&self, index: *mut u32, size: u32, count: *mut u32) -> Result<()> {
        let starts = self.data.borrow().page_starts();
        unsafe {
            if !count.is_null() {
                *count = starts.len() as u32;
            }
            if !index.is_null() {
                for (i, start) in starts.iter().take(size as usize).enumerate() {
                    *index.add(i) = *start;
                }
            }
        }
        Ok(())
    }

    /// 페이지 나누기는 엔진(우리 후보창과 같게)이 정한다.
    fn SetPageIndex(&self, _index: *const u32, _count: u32) -> Result<()> {
        Ok(())
    }

    fn GetCurrentPage(&self) -> Result<u32> {
        Ok(self.data.borrow().current_page())
    }
}

/// 게임 스레드의 후보를 앱에 알리는 다리.
pub struct GameCandidates {
    manager: ITfUIElementMgr,
    element: ComObject<CandidateList>,
    /// 앱에 알린 UI 요소(후보가 떠 있는 동안).
    id: Option<u32>,
    /// 앱이 그린다고 했다(우리 창은 띄우지 않는다).
    app_draws: bool,
}

impl GameCandidates {
    pub fn new(thread_mgr: &ITfThreadMgr) -> Option<Self> {
        let manager = thread_mgr.cast::<ITfUIElementMgr>().ok()?;
        let element = ComObject::new(CandidateList {
            thread_mgr: thread_mgr.clone(),
            data: RefCell::default(),
            shown: Cell::new(false),
        });
        Some(Self { manager, element, id: None, app_draws: false })
    }

    /// 후보를 앱에 알린다. 앱이 그리면 true(우리 후보창은 띄우지 않는다).
    pub fn show(&mut self, c: &Candidates) -> bool {
        *self.element.data.borrow_mut() = ListData::from(c);
        unsafe {
            match self.id {
                Some(id) => {
                    let _ = self.manager.UpdateUIElement(id);
                }
                None => {
                    let mut show = BOOL::from(true);
                    let mut id = 0;
                    let element: ITfCandidateListUIElement = self.element.to_interface();
                    match self.manager.BeginUIElement(&element, &mut show, &mut id) {
                        Ok(()) => {
                            self.id = Some(id);
                            self.app_draws = !show.as_bool();
                            self.element.shown.set(show.as_bool());
                            debug_log(&format!(
                                "candidate UI element {id}: the app draws it {}",
                                self.app_draws
                            ));
                            // 시작 알림만으로는 목록을 다시 읽지 않는 앱이 있다(오버워치: 새 후보창에 앞 글자의 후보가 보이다가
                            // 방향키를 누르면 바뀌었다, 2026-10-03). 바로 갱신도 알린다.
                            if self.app_draws {
                                let _ = self.manager.UpdateUIElement(id);
                            }
                        }
                        Err(e) => {
                            debug_log(&format!("BeginUIElement: {e:?}"));
                            self.app_draws = false;
                        }
                    }
                }
            }
        }
        self.app_draws
    }

    /// 후보가 닫혔다.
    pub fn hide(&mut self) {
        if let Some(id) = self.id.take() {
            unsafe {
                let _ = self.manager.EndUIElement(id);
            }
        }
        self.app_draws = false;
        self.element.shown.set(false);
    }
}

impl Drop for GameCandidates {
    fn drop(&mut self) {
        self.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_follow_the_candidate_window() {
        let c = Candidates {
            items: (0..20).map(|i| format!("{i}")).collect(),
            notes: vec!["".into(), "나라 국".into()],
            selected: Some(10),
            page: Some((2, 3)),
            grid: false,
        };
        let list = ListData::from(&c);
        assert_eq!(list.page_starts(), [0, 9, 18]);
        assert_eq!(list.items[1], "1 나라 국", "뜻이 있으면 붙인다");
        assert_eq!(list.items[0], "0");
        assert_eq!(list.current_page(), 1, "10번은 두 번째 페이지");
        assert_eq!(
            ListData::from(&Candidates { grid: true, ..c }).page_starts(),
            [0],
            "격자는 한 페이지 30개"
        );
        assert_eq!(ListData::default().page_starts(), [0], "후보가 없어도 페이지 하나");
    }
}
