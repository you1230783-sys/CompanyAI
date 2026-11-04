//! 無邊框、逐像素透明的桌寵。僅依本機任務狀態換圖，不自行呼叫AI。
//! 視窗不搶焦點，透明區域由layered window穿透；點擊才通知主視窗。
mod bubble;
pub mod png;
mod typography;
use crate::{storage, wide, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
        WindowsAndMessaging::*,
    },
};

pub const MESSAGE: u32 = WM_APP + 28;
pub const ACTIONS: [&str; 6] = ["idle", "thinking", "working", "waiting", "success", "error"];
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub skin: String,
    pub size: u16,
    pub reduced_motion: bool,
    pub position: Option<[i32; 2]>,
    pub bubble_style: String,
    pub bubble_color: String,
    /// 0為完全透明，100為不透明；只影響泡泡。
    pub bubble_opacity: u8,
    /// 0秒表示不顯示通知泡泡，通知中心仍保存通知。
    pub notice_seconds: u8,
    /// 桌寵自己的邏輯像素字級；只隨Windows DPI縮放，不沿用主介面的字級或縮放比例。
    pub font_size: u8,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            skin: "cat".into(),
            size: 120,
            reduced_motion: false,
            position: None,
            bubble_style: "round".into(),
            bubble_color: "#FFF8ED".into(),
            bubble_opacity: 100,
            notice_seconds: 5,
            font_size: 16,
        }
    }
}
impl Settings {
    fn validate(&self) -> AppResult<()> {
        if !["cat", "dog", "custom"].contains(&self.skin.as_str())
            || !(72..=200).contains(&self.size)
            || !["round", "comic", "thought"].contains(&self.bubble_style.as_str())
            || self.bubble_color.len() != 7
            || !self.bubble_color.starts_with('#')
            || !self.bubble_color.as_bytes()[1..]
                .iter()
                .all(u8::is_ascii_hexdigit)
            || self.bubble_opacity > 100
            || self.notice_seconds > 5
            || !(12..=28).contains(&self.font_size)
        {
            return Err("桌寵角色、尺寸、字級或泡泡設定不正確。".into());
        }
        Ok(())
    }
}
struct State {
    typography: typography::Typography,
    owner: HWND,
    settings: Settings,
    images: Vec<png::Image>,
    phase: usize,
    notice: Option<(String, Instant)>,
    drag: Option<(POINT, RECT)>,
    moved: bool,
    position_dirty: bool,
    started: Instant,
}
pub struct Pet {
    window: HWND,
    state: Box<RefCell<State>>,
    root: PathBuf,
    encoded: Vec<Vec<u8>>,
    last_draw: Instant,
}
impl Drop for Pet {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.window);
        }
    }
}

