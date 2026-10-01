//! 편집 세션: 앱 문서를 바꾸는 일은 TSF가 허락한 세션(편집 쿠키 `ec`) 안에서만 한다.
//!
//! 조합은 TSF 조합(ITfComposition)이다. 지금 조합은 텍스트 서비스와 편집 세션이 같이 쥐는 칸([`Slot`])에 둔다:
//! 비동기로 늦게 도는 세션도 그때의 조합을 이어서 쓴다.

use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::rc::Rc;

use windows::Win32::Foundation::E_UNEXPECTED;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::TextServices::{
    GUID_PROP_ATTRIBUTE, INSERT_TEXT_AT_SELECTION_FLAGS, ITfComposition, ITfCompositionSink, ITfContext,
    ITfContextComposition, ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection, ITfRange, TF_AE_END,
    TF_ANCHOR_END, TF_ANCHOR_START, TF_IAS_QUERYONLY, TF_SELECTION, TF_SELECTIONSTYLE,
};
use windows::core::{BOOL, Interface, Result, implement};

use crate::guard::guarded;
use crate::plan::{Op, Segment};

/// 지금 조합(없으면 None)과 그 조합이 있는 문맥.
pub type Slot = Rc<RefCell<Option<Composing>>>;

#[derive(Clone)]
pub struct Composing {
    pub composition: ITfComposition,
    pub context: ITfContext,
}

/// 조합 밑줄 표시 속성의 아톰(ITfCategoryMgr::RegisterGUID). 0이면 표시 속성을 달지 않는다.
#[derive(Clone, Copy, Default)]
pub struct Attrs {
    pub input: i32,
    pub focused: i32,
}

/// [`Op`] 목록을 차례로 문서에 적용한다.
#[implement(ITfEditSession)]
pub struct ApplyOps {
    context: ITfContext,
    ops: Vec<Op>,
    slot: Slot,
    sink: ITfCompositionSink,
    attrs: Attrs,
    /// 세션이 돌았으면 true(RequestEditSession 안에서 바로 돌았는지 보는 데 쓴다).
    ran: Rc<Cell<bool>>,
}

impl ApplyOps {
    pub fn new(
        context: ITfContext,
        ops: Vec<Op>,
        slot: Slot,
        sink: ITfCompositionSink,
        attrs: Attrs,
    ) -> Self {
        Self { context, ops, slot, sink, attrs, ran: Rc::default() }
    }

    pub fn ran(&self) -> Rc<Cell<bool>> {
        self.ran.clone()
    }
}

impl ITfEditSession_Impl for ApplyOps_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        self.ran.set(true);
        guarded(
            || Err(E_UNEXPECTED.into()),
            || {
                for op in &self.ops {
                    self.apply(ec, op)?;
                }
                Ok(())
            },
        )
    }
}

impl ApplyOps {
    fn apply(&self, ec: u32, op: &Op) -> Result<()> {
        match op {
            Op::Commit(text) => match self.take() {
                Some(c) => unsafe {
                    let range = c.composition.GetRange()?;
                    range.SetText(ec, 0, text)?;
                    finish(ec, &c, &range)
                },
                None => unsafe {
                    let insert: ITfInsertAtSelection = self.context.cast()?;
                    let range = insert.InsertTextAtSelection(ec, INSERT_TEXT_AT_SELECTION_FLAGS(0), text)?;
                    caret_at_end(ec, &self.context, &range)
                },
            },
            Op::Compose { text, segments } => unsafe {
                let composing = match self.current() {
                    Some(c) => c,
                    None => self.start(ec)?,
                };
                let range = composing.composition.GetRange()?;
                range.SetText(ec, 0, text)?;
                self.mark(ec, &composing.context, &range, segments)?;
                caret_at_end(ec, &composing.context, &range)
            },
            Op::Clear => match self.take() {
                Some(c) => unsafe {
                    let range = c.composition.GetRange()?;
                    range.SetText(ec, 0, &[])?;
                    finish(ec, &c, &range)
                },
                None => Ok(()),
            },
        }
    }

    fn current(&self) -> Option<Composing> {
        self.slot.try_borrow().ok()?.clone()
    }

    fn take(&self) -> Option<Composing> {
        self.slot.try_borrow_mut().ok()?.take()
    }

    /// 커서 자리에서 조합을 시작한다.
    unsafe fn start(&self, ec: u32) -> Result<Composing> {
        unsafe {
            let insert: ITfInsertAtSelection = self.context.cast()?;
            let range = insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?;
            let compositions: ITfContextComposition = self.context.cast()?;
            let composition = compositions.StartComposition(ec, &range, &self.sink)?;
            let composing = Composing { composition, context: self.context.clone() };
            if let Ok(mut slot) = self.slot.try_borrow_mut() {
                *slot = Some(composing.clone());
            }
            Ok(composing)
        }
    }

    /// 조합 전체에 입력 밑줄, 포커스된 문절에 굵은 밑줄.
    unsafe fn mark(
        &self,
        ec: u32,
        context: &ITfContext,
        range: &ITfRange,
        segments: &[Segment],
    ) -> Result<()> {
        if self.attrs.input == 0 {
            return Ok(());
        }
        unsafe {
            let property = context.GetProperty(&GUID_PROP_ATTRIBUTE)?;
            property.SetValue(ec, range, &VARIANT::from(self.attrs.input))?;
            if self.attrs.focused != 0 {
                for s in segments.iter().filter(|s| s.focused && s.len > 0) {
                    let part = range.Clone()?;
                    part.Collapse(ec, TF_ANCHOR_START)?;
                    let mut moved = 0;
                    part.ShiftEnd(ec, (s.start + s.len) as i32, &mut moved, std::ptr::null())?;
                    part.ShiftStart(ec, s.start as i32, &mut moved, std::ptr::null())?;
                    property.SetValue(ec, &part, &VARIANT::from(self.attrs.focused))?;
                }
            }
        }
        Ok(())
    }
}

/// 조합을 끝낸다(글자는 그대로 남아 확정된다). 밑줄을 지우고 커서를 끝으로.
unsafe fn finish(ec: u32, c: &Composing, range: &ITfRange) -> Result<()> {
    unsafe {
        if let Ok(property) = c.context.GetProperty(&GUID_PROP_ATTRIBUTE) {
            let _ = property.Clear(ec, range);
        }
        caret_at_end(ec, &c.context, range)?;
        c.composition.EndComposition(ec)
    }
}

unsafe fn caret_at_end(ec: u32, context: &ITfContext, range: &ITfRange) -> Result<()> {
    unsafe {
        let caret = range.Clone()?;
        caret.Collapse(ec, TF_ANCHOR_END)?;
        let selection = TF_SELECTION {
            range: ManuallyDrop::new(Some(caret)),
            style: TF_SELECTIONSTYLE { ase: TF_AE_END, fInterimChar: BOOL::from(false) },
        };
        let result = context.SetSelection(ec, std::slice::from_ref(&selection));
        drop(ManuallyDrop::into_inner(selection.range));
        result
    }
}

/// 조합을 그 자리에서 끝낸다(포커스가 옮겨 갈 때, 입력기를 끌 때). 글자는 확정된 채로 남는다.
#[implement(ITfEditSession)]
pub struct EndComposition {
    composing: Composing,
}

impl EndComposition {
    pub fn new(composing: Composing) -> Self {
        Self { composing }
    }
}

impl ITfEditSession_Impl for EndComposition_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        guarded(
            || Err(E_UNEXPECTED.into()),
            || unsafe {
                let range = self.composing.composition.GetRange()?;
                finish(ec, &self.composing, &range)
            },
        )
    }
}
