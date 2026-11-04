//! 截圖分為能力檢查、框選、預覽與明確接受；期間不提交模型請求。
use super::*;
use crate::{
    attachments::{Attachment, Incoming, CHUNK_BYTES},
    projects::Project,
};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ScreenshotCommand {
    Prepare,
    Begin { id: String },
    Accept { id: String },
    Cancel,
}
struct Pending {
    id: String,
    conversation: String,
    generation: u64,
    model: String,
    project: Option<Project>,
    // 寫入時於固定的handle下核對身分，預覽期間不持有檔案鎖。
    root_identity: Option<[u32; 3]>,
    bytes: Option<Vec<u8>>,
    started: bool,
    cancel: Arc<AtomicBool>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
#[derive(Default)]
pub(super) struct Runtime {
    pending: Option<Pending>,
    pub hotkey_registered: bool,
}
impl App {
    pub(super) fn unregister_screenshot_hotkey(&mut self) {
        if self.screenshot.hotkey_registered {
            // SAFETY: 只解除本視窗擁有的固定截圖識別碼。
            unsafe {
                windows_sys::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey(
                    self.window,
                    crate::screenshot::HOTKEY_ID,
                );
            }
            self.screenshot.hotkey_registered = false;
        }
    }
    /// 使用者的選字快捷鍵優先；錄製期間不讓截圖快捷鍵攔截同一組按鍵。
    pub(super) fn sync_screenshot_hotkey(&mut self) {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
        };
        self.unregister_screenshot_hotkey();
        if self.smoke || self.recording_since.is_some() || !screenshot_key_available(&self.config) {
            return;
        }
        // SAFETY: 呼叫者持有有效主視窗，失敗僅停用快捷鍵，截圖按鈕仍可用。
        self.screenshot.hotkey_registered = unsafe {
            RegisterHotKey(
                self.window,
                crate::screenshot::HOTKEY_ID,
                MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
                0x53,
            )
        } != 0;
    }

