//! 입력기 화면: 후보창과 모드 HUD. 앱 프로세스 안(입력기를 쓰는 스레드)에서 GDI로 그린다.
//! 맥 후보창(NRIME CandidatePanel)과 같은 모양이다: 목록 9개("1." 번호, 후보 뒤에 작고 흐린 뜻, 고른 줄은 강조색),
//! 격자 5×6(고른 후보의 뜻은 아래 줄), 페이지가 여럿이면 "n/N". 모드 HUD는 모드를 바꿀 때 커서 위에 1초 보이고
//! 0.3초 동안 사라진다. 둥근 모서리·그림자는 윈도우 11 DWM에 맡기고, 밝은·어두운 앱 테마를 따른다.
//!
//! 창은 포커스를 가져가지 않는다(WS_EX_NOACTIVATE, 클릭에도 MA_NOACTIVATE). 창 상태는 GWLP_USERDATA에 둔
//! Rc가 쥐고, WM_NCDESTROY에서 놓는다.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Once;

use cssgsg_core::Mode;
use cssgsg_core::config::{HudPosition, WindowsConfig};
use cssgsg_core::engine::{CAND_GRID_COLUMNS, CAND_GRID_PAGE, CAND_LIST_PAGE, Candidates};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap, CreateCompatibleDC,
    CreateFontW, CreateSolidBrush, DEFAULT_CHARSET, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX,
    DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, EndPaint, FF_DONTCARE, FW_BOLD,
    FW_NORMAL, FillRect, GetDC, GetMonitorInfoW, GetTextExtentPoint32W, HDC, HFONT, HGDIOBJ, InvalidateRect,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT, ReleaseDC,
    SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DROPSHADOW, CS_IME, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetCursorPos,
    GetWindowLongPtrW, HWND_TOPMOST, IDC_ARROW, IsWindow, KillTimer, LWA_ALPHA, LoadCursorW, MA_NOACTIVATE,
    RegisterClassExW, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetLayeredWindowAttributes, SetTimer,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_MOUSEACTIVATE, WM_NCDESTROY, WM_PAINT, WM_TIMER,
    WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::module;

// ---- 색과 글꼴 -----------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Theme {
    background: COLORREF,
    text: COLORREF,
    dim: COLORREF,
    accent: COLORREF,
    on_accent: COLORREF,
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}

