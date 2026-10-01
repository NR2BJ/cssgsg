//! 작업 표시줄 입력 표시기의 모드 아이콘(GUID_LBI_INPUTMODE): G(영어) / ㅊ(한국어) / 月(일본어).
//! 맥 메뉴 막대 아이콘과 같은 글자다. 작업 표시줄이 밝으면 검은 글자, 어두우면 흰 글자로 그린다.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;

use cssgsg_core::Mode;
use windows::Win32::Foundation::{COLORREF, E_POINTER, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS, CreateBitmap,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DIB_RGB_COLORS, DT_CENTER,
    DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, FF_DONTCARE, FW_BOLD,
    GdiFlush, HGDIOBJ, OUT_DEFAULT_PRECIS, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::Ole::{CONNECT_E_ADVISELIMIT, CONNECT_E_CANNOTCONNECT, CONNECT_E_NOCONNECTION};
use windows::Win32::UI::TextServices::{
    GUID_LBI_INPUTMODE, ITfLangBarItem_Impl, ITfLangBarItemButton, ITfLangBarItemButton_Impl,
    ITfLangBarItemSink, ITfMenu, ITfSource, ITfSource_Impl, TF_LANGBARITEMINFO, TF_LBI_ICON,
    TF_LBI_STYLE_BTN_BUTTON, TF_LBI_STYLE_SHOWNINTRAY, TF_LBI_TEXT, TF_LBI_TOOLTIP, TfLBIClick,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
};
use windows::core::{BOOL, BSTR, GUID, IUnknown, Interface, Ref, Result, implement, w};

use crate::guard::guarded;
use crate::{CLSID_TEXT_SERVICE, debug_log, menu};

pub fn label(mode: Mode) -> &'static str {
    match mode {
        Mode::En => "G",
        Mode::Ko => "ㅊ",
        Mode::Ja => "月",
    }
}

fn tooltip(mode: Mode) -> &'static str {
    match mode {
        Mode::En => "cssgsg: 영어(Graphite)",
        Mode::Ko => "cssgsg: 한국어(참신세벌식)",
        Mode::Ja => "cssgsg: 일본어(新月)",
    }
}

const SINK_COOKIE: u32 = 1;

#[implement(ITfLangBarItemButton, ITfSource, Agile = false)]
pub struct ModeButton {
    mode: Cell<Mode>,
    sink: RefCell<Option<ITfLangBarItemSink>>,
    /// 아이콘 메뉴에서 고른 것을 처리한다([`crate::menu`]).
    menu: Option<menu::Handler>,
}

impl ModeButton {
    pub fn new(mode: Mode, menu: Option<menu::Handler>) -> Self {
        Self { mode: Cell::new(mode), sink: RefCell::new(None), menu }
    }

    fn chosen(&self, command: Option<menu::Command>) {
        if let (Some(command), Some(handler)) = (command, &self.menu) {
            debug_log(&format!("mode icon menu: {command:?}"));
            handler(command);
        }
    }

    /// 모드가 바뀌면 작업 표시줄에 다시 그려 달라고 알린다.
    pub fn set_mode(&self, mode: Mode) {
        if self.mode.replace(mode) == mode {
            return;
        }
        if let Ok(sink) = self.sink.try_borrow()
            && let Some(sink) = sink.as_ref()
        {
            let _ = unsafe { sink.OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP) };
        }
    }
}

impl ITfLangBarItem_Impl for ModeButton_Impl {
    fn GetInfo(&self, info: *mut TF_LANGBARITEMINFO) -> Result<()> {
        if info.is_null() {
            return Err(E_POINTER.into());
        }
        let mut description = [0u16; 32];
        for (d, c) in description.iter_mut().zip("cssgsg 입력 모드".encode_utf16()) {
            *d = c;
        }
        let value = TF_LANGBARITEMINFO {
            clsidService: CLSID_TEXT_SERVICE,
            guidItem: GUID_LBI_INPUTMODE,
            dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY,
            ulSort: 0,
            szDescription: description,
        };
        unsafe { info.write(value) };
        Ok(())
    }

    fn GetStatus(&self) -> Result<u32> {
        Ok(0)
    }

    fn Show(&self, _show: BOOL) -> Result<()> {
        Ok(())
    }

    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(BSTR::from(tooltip(self.mode.get())))
    }
}

impl ITfLangBarItemButton_Impl for ModeButton_Impl {
    /// 작업 표시줄 아이콘을 눌렀다(왼쪽·오른쪽 모두): 맥 메뉴 막대처럼 메뉴를 띄운다.
    fn OnClick(&self, click: TfLBIClick, pt: &POINT, area: *const RECT) -> Result<()> {
        guarded(
            || Ok(()),
            || {
                debug_log(&format!("mode icon click {}", click.0));
                // SAFETY: TSF가 준 아이콘 사각형(없으면 NULL).
                let around = unsafe { area.as_ref() }.copied();
                let command = menu::show(*pt, around, self.mode.get());
                self.chosen(command);
                Ok(())
            },
        )
    }

    /// TSF가 메뉴를 대신 그리는 길(언어 막대): 같은 항목을 채운다.
    fn InitMenu(&self, menu: Ref<ITfMenu>) -> Result<()> {
        guarded(
            || Ok(()),
            || {
                debug_log("mode icon InitMenu");
                if let Ok(menu) = menu.ok() {
                    menu::fill(menu, self.mode.get());
                }
                Ok(())
            },
        )
    }

    fn OnMenuSelect(&self, id: u32) -> Result<()> {
        guarded(
            || Ok(()),
            || {
                self.chosen(menu::command(id));
                Ok(())
            },
        )
    }

