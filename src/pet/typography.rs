//! WebView的CSS字型不會自動出現在GDI。將相同內建TTF登記為程序私有字型，
//! 才能在沒有安裝這些字型的使用者電腦上畫出一致的桌寵通知。
use crate::config::{Config, UiFont};
use std::ptr;
use windows_sys::Win32::{Foundation::HANDLE, Graphics::Gdi::*};

pub(super) struct Typography {
    font: UiFont,
    size: u8,
    scale: u16,
    registration: HANDLE,
}

impl Default for Typography {
    fn default() -> Self {
        // 首次update時載入實際設定；Microsoft正黑體不需要額外登記。
        Self {
            font: UiFont::MicrosoftJhenghei,
            size: 14,
            scale: 100,
            registration: ptr::null_mut(),
        }
    }
}

impl Typography {
    pub(super) fn update(&mut self, config: &Config) -> bool {
        let scale = crate::config::interface_scale(config.interface_scale);
        let size = config.font_size.clamp(12, 20);
        if (self.font, self.size, self.scale) == (config.ui_font, size, scale) {
            return false;
        }
        if self.font != config.ui_font {
            self.unregister();
            let name = match config.ui_font {
                UiFont::MicrosoftJhenghei => "",
                UiFont::Huninn => "/fonts/jf-openhuninn-2.1.ttf",
                UiFont::WenkaiTc => "/fonts/LXGWWenKaiTC-Medium.ttf",
                UiFont::WenkaiMonoTc => "/fonts/LXGWWenKaiMonoTC-Medium.ttf",
            };
            // 共用WebView已內嵌的同一份TTF，避免在EXE重複保存約30MB字型。
            let bytes = crate::webview::ASSETS
                .iter()
                .find_map(|(path, bytes)| (*path == name).then_some(*bytes))
                .unwrap_or(&[]);
            if !bytes.is_empty() {
                let mut count = 0;
                // 內嵌bytes在程序生命週期內有效，不寫入Windows字型目錄。
                self.registration = unsafe {
                    AddFontMemResourceEx(
                        bytes.as_ptr().cast(),
                        bytes.len() as u32,
                        ptr::null(),
                        &raw mut count,
                    )
                };
            }
            self.font = config.ui_font;
        }
        self.size = size;
        self.scale = scale;
        true
    }

    pub(super) fn pixels(&self) -> i32 {
        (i32::from(self.size) * i32::from(self.scale) + 50) / 100
    }

    pub(super) fn family(&self) -> &'static str {
        match self.font {
            UiFont::MicrosoftJhenghei => "Microsoft JhengHei UI",
            UiFont::Huninn => "jf-openhuninn-2.1",
            UiFont::WenkaiTc => "LXGW WenKai TC Medium",
            UiFont::WenkaiMonoTc => "LXGW WenKai Mono TC Medium",
        }
    }

    fn unregister(&mut self) {
        if !self.registration.is_null() {
            unsafe {
                RemoveFontMemResourceEx(self.registration);
            }
            self.registration = ptr::null_mut();
        }
    }
}

impl Drop for Typography {
    fn drop(&mut self) {
        self.unregister();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_fonts_are_registered_and_settings_are_shared() {
        let mut typography = Typography::default();
        for font in [UiFont::Huninn, UiFont::WenkaiTc, UiFont::WenkaiMonoTc] {
            let config = Config {
                ui_font: font,
                font_size: 18,
                interface_scale: 125,
                ..Config::default()
            };
            assert!(typography.update(&config));
            assert!(!typography.registration.is_null());
            assert_eq!(typography.pixels(), 23);
            assert!(!typography.update(&config));
            // 核對GDI確實選到該字型，避免只驗證非空handle卻仍悄悄使用系統替代字型。
            unsafe {
                let dc = CreateCompatibleDC(ptr::null_mut());
                let font = CreateFontW(
                    -18,
                    0,
                    0,
                    0,
                    400,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    0,
                    0,
                    0,
                    0,
                    crate::wide(typography.family()).as_ptr(),
                );
                let old = SelectObject(dc, font);
                let mut actual = [0u16; 128];
                let length = GetTextFaceW(dc, actual.len() as i32, actual.as_mut_ptr());
                assert!(length > 0);
                let end = actual.iter().position(|v| *v == 0).unwrap_or(actual.len());
                let actual = String::from_utf16_lossy(&actual[..end]);
                // Windows可回傳TTF內的本地化別名，兩者必須都屬於同一內建字型。
                let aliases: &[&str] = match config.ui_font {
                    UiFont::Huninn => &["jf-openhuninn-2.1", "jf open 粉圓 2.1"],
                    UiFont::WenkaiTc => &[
                        "LXGW WenKai TC Medium",
                        "霞鶩文楷 TC Medium",
                        "霞鹜文楷 TC Medium",
                    ],
                    UiFont::WenkaiMonoTc => &[
                        "LXGW WenKai Mono TC Medium",
                        "霞鶩文楷等寬 TC Medium",
                        "霞鹜文楷等宽 TC Medium",
                    ],
                    UiFont::MicrosoftJhenghei => &["Microsoft JhengHei UI"],
                };
                assert!(
                    aliases.contains(&actual.as_str()),
                    "GDI選到了非預期字型：{actual}"
                );
                SelectObject(dc, old);
                DeleteObject(font);
                DeleteDC(dc);
            }
        }
    }
}