fn builtin(skin: &str, action: usize) -> &'static [u8] {
    const CAT: [&[u8]; 6] = [
        include_bytes!("../ui/pets/cat_idle.png"),
        include_bytes!("../ui/pets/cat_thinking.png"),
        include_bytes!("../ui/pets/cat_working.png"),
        include_bytes!("../ui/pets/cat_waiting.png"),
        include_bytes!("../ui/pets/cat_success.png"),
        include_bytes!("../ui/pets/cat_error.png"),
    ];
    const DOG: [&[u8]; 6] = [
        include_bytes!("../ui/pets/dog_idle.png"),
        include_bytes!("../ui/pets/dog_thinking.png"),
        include_bytes!("../ui/pets/dog_working.png"),
        include_bytes!("../ui/pets/dog_waiting.png"),
        include_bytes!("../ui/pets/dog_success.png"),
        include_bytes!("../ui/pets/dog_error.png"),
    ];
    if skin == "dog" {
        DOG[action]
    } else {
        CAT[action]
    }
}
impl Pet {
    pub fn new(owner: HWND, root: &Path, hidden: bool) -> AppResult<Self> {
        let mut settings = std::fs::read(root.join("pet-settings.json"))
            .ok()
            .and_then(|v| serde_json::from_slice::<Settings>(&v).ok())
            .filter(|s| s.validate().is_ok())
            .unwrap_or_default();
        if hidden {
            settings.enabled = false;
        }
        let mut state = Box::new(RefCell::new(State {
            typography: typography::Typography::default(),
            owner,
            settings,
            images: vec![],
            phase: 0,
            notice: None,
            drag: None,
            moved: false,
            position_dirty: false,
            started: Instant::now(),
        }));
        let window = unsafe {
            let class_name = wide("LM_AI_DesktopPet");
            let instance = GetModuleHandleW(ptr::null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class_name.as_ptr(),
                hCursor: LoadCursorW(ptr::null_mut(), IDC_HAND),
                ..Default::default()
            };
            RegisterClassW(&class);
            // 不設主視窗為owner，避免主視窗最小化時Windows連帶隱藏桌寵。
            let window = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                class_name.as_ptr(),
                wide("LM_AI 桌寵通知").as_ptr(),
                WS_POPUP,
                0,
                0,
                320,
                250,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null(),
            );
            if window.is_null() {
                return Err("無法建立桌寵視窗。".into());
            }
            SetWindowLongPtrW(
                window,
                GWLP_USERDATA,
                (&mut *state as *mut RefCell<State>) as isize,
            );
            window
        };
        let mut pet = Self {
            window,
            state,
            root: root.to_path_buf(),
            encoded: vec![],
            last_draw: Instant::now() - Duration::from_secs(1),
        };
        pet.load_images()?;
        let position = pet.state.borrow().settings.position;
        unsafe {
            let mut work = RECT::default();
            SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut RECT).cast(), 0);
            let [x, y] = position.unwrap_or([work.right - 345, work.bottom - 275]);
            SetWindowPos(
                window,
                ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        pet.tick("idle")?;
        Ok(pet)
    }
    fn load_images(&mut self) -> AppResult<()> {
        let skin = self.state.borrow().settings.skin.clone();
        let mut images = vec![];
        let mut encoded = vec![];
        for (index, action) in ACTIONS.iter().enumerate() {
            let custom = if skin == "custom" {
                std::fs::read(self.root.join("pet-images").join(format!("{action}.png")))
                    .ok()
                    .filter(|v| v.len() < png::MAX_BYTES)
            } else {
                None
            };
            let result = custom
                .as_deref()
                .and_then(|v| png::normalize(v).ok())
                .map(Ok)
                .unwrap_or_else(|| png::normalize(builtin(&skin, index)))?;
            encoded.push(result.0);
            images.push(result.1);
        }
        self.state.borrow_mut().images = images;
        self.encoded = encoded;
        Ok(())
    }
    fn save(&self) -> AppResult<()> {
        storage::atomic_write(
            &self.root.join("pet-settings.json"),
            &serde_json::to_vec_pretty(&self.state.borrow().settings).map_err(|e| e.to_string())?,
        )
    }
    pub fn settings(&self) -> Settings {
        self.state.borrow().settings.clone()
    }

    /// 只有字型與應用程式共用；字級由桌寵偏好獨立保存。
    pub fn typography(&mut self, font: crate::config::UiFont) {
        if self.state.borrow_mut().typography.update(font) {
            self.last_draw = Instant::now() - Duration::from_secs(1);
        }
    }
    pub fn configure(&mut self, settings: Settings) -> AppResult<()> {
        settings.validate()?;
        let old = self.settings();
        let reload = old.skin != settings.skin;
        if old.notice_seconds != settings.notice_seconds {
            if let Some((_, deadline)) = &mut self.state.borrow_mut().notice {
                *deadline =
                    Instant::now() + Duration::from_secs(u64::from(settings.notice_seconds));
            }
        }
        if settings.notice_seconds == 0 {
            self.state.borrow_mut().notice = None;
        }
        self.state.borrow_mut().settings = settings;
        if reload || self.encoded.is_empty() {
            self.load_images()?;
        }
        self.save()?;
        self.last_draw = Instant::now() - Duration::from_secs(1);
        self.tick("idle")
    }
    pub fn import(&mut self, action: &str, path: &Path) -> AppResult<usize> {
        if !ACTIONS.contains(&action)
            || !path
                .extension()
                .is_some_and(|v| v.eq_ignore_ascii_case("png"))
        {
            return Err("每個動作只接受PNG，建議使用已去背的圖片。".into());
        }
        // 先檢查檔案長度，再讀取；WIC還會核對真正PNG格式及像素上限。
        if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 20_000_000 {
            return Err("匯入PNG請小於20MB。".into());
        }
        let source = std::fs::read(path).map_err(|e| e.to_string())?;
        let (bytes, _) = png::normalize(&source)?;
        let folder = self.root.join("pet-images");
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        storage::atomic_write(&folder.join(format!("{action}.png")), &bytes)?;
        self.encoded.clear(); // 已是自訂角色時，也必須重載剛匯入的動作。
        let mut settings = self.settings();
        settings.skin = "custom".into();
        self.configure(settings)?;
        Ok(bytes.len())
    }
    pub fn state(&self) -> AppResult<Value> {
        let images = self
            .encoded
            .iter()
            .zip(ACTIONS)
            .map(|(bytes, action)| {
                Ok(json!({"action":action,"bytes":bytes.len(),"url":png::data_url(bytes)?}))
            })
            .collect::<AppResult<Vec<_>>>()?;
        Ok(json!({"settings":self.settings(),"images":images}))
    }
    pub fn notify(&mut self, message: &str) {
        let seconds = self.settings().notice_seconds;
        if seconds == 0 {
            return;
        }
        self.state.borrow_mut().notice = Some((
            message.chars().take(180).collect(),
            Instant::now() + Duration::from_secs(u64::from(seconds)),
        ));
        self.last_draw = Instant::now() - Duration::from_secs(1);
        let _ = self.tick(if message.contains("失敗") || message.contains("錯誤") {
            "error"
        } else {
            "success"
        });
    }
    pub fn tick(&mut self, phase: &str) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        if state.position_dirty {
            state.position_dirty = false;
            drop(state);
            self.save()?;
            state = self.state.borrow_mut();
        }
        if state.notice.as_ref().is_some_and(|n| Instant::now() >= n.1) {
            state.notice = None;
        }
        if state.notice.is_none() {
            state.phase = ACTIONS.iter().position(|v| *v == phase).unwrap_or(0);
        } else if matches!(phase, "error" | "success") {
            state.phase = ACTIONS.iter().position(|v| *v == phase).unwrap_or(4);
        }
        let visible = state.settings.enabled || state.notice.is_some();
        unsafe {
            ShowWindow(
                self.window,
                if visible { SW_SHOWNOACTIVATE } else { SW_HIDE },
            );
        }
        if !visible || self.last_draw.elapsed() < Duration::from_millis(160) {
            return Ok(());
        }
        self.last_draw = Instant::now();
        unsafe { draw(self.window, &state) }
    }
}