    /// 부를 때마다 새 아이콘을 만든다(받은 쪽이 지운다). 작업 표시줄 색도 그때 본다.
    fn GetIcon(&self) -> Result<HICON> {
        guarded(
            || Err(windows::Win32::Foundation::E_UNEXPECTED.into()),
            || icon(self.mode.get(), light_taskbar()),
        )
    }

    fn GetText(&self) -> Result<BSTR> {
        Ok(BSTR::from(label(self.mode.get())))
    }
}

impl ITfSource_Impl for ModeButton_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<IUnknown>) -> Result<u32> {
        if riid.is_null() || unsafe { *riid } != ITfLangBarItemSink::IID {
            return Err(CONNECT_E_CANNOTCONNECT.into());
        }
        let mut slot = self.sink.try_borrow_mut().map_err(|_| windows::Win32::Foundation::E_UNEXPECTED)?;
        if slot.is_some() {
            return Err(CONNECT_E_ADVISELIMIT.into());
        }
        *slot = Some(punk.ok()?.cast()?);
        Ok(SINK_COOKIE)
    }

    fn UnadviseSink(&self, cookie: u32) -> Result<()> {
        let mut slot = self.sink.try_borrow_mut().map_err(|_| windows::Win32::Foundation::E_UNEXPECTED)?;
        if cookie != SINK_COOKIE || slot.take().is_none() {
            return Err(CONNECT_E_NOCONNECTION.into());
        }
        Ok(())
    }
}

/// 작업 표시줄(시스템) 색이 밝은지. 못 읽으면 윈도우 11 기본인 밝음.
pub fn light_taskbar() -> bool {
    windows_registry::CURRENT_USER
        .open("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
        .and_then(|k| k.get_u32("SystemUsesLightTheme"))
        .map(|v| v != 0)
        .unwrap_or(true)
}

/// 모드 글자 아이콘을 그린다(작은 아이콘 크기, 맑은 고딕 굵게). 검은 바탕에 흰 글자로 그려 밝기를 알파로 쓴다.
pub fn icon(mode: Mode, light: bool) -> Result<HICON> {
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    unsafe {
        let dc = CreateCompatibleDC(None);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let color = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
        let old_bitmap = SelectObject(dc, HGDIOBJ(color.0));
        let font = CreateFontW(
            -(size * 7 / 8),
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            FF_DONTCARE.0 as u32,
            w!("Malgun Gothic"),
        );
        let old_font = SelectObject(dc, HGDIOBJ(font.0));
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(0x00FF_FFFF));
        let mut rect = RECT { left: 0, top: 0, right: size, bottom: size };
        let mut text: Vec<u16> = label(mode).encode_utf16().collect();
        DrawTextW(dc, &mut text, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        let _ = GdiFlush();
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (size * size) as usize);
        let ink: u32 = if light { 0x00 } else { 0xFF };
        for p in pixels.iter_mut() {
            let a = (*p >> 16 & 0xFF).max(*p >> 8 & 0xFF).max(*p & 0xFF);
            let c = ink * a / 255;
            *p = (a << 24) | (c << 16) | (c << 8) | c;
        }
        SelectObject(dc, old_font);
        let _ = DeleteObject(HGDIOBJ(font.0));
        SelectObject(dc, old_bitmap);
        let _ = DeleteDC(dc);
        let mask = CreateBitmap(size, size, 1, 1, None);
        let made = CreateIconIndirect(&ICONINFO {
            fIcon: BOOL::from(true),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        });
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        made
    }
}

/// 시험용: 이 버튼을 COM 인터페이스로.
#[cfg(test)]
pub fn button(mode: Mode) -> (ITfLangBarItemButton, ITfSource) {
    let b: ITfLangBarItemButton = ModeButton::new(mode, None).into();
    let s: ITfSource = b.cast().unwrap();
    (b, s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::{BITMAP, GetObjectW};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo};

    /// 아이콘 색 비트맵에서 알파가 있는(글자) 픽셀 수.
    fn ink_pixels(icon: HICON) -> usize {
        unsafe {
            let mut info = ICONINFO::default();
            GetIconInfo(icon, &mut info).unwrap();
            let mut bm = BITMAP::default();
            GetObjectW(
                HGDIOBJ(info.hbmColor.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bm as *mut _ as *mut c_void),
            );
            let n = (bm.bmWidthBytes * bm.bmHeight) as usize;
            let mut buf = vec![0u8; n];
            let got = windows::Win32::Graphics::Gdi::GetBitmapBits(
                info.hbmColor,
                n as i32,
                buf.as_mut_ptr() as *mut c_void,
            );
            let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
            let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
            buf[..got as usize].chunks(4).filter(|px| px[3] > 0).count()
        }
    }

    #[test]
    fn draws_each_mode_letter_in_both_themes() {
        for mode in [Mode::En, Mode::Ko, Mode::Ja] {
            for light in [true, false] {
                let icon = icon(mode, light).unwrap();
                assert!(ink_pixels(icon) > 10, "{mode:?} light={light}: 글자가 그려지지 않았다");
                unsafe { DestroyIcon(icon).unwrap() };
            }
        }
    }

    #[test]
    fn sink_is_told_when_the_mode_changes() {
        let (b, source) = button(Mode::Ko);
        unsafe {
            assert_eq!(b.GetText().unwrap(), "ㅊ");
            let mut info = TF_LANGBARITEMINFO::default();
            b.GetInfo(&mut info).unwrap();
            assert_eq!((info.guidItem, info.clsidService), (GUID_LBI_INPUTMODE, CLSID_TEXT_SERVICE));
            // 다른 싱크 종류는 거절한다.
            assert!(source.AdviseSink(&ITfMenu::IID, &b.cast::<IUnknown>().unwrap()).is_err());
        }
    }
}