/// 앱 테마(AppsUseLightTheme)와 강조색(DWM AccentColor, ABGR)을 읽는다. 못 읽으면 밝은 테마, 윈도우 기본 파랑.
fn theme() -> Theme {
    let personalize = windows_registry::CURRENT_USER
        .open("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let light = personalize.and_then(|k| k.get_u32("AppsUseLightTheme")).map(|v| v != 0).unwrap_or(true);
    let accent = windows_registry::CURRENT_USER
        .open("Software\\Microsoft\\Windows\\DWM")
        .and_then(|k| k.get_u32("AccentColor"))
        .map(|abgr| COLORREF(abgr & 0x00FF_FFFF))
        .unwrap_or(rgb(0x00, 0x78, 0xD4));
    if light {
        Theme {
            background: rgb(0xF9, 0xF9, 0xF9),
            text: rgb(0x1A, 0x1A, 0x1A),
            dim: rgb(0x6E, 0x6E, 0x6E),
            accent,
            on_accent: rgb(0xFF, 0xFF, 0xFF),
        }
    } else {
        Theme {
            background: rgb(0x2C, 0x2C, 0x2C),
            text: rgb(0xFF, 0xFF, 0xFF),
            dim: rgb(0xA8, 0xA8, 0xA8),
            accent,
            on_accent: rgb(0xFF, 0xFF, 0xFF),
        }
    }
}

/// 이 창의 DPI에 맞춘 픽셀(설계 값은 96 DPI 기준).
fn px(value: i32, dpi: u32) -> i32 {
    (value as i64 * dpi as i64 / 96) as i32
}

struct Font(HFONT);

impl Font {
    fn new(height_px: i32, bold: bool, face: PCWSTR) -> Self {
        Font(unsafe {
            CreateFontW(
                -height_px,
                0,
                0,
                0,
                if bold { FW_BOLD.0 as i32 } else { FW_NORMAL.0 as i32 },
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                FF_DONTCARE.0 as u32,
                face,
            )
        })
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        let _ = unsafe { DeleteObject(HGDIOBJ(self.0.0)) };
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn text_width(dc: HDC, font: &Font, s: &str) -> i32 {
    if s.is_empty() {
        return 0;
    }
    let mut size = SIZE::default();
    unsafe {
        let old = SelectObject(dc, HGDIOBJ(font.0.0));
        let _ = GetTextExtentPoint32W(dc, &wide(s), &mut size);
        SelectObject(dc, old);
    }
    size.cx
}

fn draw(
    dc: HDC,
    font: &Font,
    color: COLORREF,
    s: &str,
    mut rect: RECT,
    flags: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    if s.is_empty() {
        return;
    }
    unsafe {
        let old = SelectObject(dc, HGDIOBJ(font.0.0));
        SetTextColor(dc, color);
        let mut text = wide(s);
        DrawTextW(dc, &mut text, &mut rect, flags | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
        SelectObject(dc, old);
    }
}

fn fill(dc: HDC, rect: RECT, color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(dc, &rect, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

/// 화면 안에 들어가는 자리: 기준 사각형 아래(모자라면 위), 왼쪽 맞춤, 작업 영역 안으로.
fn place(anchor: RECT, width: i32, height: i32, gap: i32, prefer_above: bool) -> (i32, i32) {
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    let work = unsafe {
        let monitor = MonitorFromRect(&anchor, MONITOR_DEFAULTTONEAREST);
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            info.rcWork
        } else {
            RECT { left: 0, top: 0, right: 1920, bottom: 1080 }
        }
    };
    let below = anchor.bottom + gap;
    let above = anchor.top - gap - height;
    let y = if prefer_above {
        if above >= work.top { above } else { below }
    } else if below + height <= work.bottom || above < work.top {
        below.min(work.bottom - height)
    } else {
        above
    };
    let x = anchor.left.clamp(work.left, (work.right - width).max(work.left));
    (x, y.max(work.top))
}

// ---- 창 바탕 ---------------------------------------------------------------------------------------

/// 창 하나의 그리기·타이머 동작.
trait Surface {
    fn paint(&self, hwnd: HWND, dc: HDC, width: i32, height: i32);
    fn timer(&self, _hwnd: HWND, _id: usize) {}
}

const CLASS: PCWSTR = w!("cssgsg.Popup");

fn register_class() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DROPSHADOW | CS_IME,
            lpfnWndProc: Some(window_proc),
            hInstance: module().into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassExW(&class);
    });
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    crate::guard::guarded(
        || LRESULT(0),
        || unsafe {
            let surface = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Rc<dyn Surface>;
            match msg {
                WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
                WM_PAINT if !surface.is_null() => {
                    let mut ps = PAINTSTRUCT::default();
                    let dc = BeginPaint(hwnd, &mut ps);
                    let mut rect = RECT::default();
                    let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
                    let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
                    // 한 번에 옮겨 그려 깜빡이지 않게.
                    let memory = CreateCompatibleDC(Some(dc));
                    let bitmap = CreateCompatibleBitmap(dc, w.max(1), h.max(1));
                    let old = SelectObject(memory, HGDIOBJ(bitmap.0));
                    SetBkMode(memory, TRANSPARENT);
                    (*surface).paint(hwnd, memory, w, h);
                    let _ = BitBlt(dc, 0, 0, w, h, Some(memory), 0, 0, SRCCOPY);
                    SelectObject(memory, old);
                    let _ = DeleteObject(HGDIOBJ(bitmap.0));
                    let _ = DeleteDC(memory);
                    let _ = EndPaint(hwnd, &ps);
                    LRESULT(0)
                }
                WM_TIMER if !surface.is_null() => {
                    (*surface).timer(hwnd, wparam.0);
                    LRESULT(0)
                }
                WM_NCDESTROY => {
                    if !surface.is_null() {
                        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                        drop(Box::from_raw(surface as *mut Rc<dyn Surface>));
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        },
    )
}

/// 포커스를 가져가지 않는 맨 위 팝업. 닫을 때(Drop) 창을 없앤다.
struct Popup {
    hwnd: HWND,
}

impl Popup {
    fn new(surface: Rc<dyn Surface>, layered: bool) -> Option<Self> {
        register_class();
        let mut ex = WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE;
        if layered {
            ex |= WS_EX_LAYERED;
        }
        let hwnd = unsafe {
            CreateWindowExW(ex, CLASS, w!(""), WS_POPUP, 0, 0, 1, 1, None, None, Some(module().into()), None)
                .ok()?
        };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(Box::new(surface)) as isize);
            let corner = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner as *const _ as *const _,
                std::mem::size_of_val(&corner) as u32,
            );
            if layered {
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
            }
        }
        Some(Popup { hwnd })
    }

    fn dpi(&self) -> u32 {
        match unsafe { GetDpiForWindow(self.hwnd) } {
            0 => 96,
            d => d,
        }
    }

    fn show_at(&self, x: i32, y: i32, width: i32, height: i32) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// 글자 폭을 재는 DC(창 DPI의 글꼴로 잰다).
    fn measure<T>(&self, f: impl FnOnce(HDC) -> T) -> T {
        unsafe {
            let dc = GetDC(Some(self.hwnd));
            let r = f(dc);
            ReleaseDC(Some(self.hwnd), dc);
            r
        }
    }
}

impl Drop for Popup {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(Some(self.hwnd)).as_bool() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

// ---- 후보창 --------------------------------------------------------------------------------------

struct CandidateView {
    candidates: Candidates,
    japanese: bool,
    /// 후보 글자 크기(96 DPI 픽셀, 설정 `[windows] candidate_font_size`).
    font_size: i32,
    theme: Theme,
    dpi: u32,
    /// 그릴 때 쓰는 값(show에서 잰다).
    layout: Layout,
}

#[derive(Default, Clone, Copy)]
struct Layout {
    width: i32,
    height: i32,
    row: i32,
    number: i32,
    cell: i32,
    footer: i32,
}

struct CandidateFonts {
    text: Font,
    small: Font,
    number: Font,
}

impl CandidateView {
    fn fonts(&self) -> CandidateFonts {
        let face = if self.japanese { w!("Yu Gothic UI") } else { w!("Malgun Gothic") };
        let size = px(self.font_size, self.dpi);
        let grid = self.candidates.grid;
        CandidateFonts {
            text: Font::new(if grid { size - px(1, self.dpi) } else { size }, false, face),
            small: Font::new((size - px(3, self.dpi)).max(px(9, self.dpi)), false, face),
            number: Font::new((size - px(2, self.dpi)).max(px(9, self.dpi)), false, w!("Segoe UI")),
        }
    }

    fn page_range(&self) -> (usize, usize) {
        let size = if self.candidates.grid { CAND_GRID_PAGE } else { CAND_LIST_PAGE };
        let selected = self.candidates.selected.unwrap_or(0);
        let start = selected / size * size;
        (start, (start + size).min(self.candidates.items.len()))
    }

    fn footer_text(&self) -> String {
        let pages = match self.candidates.page {
            Some((page, total)) if total > 1 => format!("{page}/{total}"),
            _ => String::new(),
        };
        if self.candidates.grid {
            let note = self
                .candidates
                .selected
                .and_then(|i| self.candidates.notes.get(i))
                .cloned()
                .unwrap_or_default();
            [note, pages].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("   ")
        } else {
            pages
        }
    }

    fn measure(&mut self, dc: HDC) {
        let f = self.fonts();
        let d = self.dpi;
        let pad = px(4, d);
        let (start, end) = self.page_range();
        let c = &self.candidates;
        let footer = if self.footer_text().is_empty() { 0 } else { px(18, d) };
        if c.grid {
            let mut cell = px(60, d);
            for item in &c.items[start..end] {
                cell = cell.max(text_width(dc, &f.text, item) + px(16, d));
            }
            let rows = (end - start).div_ceil(CAND_GRID_COLUMNS) as i32;
            let row = px(26, d).max(px(self.font_size, d) * 185 / 100);
            self.layout = Layout {
                width: cell * CAND_GRID_COLUMNS as i32 + pad * 2,
                height: pad * 2 + rows * row + footer,
                row,
                number: 0,
                cell,
                footer,
            };
        } else {
            let number = px(22, d).max(px(self.font_size - 2, d) * 18 / 10);
            let mut width = px(160, d);
            for i in start..end {
                let mut w = text_width(dc, &f.text, &c.items[i]);
                if let Some(note) = c.notes.get(i).filter(|n| !n.is_empty()) {
                    w += text_width(dc, &f.small, &format!("  {note}"));
                }
                width = width.max(px(6, d) + number + px(2, d) + w + px(12, d));
            }
            let row = px(24, d).max(px(self.font_size, d) * 17 / 10);
            self.layout = Layout {
                width,
                height: pad * 2 + (end - start) as i32 * row + footer,
                row,
                number,
                cell: 0,
                footer,
            };
        }
    }
}

impl Surface for RefCell<Option<CandidateView>> {
    fn paint(&self, _hwnd: HWND, dc: HDC, width: i32, height: i32) {
        let Ok(view) = self.try_borrow() else { return };
        let Some(v) = view.as_ref() else { return };
        let t = v.theme;
        let d = v.dpi;
        let l = v.layout;
        let f = v.fonts();
        let pad = px(4, d);
        fill(dc, RECT { left: 0, top: 0, right: width, bottom: height }, t.background);
        let (start, end) = v.page_range();
        let c = &v.candidates;
        for i in start..end {
            let k = (i - start) as i32;
            let selected = c.selected == Some(i);
            let cell = if c.grid {
                let col = k % CAND_GRID_COLUMNS as i32;
                let row = k / CAND_GRID_COLUMNS as i32;
                RECT {
                    left: pad + col * l.cell,
                    top: pad + row * l.row,
                    right: pad + (col + 1) * l.cell,
                    bottom: pad + (row + 1) * l.row,
                }
            } else {
                RECT { left: 0, top: pad + k * l.row, right: width, bottom: pad + (k + 1) * l.row }
            };
            if selected {
                let inset = px(2, d);
                fill(
                    dc,
                    RECT {
                        left: cell.left + inset,
                        top: cell.top + 1,
                        right: cell.right - inset,
                        bottom: cell.bottom - 1,
                    },
                    t.accent,
                );
            }
            let ink = if selected { t.on_accent } else { t.text };
            let soft = if selected { t.on_accent } else { t.dim };
            if c.grid {
                draw(dc, &f.text, ink, &c.items[i], cell, DT_CENTER | DT_END_ELLIPSIS);
            } else {
                let n = (k + 1) % 10;
                let number = RECT { left: px(6, d), right: px(6, d) + l.number, ..cell };
                draw(dc, &f.number, soft, &format!("{n}."), number, DT_LEFT);
                let text_left = number.right + px(2, d);
                let item = &c.items[i];
                draw(
                    dc,
                    &f.text,
                    ink,
                    item,
                    RECT { left: text_left, right: cell.right - px(6, d), ..cell },
                    DT_LEFT | DT_END_ELLIPSIS,
                );
                if let Some(note) = c.notes.get(i).filter(|n| !n.is_empty()) {
                    let after = text_left + text_width(dc, &f.text, item);
                    draw(
                        dc,
                        &f.small,
                        soft,
                        &format!("  {note}"),
                        RECT { left: after, right: cell.right - px(6, d), ..cell },
                        DT_LEFT | DT_END_ELLIPSIS,
                    );
                }
            }
        }
        let footer = v.footer_text();
        if !footer.is_empty() {
            let rect = RECT {
                left: px(6, d),
                top: height - pad - l.footer,
                right: width - px(6, d),
                bottom: height - pad,
            };
            draw(dc, &f.number, t.dim, &footer, rect, DT_RIGHT | DT_END_ELLIPSIS);
        }
    }
}

/// 후보창. 입력기를 쓰는 스레드마다 하나(처음 보일 때 만든다).
#[derive(Default)]
pub struct CandidateWindow {
    popup: Option<Popup>,
    view: Rc<RefCell<Option<CandidateView>>>,
}

/// 마우스 커서 자리(기준 사각형을 모를 때).
fn mouse_anchor(dpi: u32) -> RECT {
    let mut p = POINT::default();
    let _ = unsafe { GetCursorPos(&mut p) };
    RECT {
        left: p.x + px(16, dpi),
        top: p.y + px(8, dpi),
        right: p.x + px(16, dpi),
        bottom: p.y + px(8, dpi),
    }
}

impl CandidateWindow {
    /// 후보를 기준 사각형(조합이나 포커스 문절, 화면 좌표) 아래에 보인다. 모르면 마우스 옆.
    /// `font_size`는 후보 글자 크기(96 DPI 픽셀).
    pub fn show(&mut self, candidates: &Candidates, japanese: bool, anchor: Option<RECT>, font_size: i32) {
        if candidates.items.is_empty() {
            self.hide();
            return;
        }
        if self.popup.is_none() {
            self.popup = Popup::new(self.view.clone(), false);
        }
        let Some(popup) = &self.popup else { return };
        let mut view = CandidateView {
            candidates: candidates.clone(),
            japanese,
            font_size,
            theme: theme(),
            dpi: popup.dpi(),
            layout: Layout::default(),
        };
        popup.measure(|dc| view.measure(dc));
        let l = view.layout;
        let anchor = anchor.unwrap_or_else(|| mouse_anchor(view.dpi));
        let (x, y) = place(anchor, l.width, l.height, px(2, view.dpi), false);
        if let Ok(mut slot) = self.view.try_borrow_mut() {
            *slot = Some(view);
        }
        popup.show_at(x, y, l.width, l.height);
    }

    pub fn hide(&mut self) {
        if let Some(popup) = &self.popup {
            popup.hide();
        }
        if let Ok(mut slot) = self.view.try_borrow_mut() {
            *slot = None;
        }
    }

    #[cfg(test)]
    pub fn visible(&self) -> bool {
        self.view.try_borrow().map(|v| v.is_some()).unwrap_or(false)
    }
}

// ---- 모드 HUD ------------------------------------------------------------------------------------

const HUD_SHOW_MS: u32 = 1000;
const HUD_FADE_MS: u32 = 300;
const HUD_FADE_STEP_MS: u32 = 30;
const TIMER_HOLD: usize = 1;
const TIMER_FADE: usize = 2;

struct HudView {
    mode: Mode,
    theme: Theme,
    dpi: u32,
    alpha: std::cell::Cell<u8>,
}

impl Surface for RefCell<Option<HudView>> {
    fn paint(&self, _hwnd: HWND, dc: HDC, width: i32, height: i32) {
        let Ok(view) = self.try_borrow() else { return };
        let Some(v) = view.as_ref() else { return };
        fill(dc, RECT { left: 0, top: 0, right: width, bottom: height }, v.theme.background);
        let font = Font::new(px(22, v.dpi), true, w!("Malgun Gothic"));
        draw(
            dc,
            &font,
            v.theme.text,
            crate::langbar::label(v.mode),
            RECT { left: 0, top: 0, right: width, bottom: height },
            DT_CENTER,
        );
    }

    fn timer(&self, hwnd: HWND, id: usize) {
        let Ok(view) = self.try_borrow() else { return };
        let Some(v) = view.as_ref() else { return };
        unsafe {
            match id {
                TIMER_HOLD => {
                    let _ = KillTimer(Some(hwnd), TIMER_HOLD);
                    SetTimer(Some(hwnd), TIMER_FADE, HUD_FADE_STEP_MS, None);
                }
                TIMER_FADE => {
                    let step = (255 * HUD_FADE_STEP_MS / HUD_FADE_MS) as u8;
                    let alpha = v.alpha.get().saturating_sub(step);
                    v.alpha.set(alpha);
                    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
                    if alpha == 0 {
                        let _ = KillTimer(Some(hwnd), TIMER_FADE);
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    }
                }
                _ => {}
            }
        }
    }
}

/// 모드를 바꿀 때 잠깐 보이는 모드 글자(G/ㅊ/月).
#[derive(Default)]
pub struct Hud {
    popup: Option<Popup>,
    view: Rc<RefCell<Option<HudView>>>,
}

impl Hud {
    /// 커서 사각형(화면 좌표) 위에 보인다. 커서를 모르면 마우스 옆.
    pub fn show(&mut self, mode: Mode, caret: Option<RECT>) {
        if self.popup.is_none() {
            self.popup = Popup::new(self.view.clone(), true);
        }
        let Some(popup) = &self.popup else { return };
        let dpi = popup.dpi();
        let size = px(36, dpi);
        let anchor = caret.unwrap_or_else(|| mouse_anchor(dpi));
        let (x, y) = place(anchor, size, size, px(6, dpi), caret.is_some());
        if let Ok(mut slot) = self.view.try_borrow_mut() {
            *slot = Some(HudView { mode, theme: theme(), dpi, alpha: std::cell::Cell::new(255) });
        }
        unsafe {
            let _ = KillTimer(Some(popup.hwnd), TIMER_FADE);
            let _ = SetLayeredWindowAttributes(popup.hwnd, COLORREF(0), 255, LWA_ALPHA);
            SetTimer(Some(popup.hwnd), TIMER_HOLD, HUD_SHOW_MS, None);
        }
        popup.show_at(x, y, size, size);
    }
}

// ---- 화면 상태 -----------------------------------------------------------------------------------

/// 엔진 출력이 바꾼 화면(후보창, 모드 HUD)을 편집 세션이 자리를 잰 뒤에 맞춘다.
#[derive(Default)]
pub struct Screen {
    candidates: CandidateWindow,
    hud: Hud,
    /// 설정 파일의 `[windows]`(모드 HUD를 보일지·어디에, 후보 글자 크기).
    settings: WindowsConfig,
    /// 보일 후보(Some(None)이면 닫기). 다음 [`Screen::flush`]에서 맞춘다.
    pending_candidates: Option<Option<Candidates>>,
    pending_hud: Option<Mode>,
    japanese: bool,
    /// 마지막으로 잰 자리(이번에 못 재면 이것을 쓴다).
    last: Option<RECT>,
}

impl Screen {
    pub fn set_settings(&mut self, settings: WindowsConfig) {
        self.settings = settings;
    }

    /// 엔진 출력의 화면 변경을 받아 둔다. `hud`는 이 앱에서 모드를 바꿨을 때만(다른 앱을 따라갈 때는 보이지 않는다).
    pub fn queue(&mut self, candidates: Option<Option<Candidates>>, hud: Option<Mode>, japanese: bool) {
        if candidates.is_some() {
            self.pending_candidates = candidates;
        }
        if hud.is_some() && self.settings.hud {
            self.pending_hud = hud;
        }
        self.japanese = japanese;
    }

    pub fn pending(&self) -> bool {
        self.pending_candidates.is_some() || self.pending_hud.is_some()
    }

    /// 잰 자리로 받아 둔 변경을 보인다.
    pub fn flush(&mut self, rect: Option<RECT>) {
        if rect.is_some() {
            self.last = rect;
        }
        let rect = rect.or(self.last);
        match self.pending_candidates.take() {
            Some(Some(c)) => {
                self.candidates.show(&c, self.japanese, rect, self.settings.candidate_font_size as i32)
            }
            Some(None) => self.candidates.hide(),
            None => {}
        }
        if let Some(mode) = self.pending_hud.take() {
            let at = match self.settings.hud_position {
                HudPosition::Caret => rect,
                HudPosition::Mouse => None,
            };
            self.hud.show(mode, at);
        }
    }

    /// 조합이 끝났다(포커스 이동, 앱이 끝냄, 입력기 끄기): 후보창을 닫는다.
    pub fn close_candidates(&mut self) {
        self.pending_candidates = None;
        self.candidates.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_below_and_flips_above_at_the_screen_bottom() {
        let anchor = RECT { left: 100, top: 100, right: 110, bottom: 120 };
        assert_eq!(place(anchor, 50, 40, 2, false), (100, 122));
        assert_eq!(place(anchor, 50, 40, 2, true), (100, 58));
    }

    #[test]
    fn candidate_window_shows_and_hides() {
        let mut w = CandidateWindow::default();
        let c = Candidates {
            items: (0..12).map(|i| format!("候補{i}")).collect(),
            notes: vec![],
            selected: Some(10),
            page: Some((2, 2)),
            grid: false,
        };
        w.show(&c, true, Some(RECT { left: 200, top: 200, right: 210, bottom: 220 }), 15);
        assert!(w.visible());
        let view = w.view.borrow();
        let v = view.as_ref().unwrap();
        assert_eq!(v.page_range(), (9, 12), "고른 10번은 두 번째 페이지(9..12)");
        assert!(v.layout.width >= px(160, v.dpi) && v.layout.height > 0);
        assert_eq!(v.footer_text(), "2/2");
        drop(view);
        w.hide();
        assert!(!w.visible());
    }

    #[test]
    fn grid_footer_carries_the_selected_note() {
        let c = Candidates {
            items: vec!["國".into(), "局".into()],
            notes: vec!["나라 국".into(), "판 국".into()],
            selected: Some(1),
            page: Some((1, 1)),
            grid: true,
        };
        let v = CandidateView {
            candidates: c,
            japanese: false,
            font_size: 15,
            theme: theme(),
            dpi: 96,
            layout: Layout::default(),
        };
        assert_eq!(v.footer_text(), "판 국");
    }
}
