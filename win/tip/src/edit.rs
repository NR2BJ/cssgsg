//! 편집 세션: 앱 문서를 바꾸는 일은 TSF가 허락한 세션(편집 쿠키 `ec`) 안에서만 한다.
//!
//! 조합은 TSF 조합(ITfComposition)이다. 지금 조합은 텍스트 서비스와 편집 세션이 같이 쥐는 칸([`Slot`])에 둔다:
//! 비동기로 늦게 도는 세션도 그때의 조합을 이어서 쓴다. 세션 끝에서 후보창·HUD를 붙일 자리를 재서 [`UiHook`]에 준다.

use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::rc::Rc;

use windows::Win32::Foundation::{E_UNEXPECTED, RECT};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::TextServices::{
    GUID_PROP_ATTRIBUTE, GUID_PROP_INPUTSCOPE, INSERT_TEXT_AT_SELECTION_FLAGS, IS_NUMERIC_PASSWORD,
    IS_PASSWORD, ITfComposition, ITfCompositionSink, ITfContext, ITfContextComposition, ITfEditSession,
    ITfEditSession_Impl, ITfInputScope, ITfInsertAtSelection, ITfRange, InputScope, TF_AE_END, TF_ANCHOR_END,
    TF_ANCHOR_START, TF_DEFAULT_SELECTION, TF_IAS_QUERYONLY, TF_SELECTION, TF_SELECTIONSTYLE,
};
use windows::core::{BOOL, IUnknown, Interface, Result, implement};

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

/// 문서를 고친 뒤(또는 읽기만 하는 세션 끝에) 화면을 맞추라고 부른다. 인자는 후보창·HUD를 붙일 사각형(화면 좌표):
/// 조합이 있으면 포커스된 문절(없으면 조합 전체), 없으면 커서. 모르면 None.
pub type UiHook = Rc<dyn Fn(Option<RECT>)>;

/// [`Op`] 목록을 차례로 문서에 적용한다.
#[implement(ITfEditSession)]
pub struct ApplyOps {
    context: ITfContext,
    ops: Vec<Op>,
    slot: Slot,
    sink: ITfCompositionSink,
    attrs: Attrs,
    hook: Option<UiHook>,
    /// 마지막 조합의 포커스된 문절(UTF-16 시작, 길이). 후보창을 그 아래에 붙인다.
    focus: Cell<Option<(usize, usize)>>,
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
        hook: Option<UiHook>,
    ) -> Self {
        Self { context, ops, slot, sink, attrs, hook, focus: Cell::new(None), ran: Rc::default() }
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
                if let Some(hook) = &self.hook {
                    hook(unsafe { anchor(ec, &self.context, &self.slot, self.focus.get()) });
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
                self.focus.set(segments.iter().find(|s| s.focused).map(|s| (s.start, s.len)));
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
                    let part = sub_range(ec, range, s.start, s.len)?;
                    property.SetValue(ec, &part, &VARIANT::from(self.attrs.focused))?;
                }
            }
        }
        Ok(())
    }
}

/// `range` 안의 [start, start+len) 부분(UTF-16 단위).
unsafe fn sub_range(ec: u32, range: &ITfRange, start: usize, len: usize) -> Result<ITfRange> {
    unsafe {
        let part = range.Clone()?;
        part.Collapse(ec, TF_ANCHOR_START)?;
        let mut moved = 0;
        part.ShiftEnd(ec, (start + len) as i32, &mut moved, std::ptr::null())?;
        part.ShiftStart(ec, start as i32, &mut moved, std::ptr::null())?;
        Ok(part)
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

/// 후보창·HUD를 붙일 사각형을 잰다(편집 쿠키가 있어야 한다): 조합이 있으면 포커스된 문절(없으면 조합 전체),
/// 없으면 커서(선택 영역).
unsafe fn anchor(ec: u32, context: &ITfContext, slot: &Slot, focus: Option<(usize, usize)>) -> Option<RECT> {
    unsafe {
        let composing = slot.try_borrow().ok().and_then(|s| s.clone());
        let (context, range) = match composing {
            Some(c) => {
                let whole = c.composition.GetRange().ok()?;
                let range = match focus {
                    Some((start, len)) if len > 0 => sub_range(ec, &whole, start, len).ok()?,
                    _ => whole,
                };
                (c.context, range)
            }
            None => {
                let mut selection = [TF_SELECTION::default()];
                let mut fetched = 0;
                context.GetSelection(ec, TF_DEFAULT_SELECTION, &mut selection, &mut fetched).ok()?;
                if fetched == 0 {
                    return None;
                }
                let range = ManuallyDrop::into_inner(std::mem::take(&mut selection[0].range))?;
                (context.clone(), range)
            }
        };
        let view = context.GetActiveView().ok()?;
        let mut rect = RECT::default();
        let mut clipped = BOOL::default();
        view.GetTextExt(ec, &range, &mut rect, &mut clipped).ok()?;
        (rect != RECT::default()).then_some(rect)
    }
}

/// 문서는 고치지 않고 후보창·HUD 자리만 잰다(모드만 바뀌었을 때, 후보 페이지만 바뀌었을 때).
#[implement(ITfEditSession)]
pub struct Measure {
    context: ITfContext,
    slot: Slot,
    hook: UiHook,
}

impl Measure {
    pub fn new(context: ITfContext, slot: Slot, hook: UiHook) -> Self {
        Self { context, slot, hook }
    }
}

impl ITfEditSession_Impl for Measure_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        guarded(
            || Err(E_UNEXPECTED.into()),
            || {
                (self.hook)(unsafe { anchor(ec, &self.context, &self.slot, None) });
                Ok(())
            },
        )
    }
}

