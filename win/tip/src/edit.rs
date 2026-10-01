//! 편집 세션: 앱 문서를 바꾸는 일은 TSF가 허락한 세션(편집 쿠키 `ec`) 안에서만 한다.

use std::mem::ManuallyDrop;

use windows::Win32::Foundation::E_UNEXPECTED;
use windows::Win32::UI::TextServices::{
    INSERT_TEXT_AT_SELECTION_FLAGS, ITfContext, ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection,
    TF_AE_END, TF_ANCHOR_END, TF_SELECTION, TF_SELECTIONSTYLE,
};
use windows::core::{BOOL, Interface, Result, implement};

use crate::guard::guarded;

/// 선택 영역(커서) 자리에 글자를 확정해 넣고 커서를 그 뒤로 옮긴다.
#[implement(ITfEditSession)]
pub struct InsertText {
    context: ITfContext,
    text: Vec<u16>,
}

impl InsertText {
    pub fn new(context: ITfContext, text: &str) -> Self {
        Self { context, text: text.encode_utf16().collect() }
    }
}

impl ITfEditSession_Impl for InsertText_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        guarded(
            || Err(E_UNEXPECTED.into()),
            || unsafe {
                let insert: ITfInsertAtSelection = self.context.cast()?;
                let range =
                    insert.InsertTextAtSelection(ec, INSERT_TEXT_AT_SELECTION_FLAGS(0), &self.text)?;
                range.Collapse(ec, TF_ANCHOR_END)?;
                let selection = TF_SELECTION {
                    range: ManuallyDrop::new(Some(range)),
                    style: TF_SELECTIONSTYLE { ase: TF_AE_END, fInterimChar: BOOL::from(false) },
                };
                let result = self.context.SetSelection(ec, std::slice::from_ref(&selection));
                drop(ManuallyDrop::into_inner(selection.range));
                result
            },
        )
    }
}
