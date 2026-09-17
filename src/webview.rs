//! WebView2 只顯示內嵌的本機介面，所有公司 API 與憑證仍由 Rust 管理。
//! 自訂來源攔截所有資源，不需要本機 HTTP 伺服器，也不依賴 CDN。
use crate::{wide, AppResult};
use std::{cell::Cell, path::Path, rc::Rc, sync::mpsc};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    core::{Interface, PCWSTR, PWSTR},
    Win32::{
        Foundation::{E_POINTER, HWND, RECT},
        UI::Shell::SHCreateMemStream,
    },
};
include!(concat!(env!("OUT_DIR"), "/assets.rs"));
const ORIGIN: &str = "https://lm-ai.local";
const PAGE: &str = "https://lm-ai.local/index.html";

pub struct WebView {
    controller: ICoreWebView2Controller,
    view: ICoreWebView2,
    recording: Rc<Cell<bool>>,
}
impl Drop for WebView {
    fn drop(&mut self) {
        unsafe {
            let _ = self.controller.Close();
        }
    }
}

impl WebView {
    pub fn create(
        window: windows_sys::Win32::Foundation::HWND,
        profile: &Path,
        messages: mpsc::Sender<String>,
    ) -> AppResult<Self> {
        Self::create_inner(HWND(window), profile, messages).map_err(|e| format!("無法啟動 LM_AI 介面：{e}。若尚未安裝 WebView2，請執行隨附的 MicrosoftEdgeWebView2RuntimeInstallerX64.exe，再重新開啟 LM_AI。"))
    }
    fn create_inner(
        window: HWND,
        profile: &Path,
        messages: mpsc::Sender<String>,
    ) -> webview2_com::Result<Self> {
        let profile = wide(&profile.to_string_lossy());
        let (tx, rx) = mpsc::channel();
        CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                CreateCoreWebView2EnvironmentWithOptions(
                    PCWSTR::null(),
                    PCWSTR(profile.as_ptr()),
                    None,
                    &handler,
                )
                .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |code, environment| {
                code?;
                tx.send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                    .map_err(|_| windows::core::Error::from(E_POINTER))?;
                Ok(())
            }),
        )?;
        let environment = rx.recv().map_err(|_| webview2_com::Error::SendError)??;
        let creator = environment.clone();
        let (tx, rx) = mpsc::channel();
        CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                creator
                    .CreateCoreWebView2Controller(window, &handler)
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |code, controller| {
                code?;
                tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                    .map_err(|_| windows::core::Error::from(E_POINTER))?;
                Ok(())
            }),
        )?;
        let controller = rx.recv().map_err(|_| webview2_com::Error::SendError)??;
        let recording = Rc::new(Cell::new(false));
        unsafe {
            let view = controller.CoreWebView2()?;
            let settings = view.Settings()?;
            settings.SetAreDevToolsEnabled(false)?;
            settings.SetIsStatusBarEnabled(false)?;
            settings.SetAreDefaultContextMenusEnabled(false)?;
            if let Ok(settings) = settings.cast::<ICoreWebView2Settings4>() {
                settings.SetIsPasswordAutosaveEnabled(false)?;
                settings.SetIsGeneralAutofillEnabled(false)?;
            }
            let mut token = 0;
            let recording_keys = recording.clone();
            let key_messages = messages.clone();
            // WebView2 的原生加速鍵事件比網頁 keydown 更早收到 Esc／功能鍵。
            // 只在使用者明確開啟錄製時攔截，且只作用於本程式的 WebView2。
            controller.add_AcceleratorKeyPressed(
                &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                    if !recording_keys.get() {
                        return Ok(());
                    }
                    if let Some(args) = args {
                        args.SetHandled(true)?;
                        let mut kind = COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;
                        args.KeyEventKind(&mut kind)?;
                        if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                            && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                        {
                            return Ok(());
                        }
                        let mut key = 0;
                        args.VirtualKey(&mut key)?;
                        if crate::hotkey::is_modifier(key) {
                            return Ok(());
                        }
                        let modifiers = crate::hotkey::pressed_modifiers();
                        let command = if key == 27 && modifiers == 0 {
                            serde_json::json!({"type": "cancel_hotkey_recording"})
                        } else {
                            serde_json::json!({
                                "type": "recorded_hotkey",
                                "modifiers": modifiers,
                                "key": key
                            })
                        };
                        let _ = key_messages.send(command.to_string());
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            view.add_PermissionRequested(
                &PermissionRequestedEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?;
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            view.add_NewWindowRequested(
                &NewWindowRequestedEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        args.SetHandled(true)?;
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            view.add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        let mut uri = PWSTR::null();
                        args.Uri(&mut uri)?;
                        if CoTaskMemPWSTR::from(uri).to_string() != PAGE {
                            args.SetCancel(true)?;
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            view.add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let mut source = PWSTR::null();
                        args.Source(&mut source)?;
                        if CoTaskMemPWSTR::from(source).to_string() == PAGE {
                            let mut message = PWSTR::null();
                            args.WebMessageAsJson(&mut message)?;
                            let message = CoTaskMemPWSTR::from(message).to_string();
                            if message.len() <= 100_000 {
                                let _ = messages.send(message);
                            }
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            view.AddWebResourceRequestedFilter(
                PCWSTR(wide("*").as_ptr()),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            )?;
            view.add_WebResourceRequested(&WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let request = args.Request()?; let mut uri = PWSTR::null(); request.Uri(&mut uri)?;
                    let uri = CoTaskMemPWSTR::from(uri).to_string();
                    let asset = url::Url::parse(&uri).ok().filter(|u| u.origin().ascii_serialization() == ORIGIN && u.query().is_none())
                        .and_then(|u| ASSETS.iter().find(|(path, _)| *path == u.path()));
                    let (status, mime, bytes) = match asset { Some((path, bytes)) => (200, mime_type(path), *bytes), None => (404, "text/plain", b"Resource unavailable".as_slice()) };
                    let stream = SHCreateMemStream(Some(bytes)).ok_or_else(|| windows::core::Error::from(E_POINTER))?;
                    let headers = wide(&format!("Content-Type: {mime}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff"));
                    let response = environment.CreateWebResourceResponse(&stream, status, PCWSTR(wide(if status == 200 {"OK"} else {"Not Found"}).as_ptr()), PCWSTR(headers.as_ptr()))?;
                    args.SetResponse(&response)?;
                } Ok(())
            })), &mut token)?;
            controller.SetIsVisible(true)?;
            view.Navigate(PCWSTR(wide(PAGE).as_ptr()))?;
            Ok(Self {
                controller,
                view,
                recording,
            })
        }
    }
    pub fn resize(&self, width: i32, height: i32) {
        unsafe {
            let _ = self.controller.SetBounds(RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            });
        }
    }
    pub fn set_recording(&self, active: bool) {
        self.recording.set(active);
    }
    /// 把已獲前景權限的主視窗焦點交給 WebView2，不用模擬貼上或附加其他程序輸入。
    pub fn focus(&self) {
        unsafe {
            let _ = self
                .controller
                .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
        }
    }
    pub fn post(&self, value: &serde_json::Value) -> AppResult<()> {
        let value = wide(&value.to_string());
        unsafe { self.view.PostWebMessageAsJson(PCWSTR(value.as_ptr())) }
            .map_err(|_| "無法更新介面。".into())
    }
}
fn mime_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}