/// 입력 범위를 읽은 결과. 세션이 돌았는지(앱 문서를 읽을 수 있는지)도 입력칸을 가리는 데 쓰고, 개발자 기록에도 남긴다.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ScopeRead {
    /// 세션이 돌지 않았다(텍스트 저장소가 없는 문맥, 거절).
    #[default]
    NotRun,
    /// 선택 영역이 없다.
    NoSelection,
    /// 앱이 입력 범위를 주지 않았다(빈 값). Chromium은 위치별 속성을 주지 않아 흔히 이렇다.
    Empty,
    /// 앱이 준 입력 범위(InputScope 값).
    Scopes(Vec<i32>),
    /// 이 단계에서 실패했다.
    Failed(&'static str),
}

impl ScopeRead {
    /// 앱 문서의 선택 영역까지 읽혔는지(텍스트 저장소가 있는 진짜 입력칸). Firefox가 입력칸 아닌 곳에 두는 문맥은
    /// 세션은 돌지만 선택 영역을 주지 않는다(개발자 기록, 2026-10-02).
    pub fn readable(&self) -> bool {
        !matches!(self, Self::NotRun | Self::Failed("selection"))
    }

    /// 입력 범위가 비밀번호 칸이다(IS_PASSWORD·IS_NUMERIC_PASSWORD, Firefox 등).
    pub fn is_password(&self) -> bool {
        matches!(self, Self::Scopes(s) if s.iter().any(|&s| s == IS_PASSWORD.0 || s == IS_NUMERIC_PASSWORD.0))
    }
}

/// 입력칸의 입력 범위를 읽는다(동기 읽기 세션).
#[implement(ITfEditSession)]
pub struct ReadInputScope {
    context: ITfContext,
    out: Rc<RefCell<ScopeRead>>,
}

impl ReadInputScope {
    pub fn new(context: ITfContext) -> Self {
        Self { context, out: Rc::default() }
    }

    pub fn result(&self) -> Rc<RefCell<ScopeRead>> {
        self.out.clone()
    }

    unsafe fn read(&self, ec: u32) -> ScopeRead {
        let failed = |stage| move |_| ScopeRead::Failed(stage);
        let run = || -> std::result::Result<ScopeRead, ScopeRead> {
            unsafe {
                let mut selection = [TF_SELECTION::default()];
                let mut fetched = 0;
                self.context
                    .GetSelection(ec, TF_DEFAULT_SELECTION, &mut selection, &mut fetched)
                    .map_err(failed("selection"))?;
                let range = ManuallyDrop::into_inner(std::mem::take(&mut selection[0].range));
                let Some(range) = range.filter(|_| fetched > 0) else { return Ok(ScopeRead::NoSelection) };
                let property =
                    self.context.GetAppProperty(&GUID_PROP_INPUTSCOPE).map_err(failed("property"))?;
                let value = property.GetValue(ec, &range).map_err(failed("value"))?;
                let Ok(unknown) = IUnknown::try_from(&value) else { return Ok(ScopeRead::Empty) };
                let scope: ITfInputScope = unknown.cast().map_err(failed("interface"))?;
                let mut scopes: *mut InputScope = std::ptr::null_mut();
                let mut count = 0u32;
                scope.GetInputScopes(&mut scopes, &mut count).map_err(failed("scopes"))?;
                if scopes.is_null() {
                    return Ok(ScopeRead::Scopes(Vec::new()));
                }
                let list = std::slice::from_raw_parts(scopes, count as usize).iter().map(|s| s.0).collect();
                CoTaskMemFree(Some(scopes as *const std::ffi::c_void));
                Ok(ScopeRead::Scopes(list))
            }
        };
        run().unwrap_or_else(|e| e)
    }
}

impl ITfEditSession_Impl for ReadInputScope_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        guarded(
            || Err(E_UNEXPECTED.into()),
            || {
                *self.out.borrow_mut() = unsafe { self.read(ec) };
                Ok(())
            },
        )
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
