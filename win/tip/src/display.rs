//! 조합 밑줄 표시 속성(ITfDisplayAttributeProvider가 넘기는 것).
//! 색은 앱 기본값을 쓰고 밑줄 모양만 정한다: 입력 중은 가는 밑줄, 변환 중 포커스된 문절은 굵은 밑줄.

use std::cell::Cell;

use windows::Win32::Foundation::{E_INVALIDARG, E_NOTIMPL, E_POINTER, S_FALSE};
use windows::Win32::UI::TextServices::{
    IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl, ITfDisplayAttributeInfo,
    ITfDisplayAttributeInfo_Impl, TF_ATTR_INPUT, TF_ATTR_TARGET_CONVERTED, TF_DA_ATTR_INFO, TF_DA_LINESTYLE,
    TF_DISPLAYATTRIBUTE, TF_LS_SOLID,
};
use windows::core::{BOOL, BSTR, GUID, Result, implement};

use crate::{GUID_DISPLAY_ATTRIBUTE_FOCUSED, GUID_DISPLAY_ATTRIBUTE_INPUT};

struct Kind {
    guid: GUID,
    description: &'static str,
    line: TF_DA_LINESTYLE,
    bold: bool,
    attr: TF_DA_ATTR_INFO,
}

const KINDS: [Kind; 2] = [
    Kind {
        guid: GUID_DISPLAY_ATTRIBUTE_INPUT,
        description: "cssgsg input",
        line: TF_LS_SOLID,
        bold: false,
        attr: TF_ATTR_INPUT,
    },
    Kind {
        guid: GUID_DISPLAY_ATTRIBUTE_FOCUSED,
        description: "cssgsg focused clause",
        line: TF_LS_SOLID,
        bold: true,
        attr: TF_ATTR_TARGET_CONVERTED,
    },
];

/// 표시 속성 하나.
#[implement(ITfDisplayAttributeInfo)]
pub struct DisplayAttribute(usize);

impl ITfDisplayAttributeInfo_Impl for DisplayAttribute_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(KINDS[self.0].guid)
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from(KINDS[self.0].description))
    }

    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        if pda.is_null() {
            return Err(E_POINTER.into());
        }
        let k = &KINDS[self.0];
        let value = TF_DISPLAYATTRIBUTE {
            lsStyle: k.line,
            fBoldLine: BOOL::from(k.bold),
            bAttr: k.attr,
            ..Default::default()
        };
        unsafe { pda.write(value) };
        Ok(())
    }

    fn SetAttributeInfo(&self, _pda: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn Reset(&self) -> Result<()> {
        Ok(())
    }
}

/// GUID에 맞는 표시 속성.
pub fn find(guid: &GUID) -> Result<ITfDisplayAttributeInfo> {
    match KINDS.iter().position(|k| k.guid == *guid) {
        Some(i) => Ok(DisplayAttribute(i).into()),
        None => Err(E_INVALIDARG.into()),
    }
}

/// 표시 속성 목록.
#[implement(IEnumTfDisplayAttributeInfo)]
pub struct EnumAttributes {
    at: Cell<usize>,
}

impl EnumAttributes {
    pub fn new() -> Self {
        Self { at: Cell::new(0) }
    }
}

impl IEnumTfDisplayAttributeInfo_Impl for EnumAttributes_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(EnumAttributes { at: Cell::new(self.at.get()) }.into())
    }

    /// 많아야 `count`개를 쓴다(그보다 많이 쓰면 호출자 메모리를 망친다, chewing #695). 모자라면 S_FALSE.
    fn Next(&self, count: u32, out: *mut Option<ITfDisplayAttributeInfo>, fetched: *mut u32) -> Result<()> {
        if out.is_null() {
            return Err(E_POINTER.into());
        }
        let mut n = 0;
        while (n as u32) < count && self.at.get() < KINDS.len() {
            unsafe { out.add(n).write(Some(DisplayAttribute(self.at.get()).into())) };
            self.at.set(self.at.get() + 1);
            n += 1;
        }
        if !fetched.is_null() {
            unsafe { fetched.write(n as u32) };
        }
        if n as u32 == count { Ok(()) } else { Err(S_FALSE.into()) }
    }

    fn Reset(&self) -> Result<()> {
        self.at.set(0);
        Ok(())
    }

    fn Skip(&self, count: u32) -> Result<()> {
        let to = self.at.get() + count as usize;
        self.at.set(to.min(KINDS.len()));
        if to <= KINDS.len() { Ok(()) } else { Err(S_FALSE.into()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_both_attributes_without_overrunning() {
        let list: IEnumTfDisplayAttributeInfo = EnumAttributes::new().into();
        let mut slots: [Option<ITfDisplayAttributeInfo>; 3] = [None, None, None];
        let mut fetched = 0;
        // 3개를 달라고 해도 2개만 쓴다(S_FALSE는 성공 코드라 Result로는 Ok, 개수로 본다).
        unsafe { list.Next(&mut slots[..], &mut fetched).unwrap() };
        assert_eq!(fetched, 2);
        assert!(slots[2].is_none());
        let guids: Vec<GUID> =
            slots[..2].iter().map(|s| unsafe { s.as_ref().unwrap().GetGUID().unwrap() }).collect();
        assert_eq!(guids, vec![GUID_DISPLAY_ATTRIBUTE_INPUT, GUID_DISPLAY_ATTRIBUTE_FOCUSED]);
        unsafe { list.Next(&mut slots[..1], &mut fetched).unwrap() };
        assert_eq!(fetched, 0, "끝까지 읽은 뒤에는 0개");
        unsafe { list.Reset().unwrap() };
        let mut one: [Option<ITfDisplayAttributeInfo>; 1] = [None];
        unsafe { list.Next(&mut one, &mut fetched).unwrap() };
        assert_eq!(fetched, 1);
    }

    #[test]
    fn focused_clause_is_a_bold_converted_underline() {
        let info = find(&GUID_DISPLAY_ATTRIBUTE_FOCUSED).unwrap();
        let mut da = TF_DISPLAYATTRIBUTE::default();
        unsafe { info.GetAttributeInfo(&mut da).unwrap() };
        assert_eq!(
            (da.lsStyle, da.fBoldLine.as_bool(), da.bAttr),
            (TF_LS_SOLID, true, TF_ATTR_TARGET_CONVERTED)
        );
        assert!(find(&GUID::from_u128(7)).is_err());
    }
}
