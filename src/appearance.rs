//! 原生控制項的配色與 GDI 資源；保持快速啟動，不加入網頁執行環境。
use crate::{wide, AppResult};
use std::ptr;
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::Controls::DRAWITEMSTRUCT};

pub const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
pub const CANVAS: u32 = rgb(245, 246, 248);
pub const SURFACE: u32 = rgb(255, 255, 255);
pub const NAV: u32 = rgb(27, 30, 35);
pub const INK: u32 = rgb(29, 34, 43);
pub const MUTED: u32 = rgb(111, 120, 134);
pub const ACCENT: u32 = rgb(36, 105, 85);

/// 字型與筆刷活得比控制項久，最後才集中釋放，避免每次繪圖都累積 GDI 物件。
pub struct Appearance {
    pub font: HFONT,
    pub title: HFONT,
    pub small: HFONT,
    pub canvas: HBRUSH,
    pub surface: HBRUSH,
    pub nav: HBRUSH,
}
impl Appearance {
    pub fn new(scale: f32) -> AppResult<Self> {
        unsafe {
            let font = |size: f32, weight| {
                CreateFontW(
                    (-size * scale) as i32,
                    0,
                    0,
                    0,
                    weight,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    0,
                    0,
                    CLEARTYPE_QUALITY as u32,
                    0,
                    wide("Microsoft JhengHei UI").as_ptr(),
                )
            };
            let style = Self {
                font: font(16.0, 400),
                title: font(25.0, 600),
                small: font(13.0, 400),
                canvas: CreateSolidBrush(CANVAS),
                surface: CreateSolidBrush(SURFACE),
                nav: CreateSolidBrush(NAV),
            };
            if [
                style.font,
                style.title,
                style.small,
                style.canvas,
                style.surface,
                style.nav,
            ]
            .iter()
            .any(|h| h.is_null())
            {
                return Err("無法建立介面繪圖資源。".into());
            }
            Ok(style)
        }
    }
    /// 背景與兩張留白卡片；實際文字仍由可存取的標準 Windows 控制項呈現。
    pub fn background(&self, dc: HDC, width: i32, height: i32, scale: f32) {
        let px = |n: i32| (n as f32 * scale).round() as i32;
        unsafe {
            FillRect(
                dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                },
                self.canvas,
            );
            FillRect(
                dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: px(220),
                    bottom: height,
                },
                self.nav,
            );
        }
        rounded(
            dc,
            RECT {
                left: px(242),
                top: px(126),
                right: width - px(24),
                bottom: height - px(290),
            },
            SURFACE,
            rgb(231, 234, 239),
        );
        rounded(
            dc,
            RECT {
                left: px(242),
                top: height - px(226),
                right: width - px(24),
                bottom: height - px(34),
            },
            SURFACE,
            rgb(221, 225, 231),
        );
    }
    /// Owner-draw 僅改外觀，保留 BUTTON 的鍵盤操作與無障礙名稱。
    pub fn button(&self, draw: &DRAWITEMSTRUCT, text: &str, primary: bool, sidebar: bool) {
        let disabled = draw.itemState & 4 != 0;
        let pressed = draw.itemState & 1 != 0;
        let background = if disabled {
            if sidebar {
                NAV
            } else {
                rgb(234, 237, 241)
            }
        } else if pressed {
            rgb(48, 76, 70)
        } else if primary {
            ACCENT
        } else if sidebar {
            rgb(40, 44, 51)
        } else {
            SURFACE
        };
        rounded(draw.hDC, draw.rcItem, background, background);
        unsafe {
            let old = SelectObject(draw.hDC, self.font);
            SetBkMode(draw.hDC, TRANSPARENT as i32);
            SetTextColor(
                draw.hDC,
                if disabled {
                    MUTED
                } else if primary || sidebar || pressed {
                    SURFACE
                } else {
                    INK
                },
            );
            let mut rect = draw.rcItem;
            DrawTextW(
                draw.hDC,
                wide(text).as_ptr(),
                -1,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            if draw.itemState & 16 != 0 {
                rect.left += 3;
                rect.right -= 3;
                rect.top += 3;
                rect.bottom -= 3;
                DrawFocusRect(draw.hDC, &rect);
            }
            SelectObject(draw.hDC, old);
        }
    }
}
impl Drop for Appearance {
    fn drop(&mut self) {
        unsafe {
            for object in [
                self.font,
                self.title,
                self.small,
                self.canvas,
                self.surface,
                self.nav,
            ] {
                if !object.is_null() {
                    DeleteObject(object);
                }
            }
        }
    }
}
fn rounded(dc: HDC, rect: RECT, color: u32, border: u32) {
    unsafe {
        let brush = CreateSolidBrush(color);
        let pen = CreatePen(PS_SOLID, 1, border);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, pen);
        RoundRect(dc, rect.left, rect.top, rect.right, rect.bottom, 12, 12);
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(brush);
        DeleteObject(pen);
    }
}
pub fn invalidate(window: HWND) {
    unsafe {
        InvalidateRect(window, ptr::null(), 0);
    }
}