    /// 隱藏視窗自檢只使用合成圖片及獨立暫存目錄，不擷取真實桌面。
    pub(super) fn verify_screenshot_flow(&mut self) -> AppResult<()> {
        if !self.smoke {
            return Err("截圖自檢僅供 self-check。".into());
        }
        let original_root = self.root.clone();
        let original_config = self.config.clone();
        self.root = original_root.join(format!("screenshot-{}", crate::jobs::new_id()?));
        let result = self.screenshot_smoke_cases();
        self.screenshot.pending = None;
        self.session = None;
        self.config = original_config;
        self.projects = Default::default();
        self.work = Default::default();
        self.archive = Default::default();
        self.active_id = None;
        self.messages.clear();
        self.draft.clear();
        self.root = original_root;
        let cases = result?;
        storage::atomic_write(
            &self.root.join("screenshot-verification.json"),
            &serde_json::to_vec_pretty(
                &json!({"version":env!("CARGO_PKG_VERSION"),"result":"PASS","cases":cases,
                "real_desktop_capture_tested":false,"company_service_tested":false}),
            )
            .map_err(|e| e.to_string())?,
        )
    }
    fn screenshot_smoke_cases(&mut self) -> AppResult<Vec<&'static str>> {
        fn verify(value: bool, case: &str) -> AppResult<()> {
            if value {
                Ok(())
            } else {
                Err(format!("截圖流程自檢失敗：{case}"))
            }
        }
        let server = crate::demo::DemoServer::start()?;
        self.config = server.config();
        self.config.model = "quality".into();
        self.session = Some(Session {
            access_token: "screenshot-fixture".into(),
            expires_at: crate::unix_now() + 3600,
            binding: self.config.binding()?,
        });
        self.versions = Default::default();
        self.history_error = None;
        self.busy = "none";
        self.work = Default::default();
        self.archive = Default::default();
        self.projects = Default::default();
        self.active_id = None;
        self.work.store.principal_id = "screenshot_fixture".into();
        // 控制器測試使用明確能力資料；網站傳輸另由既有loopback附件測試驗證。
        self.work.caps = Some(serde_json::from_value(json!({
            "contract_version":1, "principal_id":"screenshot_fixture", "execution_modes":["stream"],
            "attachments":{"enabled":true,"max_count":20,"max_file_bytes":5_000_000,
                "max_total_bytes":50_000_000,"allowed_extensions":[".jpg"]}
        })).map_err(|e|e.to_string())?);
        self.work.capability_model = "quality".into();
        self.config.model = "fast".into();
        verify(
            self.screenshot_command(ScreenshotCommand::Prepare).is_err()
                && self.screenshot.pending.is_none(),
            "fast model rejected before capture",
        )?;
        self.config.model = "quality".into();
        let jpeg = crate::projects::vision::jpeg::convert(
            include_bytes!("../../ui/pets/cat_idle.png"),
            "png",
        )?
        .bytes;
        self.screenshot_command(ScreenshotCommand::Prepare)?;
        let id = self
            .screenshot
            .pending
            .as_ref()
            .ok_or("測試預覽未建立。")?
            .id
            .clone();
        self.screenshot_event(id.clone(), Ok(jpeg.clone()))?;
        verify(
            self.work.store.attachments.is_empty() && !self.root.join("uploads").exists(),
            "preview neither saves nor uploads",
        )?;
        let conversation = self.active_id.clone();
        self.active_id = None;
        verify(
            self.screenshot_command(ScreenshotCommand::Accept { id: id.clone() })
                .is_err()
                && self.work.store.attachments.is_empty(),
            "changed conversation rejects acceptance",
        )?;
        self.active_id = conversation;
        self.screenshot_command(ScreenshotCommand::Prepare)?;
        let id = self
            .screenshot
            .pending
            .as_ref()
            .ok_or("測試預覽未建立。")?
            .id
            .clone();
        self.screenshot_event(id.clone(), Ok(jpeg.clone()))?;
        self.screenshot_command(ScreenshotCommand::Accept { id: id.clone() })?;
        verify(
            self.work.store.attachments.len() == 1 && self.work.store.tasks.is_empty(),
            "accept creates attachment only, never starts chat",
        )?;
        verify(
            self.screenshot_command(ScreenshotCommand::Accept { id })
                .is_err()
                && self.work.store.attachments.len() == 1,
            "repeated accept cannot duplicate attachment",
        )?;
        self.work = Default::default();
        let root = self.root.join("project");
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let source = root.join("source.txt");
        std::fs::write(&source, b"original").map_err(|e| e.to_string())?;
        let project = self.projects.store.add("合成圖片專案", root.clone())?;
        let conversation = self.active_id.clone().ok_or("測試對話遺失。")?;
        self.projects
            .store
            .conversations
            .insert(conversation, project);
        self.screenshot_command(ScreenshotCommand::Prepare)?;
        let id = self
            .screenshot
            .pending
            .as_ref()
            .ok_or("測試預覽未建立。")?
            .id
            .clone();
        self.screenshot_event(id.clone(), Ok(jpeg.clone()))?;
        self.set_draft("原草稿".into());
        self.screenshot_command(ScreenshotCommand::Accept { id })?;
        verify(
            self.draft.starts_with("原草稿\n\n請查看截圖：_AI_Output/")
                && self.work.store.tasks.is_empty(),
            "project saves new image and preserves draft without starting task",
        )?;
        let path = self
            .draft
            .split_once("請查看截圖：")
            .ok_or("測試圖片路徑遺失。")?
            .1;
        verify(
            std::fs::read(root.join(path)).map_err(|e| e.to_string())? == jpeg
                && std::fs::read(source).map_err(|e| e.to_string())? == b"original",
            "project bytes read back exactly and source remains unchanged",
        )?;
        self.screenshot_command(ScreenshotCommand::Prepare)?;
        let id = self
            .screenshot
            .pending
            .as_ref()
            .ok_or("測試預覽未建立。")?
            .id
            .clone();
        self.generation += 1;
        verify(
            self.screenshot_context(&id).is_err(),
            "account generation invalidates pending capture",
        )?;
        self.screenshot_command(ScreenshotCommand::Prepare)?;
        let id = self
            .screenshot
            .pending
            .as_ref()
            .ok_or("測試預覽未建立。")?
            .id
            .clone();
        self.screenshot_event(id.clone(), Ok(jpeg))?;
        std::fs::rename(&root, root.with_extension("original")).map_err(|e| e.to_string())?;
        std::fs::create_dir(&root).map_err(|e| e.to_string())?;
        verify(
            self.screenshot_command(ScreenshotCommand::Accept { id })
                .is_err()
                && !root.join("_AI_Output").exists(),
            "replaced root rejected while pinned before write",
        )?;
        self.screenshot_command(ScreenshotCommand::Prepare)?;
        let id = self
            .screenshot
            .pending
            .as_ref()
            .ok_or("測試預覽未建立。")?
            .id
            .clone();
        self.screenshot_event(id, Err("合成逾時".into()))?;
        verify(
            self.screenshot.pending.is_none() && !root.join("_AI_Output").exists(),
            "capture timeout clears pending state without writing",
        )?;
        Ok(vec![
            "fast-model precheck",
            "preview has no disk/network side effect",
            "conversation binding",
            "attachment without chat",
            "one-shot acceptance",
            "project copy and draft preservation",
            "byte readback and source preservation",
            "account generation binding",
            "root replacement rejected before write",
            "timeout clears pending state",
        ])
    }
    fn screenshot_check(&self, size: usize) -> AppResult<()> {
        if !self.logged_in() || self.versions.blocked() || self.panic_fault {
            return Err("請先登入並完成必要更新。".into());
        }
        if !crate::projects::vision::input::model_supported(&self.config.model) {
            return Err("目前模型不支援截圖，請先切換為高品質模型再框選。".into());
        }
        if self.busy != "none"
            || self.projects.active(self.active_id.as_deref())
            || self.work.incoming.is_some()
            || self.work.store.pending(self.active_id.as_deref())
            || (self.mail_flow.phase != "idle" && self.mail_flow.conversation == self.active_id)
        {
            return Err("請先完成目前工作，再加入截圖。".into());
        }
        if self
            .active_id
            .as_deref()
            .and_then(|id| self.projects.store.project_for(id))
            .is_none()
        {
            if self.work.storage_error
                || self.work.capability_loading
                || self.work.capability_model != self.config.model
            {
                return Err("網站附件能力尚未準備好，請稍後再試。".into());
            }
            let files = self.work.store.drafts(self.active_id.as_deref());
            self.work
                .caps
                .as_ref()
                .ok_or("網站尚未提供附件能力。")?
                .attachments
                .check(
                    "截圖.jpg",
                    size as u64,
                    files.len(),
                    files.iter().map(|a| a.size).sum(),
                )?;
        }
        Ok(())
    }
    fn screenshot_context(&self, id: &str) -> AppResult<&Pending> {
        let pending = self
            .screenshot
            .pending
            .as_ref()
            .ok_or("截圖已取消，請重新開始。")?;
        let current = self
            .active_id
            .as_deref()
            .and_then(|id| self.projects.store.project_for(id));
        if pending.id != id
            || self.active_id.as_deref() != Some(&pending.conversation)
            || pending.generation != self.generation
            || pending.model != self.config.model
            || pending.project.as_ref().map(|p| (&p.id, &p.root))
                != current.map(|p| (&p.id, &p.root))
        {
            return Err("對話、帳號、模型或專案已變更；這次截圖不會加入，請重新開始。".into());
        }
        self.screenshot_check(pending.bytes.as_ref().map_or(1, Vec::len))?;
        Ok(pending)
    }
    pub(super) fn screenshot_command(&mut self, command: ScreenshotCommand) -> AppResult<()> {
        let result = (|| {
            match command {
                ScreenshotCommand::Cancel => {
                    self.screenshot.pending = None;
                }
                ScreenshotCommand::Prepare => {
                    self.screenshot.pending = None;
                    self.screenshot_check(1)?;
                    let conversation = self.ensure_local_conversation()?;
                    let project = self.projects.store.project_for(&conversation).cloned();
                    let root_identity = project
                        .as_ref()
                        .map(|p| crate::projects::files::directory_identity(&p.root))
                        .transpose()?;
                    let id = crate::jobs::new_id()?;
                    let destination = if let Some(project) = &project {
                        format!("專案「{}」：確認後保存至此專案的 _AI_Output 並加入草稿；送出任務後，AI 按需將圖片送到公司模型分析。每次分析計入既有的每任務100張上限；只保存圖片不扣額度。", project.name)
                    } else {
                        "一般對話：確認後會立即上傳到公司網站，作為目前對話的附件；問題仍由你按送出。".into()
                    };
                    self.screenshot.pending = Some(Pending {
                        id: id.clone(),
                        conversation,
                        generation: self.generation,
                        model: self.config.model.clone(),
                        project,
                        root_identity,
                        bytes: None,
                        started: false,
                        cancel: Arc::new(AtomicBool::new(false)),
                    });
                    if !self.smoke {
                        show_window(self.window);
                    }
                    self.publish();
                    self.view.post(&json!({"type":"screenshot_prepare","id":id,"destination":destination,"hotkey":self.screenshot.hotkey_registered}))?;
                }
                ScreenshotCommand::Begin { id } => {
                    self.screenshot_context(&id)?;
                    let pending = self.screenshot.pending.as_mut().ok_or("截圖已取消。")?;
                    if pending.started {
                        return Err("框選已開始，請完成或取消。".into());
                    }
                    pending.started = true;
                    // 讓使用者能框選原來的工作視窗，而非被主視窗遮住。
                    unsafe {
                        ShowWindow(self.window, SW_MINIMIZE);
                    }
                    let cancel = pending.cancel.clone();
                    let tx = self.tx.clone();
                    crate::reliability::spawn(move || {
                        let _ = tx.send(Event::Screenshot(id, crate::screenshot::capture(&cancel)));
                    });
                    self.view.post(&json!({"type":"screenshot_waiting"}))?;
                }
                ScreenshotCommand::Accept { id } => {
                    self.screenshot_context(&id)?;
                    // 先消耗確認，失敗也不重播可能成功的寫入。
                    let mut pending = self.screenshot.pending.take().ok_or("截圖已取消。")?;
                    let bytes = pending.bytes.take().ok_or("請完成框選並預覽圖片。")?;
                    if let Some(project) = &pending.project {
                        if self.draft.encode_utf16().count() > 15_000 {
                            return Err("請先縮短草稿再加入截圖。".into());
                        }
                        let path = crate::projects::files::save_user_screenshot(
                            project,
                            &pending.conversation,
                            pending.root_identity.ok_or("專案根目錄身分遺失。")?,
                            &bytes,
                        )?;
                        if let Err(error) = self.append_draft_text(&format!("請查看截圖：{path}"))
                        {
                            return Err(format!("截圖已保存於 {path}，但草稿未加入：{error}。請使用現有圖片，不要重複保存。"));
                        }
                    } else {
                        self.attach_screenshot(&pending.conversation, &bytes)?;
                    }
                    self.view.post(&json!({"type":"screenshot_done"}))?;
                    self.publish();
                }
            }
            Ok(())
        })();
        if let Err(error) = &result {
            if !self.smoke {
                show_window(self.window);
            }
            self.screenshot.pending = None;
            self.view
                .post(&json!({"type":"screenshot_error","message":error}))?;
        }
        result
    }
    pub(super) fn screenshot_event(
        &mut self,
        id: String,
        result: AppResult<Vec<u8>>,
    ) -> AppResult<()> {
        if self.screenshot.pending.as_ref().is_none_or(|p| p.id != id) {
            return Ok(());
        }
        let result = result.and_then(|bytes| {
            self.screenshot_context(&id)?;
            self.screenshot_check(bytes.len())?;
            // 僅在本機WebView顯示預覽，不呼叫公司網站。
            let url = crate::png_preview::data_url(&bytes)?.replacen("image/png", "image/jpeg", 1);
            self.view
                .post(&json!({"type":"screenshot_preview","id":id,"url":url}))?;
            self.screenshot
                .pending
                .as_mut()
                .ok_or("截圖已取消。")?
                .bytes = Some(bytes);
            if !self.smoke {
                show_window(self.window);
            }
            Ok(())
        });
        if let Err(error) = result {
            if !self.smoke {
                show_window(self.window);
            }
            self.screenshot.pending = None;
            self.view
                .post(&json!({"type":"screenshot_error","message":error}))?;
        }
        Ok(())
    }
    fn attach_screenshot(&mut self, conversation: &str, bytes: &[u8]) -> AppResult<()> {
        let id = crate::jobs::new_id()?;
        let mut incoming = Incoming::new(&self.root, id.clone(), bytes.len() as u64)?;
        for chunk in bytes.chunks(CHUNK_BYTES) {
            incoming.append_bytes(chunk)?;
        }
        incoming.finish()?;
        self.work.store.attachments.push(Attachment {
            id: id.clone(),
            conversation_id: conversation.into(),
            name: format!("截圖_{}.jpg", crate::unix_now()),
            size: bytes.len() as u64,
            mime_type: "image/jpeg".into(),
            remote: None,
            state: "upload_pending".into(),
            message: String::new(),
            sent: false,
            removed: false,
            uploaded_bytes: 0,
        });
        if let Err(error) = self.work_save() {
            self.work.store.attachments.retain(|a| a.id != id);
            // 暫存是隨機識別碼的本機加密檔，未提交網路；保留失敗紀錄，不自動重試。
            return Err(error);
        }
        Ok(())
    }
}

fn screenshot_key_available(config: &Config) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL};
    !config.hotkey_enabled
        || !Hotkey::parse(&config.hotkey)
            .is_ok_and(|key| key.modifiers == (MOD_CONTROL | MOD_ALT) && key.key == 0x53)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn user_selection_hotkey_has_priority_over_screenshot() {
        let mut config = Config {
            hotkey_enabled: true,
            hotkey: "Ctrl+Alt+S".into(),
            ..Config::default()
        };
        assert!(!screenshot_key_available(&config));
        config.hotkey = "Alt+Control+s".into();
        assert!(!screenshot_key_available(&config));
        config.hotkey_enabled = false;
        assert!(screenshot_key_available(&config));
        config.hotkey_enabled = true;
        config.hotkey = "Ctrl+Alt+Q".into();
        assert!(screenshot_key_available(&config));
    }
}