unsafe fn draw(window: HWND, state: &State) -> AppResult<()> {
    let dpi = GetDpiForWindow(window).max(96) as i32;
    let scale = |n: i32| n * dpi / 96;
    let size = scale(state.settings.size as i32);
    let width = scale(320);
    let height = scale(136) + size + scale(8);
    let dc = GetDC(ptr::null_mut());
    let memory = CreateCompatibleDC(dc);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = ptr::null_mut();
    let bitmap = CreateDIBSection(memory, &info, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
    if bitmap.is_null() || bits.is_null() {
        if !bitmap.is_null() {
            DeleteObject(bitmap);
        }
        DeleteDC(memory);
        ReleaseDC(ptr::null_mut(), dc);
        return Err("無法建立桌寵繪圖緩衝區。".into());
    }
    let previous = SelectObject(memory, bitmap);
    let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), (width * height * 4) as usize);
    pixels.fill(0);
    if let Some((message, _)) = &state.notice {
        let bubble_height = scale(128) as usize;
        let mask = bubble::paint(pixels, width as usize, bubble_height, &state.settings);
        SetBkMode(memory, TRANSPARENT as i32);
        let [b, g, r] = bubble::color(&state.settings.bubble_color);
        let luminance = u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114;
        let (text_color, outline_color) = if luminance > 145_000 {
            (0x00000000, 0x00FFFFFF)
        } else {
            (0x00FFFFFF, 0x00000000)
        };
        let font = CreateFontW(
            -scale(i32::from(state.settings.font_size)),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            ANTIALIASED_QUALITY as u32,
            DEFAULT_PITCH as u32,
            wide(state.typography.family()).as_ptr(),
        );
        let old_font = SelectObject(memory, font);
        let text = wide(&format!("{message}\n點一下查看"));
        let mut text_rect = RECT {
            left: scale(24),
            top: scale(15),
            right: width - scale(24),
            bottom: scale(94),
        };
        // 八個方向先畫相反的黑／白色輪廓，再畫字心；各次矩形只平移，
        // 保持相同寬高及換行位置，避免描邊與前景文字在斷行處錯位。
        let stroke = scale(if state.settings.font_size >= 22 { 2 } else { 1 }).max(1);
        SetTextColor(memory, outline_color);
        for (dx, dy) in [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            let mut outline_rect = RECT {
                left: text_rect.left + dx * stroke,
                top: text_rect.top + dy * stroke,
                right: text_rect.right + dx * stroke,
                bottom: text_rect.bottom + dy * stroke,
            };
            DrawTextW(
                memory,
                text.as_ptr(),
                (text.len() - 1) as i32,
                &mut outline_rect,
                DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
        }
        SetTextColor(memory, text_color);
        DrawTextW(
            memory,
            text.as_ptr(),
            (text.len() - 1) as i32,
            &mut text_rect,
            DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        SelectObject(memory, old_font);
        DeleteObject(font);
        // GDI字體不寫alpha；依輪廓遮罩恢復透明度並轉為layered window預乘色。
        for (pixel, alpha) in pixels[..width as usize * bubble_height * 4]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(mask)
        {
            for channel in &mut pixel[..3] {
                *channel = (u16::from(*channel) * u16::from(alpha) / 255) as u8;
            }
            pixel[3] = alpha;
        }
    }
    if state.settings.enabled {
        let image = &state.images[state.phase];
        let bob = if state.settings.reduced_motion {
            0
        } else {
            ((state.started.elapsed().as_secs_f32() * 2.4).sin() * 2.0) as i32
        };
        let ratio = size as f64 / image.width.max(image.height) as f64;
        let w = (image.width as f64 * ratio) as i32;
        let h = (image.height as f64 * ratio) as i32;
        let left = width - size + (size - w) / 2;
        let top = scale(136) + size - h + bob.max(0);
        for y in 0..h {
            for x in 0..w {
                let from = ((y as usize * image.height / h as usize) * image.width
                    + x as usize * image.width / w as usize)
                    * 4;
                let to = (((top + y) * width + left + x) * 4) as usize;
                let alpha = u16::from(image.bgra[from + 3]);
                for c in 0..3 {
                    pixels[to + c] = (u16::from(image.bgra[from + c]) * alpha / 255) as u8;
                }
                pixels[to + 3] = alpha as u8;
            }
        }
    }
    let mut position = RECT::default();
    GetWindowRect(window, &mut position);
    let monitor = MonitorFromRect(&position, MONITOR_DEFAULTTONEAREST);
    let mut monitor_info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    GetMonitorInfoW(monitor, &mut monitor_info);
    let work = monitor_info.rcWork;
    let point = POINT {
        x: position
            .left
            .clamp(work.left, (work.right - width).max(work.left)),
        y: position
            .top
            .clamp(work.top, (work.bottom - height).max(work.top)),
    };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let ok = UpdateLayeredWindow(
        window,
        dc,
        &point,
        &SIZE {
            cx: width,
            cy: height,
        },
        memory,
        &POINT { x: 0, y: 0 },
        0,
        &blend,
        ULW_ALPHA,
    );
    SelectObject(memory, previous);
    DeleteObject(bitmap);
    DeleteDC(memory);
    ReleaseDC(ptr::null_mut(), dc);
    if ok == 0 {
        Err("桌寵透明視窗更新失敗。".into())
    } else {
        Ok(())
    }
}

unsafe extern "system" fn window_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    crate::reliability::win32(if message == WM_CREATE { -1 } else { 0 }, || {
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *const RefCell<State>;
        if !pointer.is_null() {
            if let Ok(mut state) = (&*pointer).try_borrow_mut() {
                match message {
                    WM_MOUSEACTIVATE => return MA_NOACTIVATE as isize,
                    WM_LBUTTONDOWN => {
                        let mut cursor = POINT::default();
                        let mut rect = RECT::default();
                        GetCursorPos(&mut cursor);
                        GetWindowRect(window, &mut rect);
                        state.drag = Some((cursor, rect));
                        state.moved = false;
                        SetCapture(window);
                        return 0;
                    }
                    WM_MOUSEMOVE => {
                        if let Some((start, rect)) = state.drag {
                            let mut now = POINT::default();
                            GetCursorPos(&mut now);
                            let dx = now.x - start.x;
                            let dy = now.y - start.y;
                            if dx.abs() + dy.abs() > 4 {
                                state.moved = true;
                            }
                            if state.moved {
                                SetWindowPos(
                                    window,
                                    ptr::null_mut(),
                                    rect.left + dx,
                                    rect.top + dy,
                                    0,
                                    0,
                                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE,
                                );
                            }
                        }
                        return 0;
                    }
                    WM_LBUTTONUP => {
                        if state.drag.take().is_some() {
                            ReleaseCapture();
                            if state.moved {
                                let mut rect = RECT::default();
                                GetWindowRect(window, &mut rect);
                                state.settings.position = Some([rect.left, rect.top]);
                                state.position_dirty = true;
                            } else {
                                PostMessageW(
                                    state.owner,
                                    MESSAGE,
                                    if state.notice.is_some() { 2 } else { 1 },
                                    0,
                                );
                                state.notice = None;
                            }
                        }
                        return 0;
                    }
                    WM_RBUTTONUP => {
                        // TrackPopupMenu會處理巢狀訊息；先釋放借用，讓主視窗tick可安全更新桌寵。
                        let owner = state.owner;
                        drop(state);
                        let menu = CreatePopupMenu();
                        if !menu.is_null() {
                            for (id, label) in [
                                (1, "開啟主視窗"),
                                (3, "塔羅牌桌"),
                                (4, "桌寵設定"),
                                (5, "隱藏桌寵"),
                            ] {
                                AppendMenuW(menu, MF_STRING, id, wide(label).as_ptr());
                            }
                            let mut cursor = POINT::default();
                            GetCursorPos(&mut cursor);
                            SetForegroundWindow(window);
                            let action = TrackPopupMenu(
                                menu,
                                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                                cursor.x,
                                cursor.y,
                                0,
                                window,
                                ptr::null(),
                            );
                            DestroyMenu(menu);
                            if action != 0 {
                                PostMessageW(owner, MESSAGE, action as usize, 0);
                            }
                        }
                        return 0;
                    }
                    WM_CAPTURECHANGED => {
                        state.drag = None;
                    }
                    WM_CLOSE => {
                        PostMessageW(state.owner, MESSAGE, 5, 0);
                        return 0;
                    }
                    _ => (),
                }
            }
        }
        DefWindowProcW(window, message, w, l)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_preferences_gain_bubble_defaults_and_reject_invalid_values() {
        let mut settings: Settings = serde_json::from_str(r#"{"skin":"dog","size":136}"#).unwrap();
        assert_eq!(settings.bubble_style, "round");
        assert_eq!(settings.notice_seconds, 5);
        assert_eq!(settings.font_size, 16);
        for font_size in [12, 16, 28] {
            settings.font_size = font_size;
            assert!(settings.validate().is_ok());
        }
        for font_size in [0, 11, 29] {
            settings.font_size = font_size;
            assert!(settings.validate().is_err());
        }
        settings.font_size = 16;
        for seconds in 0..=5 {
            settings.notice_seconds = seconds;
            assert!(settings.validate().is_ok());
        }
        settings.notice_seconds = 6;
        assert!(settings.validate().is_err());
        settings.notice_seconds = 0;
        settings.bubble_color = "#GGGGGG".into();
        assert!(settings.validate().is_err());
    }

    /// 實際建立透明視窗，核對隱藏角色仍能通知、通知到期及設定重開。
    /// 不向使用者視窗送輸入，也不依賴截圖外觀代替Win32狀態核對。
    #[test]
    fn native_notification_visibility_and_settings_survive_restart() {
        let _apartment = crate::com_apartment::Apartment::sta().unwrap();
        let root =
            std::env::temp_dir().join(format!("company-ai-pet-{}", crate::jobs::new_id().unwrap()));
        std::fs::create_dir(&root).unwrap();
        let mut pet = Pet::new(ptr::null_mut(), &root, true).unwrap();
        assert_eq!(unsafe { IsWindowVisible(pet.window) }, 0);
        let settings = Settings {
            enabled: true,
            skin: "dog".into(),
            size: 136,
            reduced_motion: true,
            font_size: 22,
            position: None,
            ..Default::default()
        };
        pet.configure(settings).unwrap();
        assert_ne!(unsafe { IsWindowVisible(pet.window) }, 0);
        assert_eq!(pet.state().unwrap()["images"].as_array().unwrap().len(), 6);
        for action in ACTIONS {
            pet.last_draw = Instant::now() - Duration::from_secs(1);
            pet.tick(action).unwrap();
        }
        let mut settings = pet.settings();
        settings.enabled = false;
        pet.configure(settings).unwrap();
        assert_eq!(unsafe { IsWindowVisible(pet.window) }, 0);
        pet.notify("合成通知：點擊可回到任務");
        assert_ne!(unsafe { IsWindowVisible(pet.window) }, 0);
        pet.state.borrow_mut().notice.as_mut().unwrap().1 = Instant::now();
        pet.tick("idle").unwrap();
        assert_eq!(unsafe { IsWindowVisible(pet.window) }, 0);
        let mut settings = pet.settings();
        settings.notice_seconds = 0;
        pet.configure(settings).unwrap();
        pet.notify("0秒不顯示");
        assert!(pet.state.borrow().notice.is_none());
        let window = pet.window;
        drop(pet);
        assert_eq!(unsafe { IsWindow(window) }, 0);
        let reopened = Pet::new(ptr::null_mut(), &root, false).unwrap();
        assert!(!reopened.settings().enabled);
        assert_eq!(reopened.settings().skin, "dog");
        assert_eq!(reopened.settings().size, 136);
        assert_eq!(reopened.settings().font_size, 22);
        drop(reopened);
        // 僅移除本測試建立的已知檔案；不遞迴刪除資料目錄。
        std::fs::remove_file(root.join("pet-settings.json")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
