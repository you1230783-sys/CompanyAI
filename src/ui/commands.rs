//! 主視窗的commands操作；由父模組保留共用狀態與既有介面。
use super::*;

impl App {
    pub(super) fn command(&mut self, command: Command) -> AppResult<()> {
        if self.workspace_import.is_some()
            && !matches!(
                command,
                Command::Workspace { .. } | Command::Draft { .. } | Command::Exit | Command::Logout
            )
        {
            return Err("正在匯入本機檔案，請等待完成後再切換對話或送出。".into());
        }
        if crate::reliability::faulted()
            && !matches!(
                command,
                Command::Ready
                    | Command::Exit
                    | Command::CopyError
                    | Command::ExportDiagnostics { .. }
                    | Command::SaveDiagnostics { .. }
                    | Command::CancelDiagnostics
            )
        {
            return Err("Error Code: 13\n本次執行已進入保護狀態，請匯出診斷並重新啟動。".into());
        }
        command.check_access(self.logged_in(), self.versions.blocked(), self.smoke)?;
        match command {
            Command::DocumentExport { command } => self.export_command(command)?,
            Command::Workspace { command } => self.workspace_command(command)?,
            Command::Reminder { command } => self.reminder_command(command)?,
            Command::Template { command } => self.template_command(command)?,
            Command::Screenshot { command } => self.screenshot_command(command)?,
            Command::Handoff { command } => {
                let id = command.correlation().to_owned();
                if let Err(error) = self.handoff_command(command) {
                    self.view
                        .post(&json!({"type":"handoff_error","id":id,"message":error}))?;
                    return Err(error);
                }
            }
            #[cfg(feature = "pet")]
            Command::Pet { command } => self.pet_command(command)?,
            #[cfg(feature = "tarot")]
            Command::Tarot { command } => self.tarot_command(command)?,
            Command::OnboardingAck { topic } => self.onboarding.acknowledge(&topic)?,
            Command::OnboardingWelcomeDismiss => self.onboarding.dismiss_practice_welcome()?,
            Command::ExportDiagnostics { include_details } => {
                self.export_diagnostics(include_details)?
            }
            Command::SaveDiagnostics { preview_id } => self.save_diagnostics(&preview_id)?,
            Command::CancelDiagnostics => self.diagnostic_preview = None,
            Command::CopyError => {
                let text = format!("LM_AI {}\n{}", service::CURRENT_VERSION, self.status);
                selection::copy_text(self.window, &text)?;
                self.toast("已複製目前錯誤資訊");
            }
            Command::Project { command } => {
                let stage = if matches!(
                    &command,
                    projects::ProjectCommand::Create { .. }
                        | projects::ProjectCommand::CreateDefault { .. }
                ) {
                    "project_folder"
                } else {
                    "project"
                };
                self.project_command(command)
                    .map_err(|e| self.friendly_error(stage, &e))?;
            }
            #[cfg(feature = "vnc")]
            Command::Vnc { command } => self.vnc_command(command)?,
            Command::SiteAction { command } => self.site_action(command)?,
            Command::MailBatch { command } => self.mail_batch_command(command)?,
            Command::Work { command } => self.work_command(command)?,
            Command::StartHotkeyRecording => {
                if let Err(message) = self.start_recording() {
                    // 設定對話框會遮住主畫面的狀態列，錯誤須直接顯示在錄製欄位旁。
                    self.view
                        .post(&json!({"type":"hotkey_error","message":message}))?;
                    return Err(message);
                }
            }
            Command::CancelHotkeyRecording => self.stop_recording()?,
            Command::RecordedHotkey { modifiers, key } => {
                if self.recording_since.is_some() {
                    let hotkey = match Hotkey::from_keys(modifiers, key) {
                        Ok(value) => value,
                        Err(message) => {
                            self.view
                                .post(&json!({"type":"hotkey_error","message":message}))?;
                            return Ok(());
                        }
                    };
                    self.stop_recording()?;
                    self.view
                        .post(&json!({"type":"hotkey_recorded","value":hotkey.label()}))?;
                }
            }
            Command::Ready => {
                self.view.set_interface_scale(self.config.interface_scale)?;
                self.ready = true;
                self.publish();
                if self.smoke && !self.panic_probe {
                    // 自檢樣本經正式 Rust 解析後，透過既有訊息橋傳給畫面。
                    // 前端禁止自行連網，不為測試放寬 CSP 或開放 fetch。
                    let payload: serde_json::Value = serde_json::from_str(include_str!(
                        "../../ui/fixtures/structured-reply.json"
                    ))
                    .map_err(|_| "內建結構化回覆測試資料無效。")?;
                    let message = crate::protocol::assistant_message(&payload)?;
                    let completion: serde_json::Value = serde_json::from_str(include_str!(
                        "../../ui/fixtures/completion-reply.json"
                    ))
                    .map_err(|_| "內建完成結果測試資料無效。")?;
                    let completion_message = crate::protocol::assistant_message(&json!({
                        "result":completion
                    }))?;
                    let inline_text = include_str!("../../ui/fixtures/inline-reply.txt");
                    let mut inline_message = crate::protocol::assistant_message(&json!({
                        "answer":"您好！這是一個測試訊息。我已準備好為您提供協助。"
                    }))?;
                    crate::protocol::preserve_received_reply(
                        &mut inline_message,
                        inline_text,
                        true,
                    );
                    let mut changed_message = crate::protocol::assistant_message(&json!({
                        "answer":"完成後的正文已有修改。"
                    }))?;
                    crate::protocol::preserve_received_reply(
                        &mut changed_message,
                        inline_text,
                        true,
                    );
                    self.view.post(&json!({
                        "type":"self_test",
                        "reply_fixture":{"payload":payload,"message":message,
                            "inline_text":inline_text,"inline_message":inline_message,
                            "changed_message":changed_message,"completion_message":completion_message}
                    }))?;
                }
            }
            Command::SelfTestResult { ok, detail } => {
                if self.smoke {
                    if !ok {
                        self.smoke_result = Some(Err(detail));
                    } else {
                        if let Err(error) = self.verify_project_composer() {
                            self.smoke_result = Some(Err(error));
                            return Ok(());
                        }
                        if let Err(error) = self.verify_task_tracking() {
                            self.smoke_result = Some(Err(error));
                            return Ok(());
                        }
                        if let Err(error) = self.verify_screenshot_flow() {
                            self.smoke_result = Some(Err(error));
                            return Ok(());
                        }
                        if let Err(error) = self.verify_handoff_flow() {
                            self.smoke_result = Some(Err(error));
                            return Ok(());
                        }
                        if let Err(error) = self.verify_workspace_flow() {
                            self.smoke_result = Some(Err(error));
                            return Ok(());
                        }
                        // 以正式 WebView2 callback 路徑取得 PNG，再由 Rust 驗證及落盤。
                        // 不只檢查 JS 回傳前綴，確保下一次建置能驗收原生跨執行緒橋接。
                        let chart = crate::projects::charts::Chart {
                            transform: None,
                            quality: None,
                            reference_lines: Vec::new(),
                            style: None,
                            kind: "line".into(),
                            title: "10,000 筆量測趨勢／PNG 匯出測試".into(),
                            x_label: "時間 (秒)".into(),
                            y_label: "量測值".into(),
                            x: (0..10_000).map(|i| json!(i)).collect(),
                            series: vec![crate::projects::charts::Series {
                                skip_indices: vec![],
                                name: "量測 A".into(),
                                values: (0..10_000)
                                    .map(|i| {
                                        if (3000..3100).contains(&i) {
                                            None
                                        } else {
                                            Some((i as f64 / 400.0).sin() * 10.0)
                                        }
                                    })
                                    .collect(),
                            }],
                            data_note: String::new(),
                            data_issues: vec![],
                            source: "固定測試資料，包含 3000–3099 筆缺值；不讀使用者資料".into(),
                        };
                        let (reply, response) = mpsc::channel();
                        self.view.export_chart_png(&chart, reply)?;
                        let custom:crate::projects::charts::style::Style=serde_json::from_value(json!({
                            "title":"使用者自訂圖表／上下限與顏色","x_label":"時間（秒）","y_label":"量測值","kind":"area","legend":"bottom_right",
                            "x_min":2000,"x_max":6000,"y_min":-15,"y_max":15,
                            "series":[{"name":"自訂量測名稱","color":"#008800"}],
                            "lines":[{"axis":"y","value":8,"name":"上限","color":"#d62728"},{"axis":"y","value":-8,"name":"下限","color":"#d62728"},{"axis":"x","value":5000,"name":"事件","color":"#5470c6"}]
                        })).map_err(|e|e.to_string())?;
                        let mut custom = custom;
                        custom.transform = Some(serde_json::from_value(json!({
                            "x":{"mode":"index","offset":0,"start":1,"step":1},
                            "y":{"mode":"offset","offset":2,"start":1,"step":1},"drop_empty":true
                        })).map_err(|e|e.to_string())?);
                        custom.validate(&chart)?;
                        let (custom_reply, custom_response) = mpsc::channel();
                        self.view
                            .export_custom_chart_png(&chart, Some(&custom), custom_reply)?;
                        let tx = self.tx.clone();
                        let destination = self.root.join("chart-smoke.png");
                        let mermaid_source = "flowchart TD\n A[\"讀取程式碼\"] --> B{\"資料完整？\"}\n B -->|是| C[\"產生流程圖\"]\n B -->|否| A";
                        let (mermaid_svg_reply, mermaid_svg_response) = mpsc::channel();
                        let (mermaid_png_reply, mermaid_png_response) = mpsc::channel();
                        self.view.render_mermaid(
                            mermaid_source,
                            crate::projects::diagrams::Format::Svg,
                            mermaid_svg_reply,
                        )?;
                        self.view.render_mermaid(
                            mermaid_source,
                            crate::projects::diagrams::Format::Png,
                            mermaid_png_reply,
                        )?;
                        crate::reliability::spawn(move || {
                            let result = (|| -> AppResult<()> {
                                let url = response
                                    .recv_timeout(Duration::from_secs(20))
                                    .map_err(|_| "PNG 自檢逾時。")??;
                                let png = crate::projects::charts::png::decode_url(&url)?;
                                std::fs::write(&destination, png).map_err(|e| e.to_string())?;
                                let custom_url = custom_response
                                    .recv_timeout(Duration::from_secs(20))
                                    .map_err(|_| "自訂PNG自檢逾時。")??;
                                let custom_png =
                                    crate::projects::charts::png::decode_url(&custom_url)?;
                                let folder = destination.parent().ok_or("缺少測試資料夾。")?;
                                std::fs::write(folder.join("chart-custom-smoke.png"), &custom_png)
                                    .map_err(|e| e.to_string())?;
                                let project = crate::projects::Project {
                                    id: "chart-smoke".into(),
                                    name: "圖表自檢".into(),
                                    root: folder.into(),
                                    imports: Default::default(),
                                };
                                let path = crate::projects::files::save_user_chart(
                                    &project,
                                    "ui-smoke",
                                    &custom_png,
                                )?;
                                // 驗證真正 WebView2 awaitPromise 橋接、MMD 中文及三種成果落盤。
                                let svg = crate::projects::diagrams::decode_rendered(
                                    &mermaid_svg_response
                                        .recv_timeout(Duration::from_secs(30))
                                        .map_err(|_| "Mermaid SVG 自檢逾時。")??,
                                    crate::projects::diagrams::Format::Svg,
                                )?;
                                let png = crate::projects::diagrams::decode_rendered(
                                    &mermaid_png_response
                                        .recv_timeout(Duration::from_secs(30))
                                        .map_err(|_| "Mermaid PNG 自檢逾時。")??,
                                    crate::projects::diagrams::Format::Png,
                                )?;
                                for (name, bytes) in [
                                    ("流程.mmd", mermaid_source.as_bytes()),
                                    ("流程.svg", svg.as_slice()),
                                    ("流程.png", png.as_slice()),
                                ] {
                                    let relative = crate::projects::files::diagrams::save(
                                        &project,
                                        &crate::projects::files::conversation_folder_name(
                                            "ui-smoke",
                                        )?,
                                        name,
                                        bytes,
                                    )?;
                                    if std::fs::read(project.root.join(&relative))
                                        .map_err(|e| e.to_string())?
                                        != bytes
                                    {
                                        return Err("Mermaid 自檢讀回不符。".into());
                                    }
                                }
                                std::fs::write(folder.join("mermaid-smoke.svg"), &svg)
                                    .map_err(|e| e.to_string())?;
                                std::fs::write(folder.join("mermaid-smoke.png"), &png)
                                    .map_err(|e| e.to_string())?;
                                std::fs::write(folder.join("chart-manual-save.json"),serde_json::to_vec_pretty(&json!({"result":"PASS","path":path,"bytes":custom_png.len(),"version":env!("CARGO_PKG_VERSION")})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
                            })();
                            let _ = tx.send(Event::SmokePng(result));
                        });
                    }
                }
            }
            Command::Draft { text } => {
                if self.busy != "chat" && text.encode_utf16().count() <= 16_000 {
                    self.draft = text;
                }
            }
            Command::Chat { text, action } => self.begin_chat(text, &action)?,
            Command::RetryChat {
                conversation_id,
                user_index,
                message_count,
                request_id,
            } => {
                self.retry_chat(
                    &conversation_id,
                    user_index,
                    message_count,
                    request_id.as_deref(),
                )?;
            }
            Command::NewChat => {
                if self.busy == "none" && self.work.incoming.is_none() {
                    self.new_chat();
                }
            }
            Command::Behavior {
                hotkey_enabled,
                selection_icon,
                enter_sends,
                quick_actions_fast,
                always_new_chat,
                vnc_enabled,
            } => {
                let previous = self.config.clone();
                self.config.hotkey_enabled = hotkey_enabled;
                self.config.selection_icon = selection_icon;
                self.config.enter_sends = enter_sends;
                self.config.quick_actions_fast = quick_actions_fast;
                self.config.always_new_chat = always_new_chat;
                self.config.vnc_enabled = vnc_enabled;
                if let Err(e) = self.apply_hotkey(self.config.hotkey.clone()) {
                    self.config = previous;
                    let _ = self.apply_hotkey(self.config.hotkey.clone());
                    return Err(e);
                }
                self.selection_popup
                    .enabled
                    .store(selection_icon, Ordering::Relaxed);
                if !selection_icon {
                    self.selection_popup.update(None);
                }
                #[cfg(feature = "vnc")]
                if !vnc_enabled {
                    // 停用即清除記憶體中的機台與密碼；背景搜尋結果也不再套用。
                    self.vnc.disable();
                }
            }
            Command::RenameChat { id, title } => {
                let title = title.trim();
                if title.is_empty()
                    || title.chars().count() > 100
                    || title.chars().any(char::is_control)
                {
                    return Err("標題需為 1 至 100 字，不能包含換行。".into());
                }
                let mut archive = self.archive.clone();
                let c = archive
                    .conversations
                    .iter_mut()
                    .find(|c| c.id == id)
                    .ok_or("找不到對話。")?;
                c.title = title.into();
                c.title_manual = true;
                history::save(&self.root, &archive)?;
                self.archive = archive;
            }
            Command::PinChat { id, pinned } => {
                let mut archive = self.archive.clone();
                archive
                    .conversations
                    .iter_mut()
                    .find(|c| c.id == id)
                    .ok_or("找不到對話。")?
                    .pinned = pinned;
                history::save(&self.root, &archive)?;
                self.archive = archive;
            }
            Command::CollapseSection { section, collapsed } => {
                match section.as_str() {
                    "projects" => self.config.projects_collapsed = collapsed,
                    "recent" => self.config.recent_collapsed = collapsed,
                    _ => return Err("未知側欄區塊。".into()),
                }
                storage::save_config(&self.root, &self.config)?;
            }
            Command::Preferences {
                font_size,
                ui_font,
                interface_scale,
                sidebar_collapsed,
                notification_popups,
                minimize_to_taskbar,
                dark_mode,
            } => {
                let mut config = self.config.clone();
                config.font_size = font_size.clamp(12, 20);
                if let Some(font) = ui_font {
                    config.ui_font = font;
                }
                if let Some(scale) = interface_scale {
                    config.interface_scale = crate::config::interface_scale(scale);
                }
                config.sidebar_collapsed = sidebar_collapsed;
                config.notification_popups = notification_popups;
                if let Some(enabled) = minimize_to_taskbar {
                    config.minimize_to_taskbar = enabled;
                }
                if let Some(dark_mode) = dark_mode {
                    config.dark_mode = dark_mode;
                }
                storage::save_config(&self.root, &config)?;
                self.config = config;
                self.view.set_interface_scale(self.config.interface_scale)?;
                apply_window_theme(self.window, self.config.dark_mode);
            }
            Command::ChartPalette { colors } => {
                if !crate::config::valid_chart_palette(&colors) {
                    return Err("圖表預設配色需為八個 #RRGGBB 色碼。".into());
                }
                // 先保存成功再套用，避免寫入失敗時畫面宣稱設定已保留。
                let mut config = self.config.clone();
                config.chart_palette = colors;
                storage::save_config(&self.root, &config)?;
                self.config = config;
                self.toast("圖表預設配色已儲存");
            }
            Command::Hotkey { value } => {
                if let Err(message) = self.apply_hotkey(value) {
                    self.view
                        .post(&json!({"type":"hotkey_error","message":message}))?;
                    return Err(message);
                }
                self.view
                    .post(&json!({"type":"hotkey_saved","value":self.config.hotkey}))?;
                self.toast("快捷鍵已更新");
            }
            Command::Refresh => self.refresh_services(),
            Command::Exit => unsafe {
                PostMessageW(self.window, WM_CLOSE, 1, 0);
            },
            Command::Download => {
                if self.update_busy {
                    return Ok(());
                }
                if self
                    .update_ready
                    .as_ref()
                    .is_some_and(|ready| ready.artifact.kind == "exe")
                {
                    self.apply_ready_update()?;
                } else if self.update_ready.is_some() {
                    if self.confirm_update()? {
                        self.apply_ready_update()?;
                    }
                } else {
                    self.start_update_download()?;
                }
            }
            Command::OpenLink { url } => open_browser(&url)?,
            Command::Copy { text } => {
                if text.len() > 1_048_576 {
                    return Err("內容過長，無法複製。".into());
                }
                selection::copy_text(self.window, &text)?;
                self.toast("已複製");
            }
            Command::Model { id } => {
                if self.busy == "none"
                    && self
                        .models
                        .as_ref()
                        .is_some_and(|c| c.models.iter().any(|m| m.id == id))
                {
                    if self.mail_flow.phase != "idle" {
                        return Err("請先停止郵件流程再切換模型。".into());
                    }
                    self.config.model = id;
                    self.work.status = "正在重新確認此模型的附件規則…".into();
                    self.refresh_work_capabilities();
                    storage::save_config(&self.root, &self.config)?;
                }
            }
            Command::SelectChat { id } => {
                if self.busy == "none" && self.work.incoming.is_none() {
                    self.preserve_draft()?;
                    let c = self
                        .archive
                        .conversations
                        .iter()
                        .find(|c| c.id == id)
                        .ok_or("找不到對話。")?;
                    self.messages = c.messages.clone();
                    let draft = c.draft.clone();
                    self.active_id = Some(id);
                    self.set_draft(draft);
                }
            }
            Command::DeleteChat { id } => {
                if self.projects.active(Some(&id)) {
                    return Err("請先停止此專案任務再刪除對話。".into());
                }
                if self.mail_flow.phase != "idle"
                    && self.mail_flow.conversation.as_ref() == Some(&id)
                {
                    return Err("請先停止郵件流程，再刪除對話。".into());
                }
                if self.work.store.pending(Some(&id))
                    || self.work.incoming.is_some()
                    || self
                        .work
                        .store
                        .attachments
                        .iter()
                        .any(|a| a.conversation_id == id && !a.sent && !a.removed)
                {
                    return Err(
                        "此對話仍有工作或草稿附件，請先完成／取消工作並移除草稿附件。".into(),
                    );
                }
                if self.busy == "none" {
                    let mut archive = self.archive.clone();
                    archive.remove(&id)?;
                    history::save(&self.root, &archive)?;
                    self.archive = archive;
                    self.work.store.tasks.retain(|t| t.conversation_id != id);
                    self.work
                        .store
                        .attachments
                        .retain(|a| a.conversation_id != id);
                    self.work.store.conversations.remove(&id);
                    if !self.work.store.principal_id.is_empty() {
                        self.work.store.save(&self.root, &self.config)?;
                    }
                    if self.active_id.as_ref() == Some(&id) {
                        self.new_chat();
                    }
                }
            }
            Command::Login => {
                if self.busy != "none" {
                    return Err("請先完成目前操作。".into());
                }
                self.busy = "login";
                self.grant = None;
                self.login_operation += 1;
                self.cancelled = Arc::new(AtomicBool::new(false));
                self.status = "正在申請一次性登入碼…".into();
                self.error = false;
                let (config, tx, id) = (self.config.clone(), self.tx.clone(), self.login_operation);
                crate::reliability::spawn(move || {
                    let _ = tx.send(Event::Grant(id, auth::request_device(&config)));
                });
            }
            Command::CancelLogin => {
                if self.busy != "login" {
                    return Ok(());
                }
                self.cancelled.store(true, Ordering::Relaxed);
                self.login_operation += 1;
                self.busy = "none";
                self.grant = None;
                self.status = "已取消登入".into();
            }
            Command::ReopenLogin => {
                if let Some(grant) = &self.grant {
                    open_browser(self.config.verification_url(grant.browser_url())?.as_str())?;
                }
            }
            Command::Logout => {
                if self.busy == "none" {
                    self.logout()?;
                    self.status = "已登出".into();
                    self.refresh_services();
                }
            }
            Command::AllEvents { dismiss } => {
                let mut inbox = self.inbox.clone();
                let ids = inbox.mark_all(dismiss);
                notifications::save(&self.root, &inbox)?;
                self.inbox = inbox;
                self.notification_status = if dismiss {
                    "已清除本機通知"
                } else {
                    "已在本機全部標為已讀"
                }
                .into();
                if let Some(session) = self.session.clone() {
                    let (config, tx, generation) =
                        (self.config.clone(), self.tx.clone(), self.generation);
                    crate::reliability::spawn(move || {
                        let mut failures = 0;
                        for id in ids {
                            if notifications::mark_read(&config, &session, &id).is_err() {
                                failures += 1;
                            }
                        }
                        let _ = tx.send(Event::AllRead(generation, failures));
                    });
                }
            }
            Command::NotificationsLeft => {
                if self.logged_in() {
                    self.command(Command::AllEvents { dismiss: false })?;
                    // 全站若正在讀取／標記單則通知，排隊到該操作完成後才送 ReadAll。
                    self.site.read_all_pending = true;
                    self.site_tick();
                }
            }
            Command::RefreshEvents => {
                self.fetch_events();
                self.fetch_site(true);
            }
            Command::ReadEvent { id } => {
                let event = self
                    .inbox
                    .events
                    .iter_mut()
                    .find(|e| e.id == id)
                    .ok_or("找不到通知。")?;
                if event.kind.starts_with("project.") {
                    event.read_at = Some(notifications::now_text());
                    notifications::save(&self.root, &self.inbox)?;
                    return Ok(());
                }
                let (config, session, tx, generation) = (
                    self.config.clone(),
                    self.session.clone().ok_or("請先登入。")?,
                    self.tx.clone(),
                    self.generation,
                );
                crate::reliability::spawn(move || {
                    let result = notifications::mark_read(&config, &session, &id);
                    let _ = tx.send(Event::Read(generation, id, result));
                });
            }
            Command::ReadMail => self.read_mail(false)?,
            Command::ReadMailBody => self.read_mail(true)?,
            Command::OmitMailBody => {
                if let Some(mail) = &mut self.mail {
                    mail.body = None;
                }
            }
            Command::AnalyzeMail => {
                if !self.can_send() {
                    return Err("請先登入並選擇可用模型。".into());
                }
                if !self
                    .work
                    .caps
                    .as_ref()
                    .is_some_and(|c| c.supports("background"))
                {
                    return Err("Outlook 判讀需要網站支援背景處理。".into());
                }
                self.work.mode = "background".into();
                let subject = self.mail.as_ref().ok_or("請先選取郵件。")?.subject.clone();
                let prompt = outlook::analysis_prompt(self.mail.as_ref().ok_or("請先選取郵件。")?)?;
                self.new_chat();
                self.begin_chat(prompt, "mail")?;
                if let Some(c) = self
                    .archive
                    .conversations
                    .iter_mut()
                    .find(|c| Some(&c.id) == self.active_id.as_ref())
                {
                    c.title = format!("Outlook：{}", subject.chars().take(80).collect::<String>());
                }
                history::save(&self.root, &self.archive)?;
            }
        }
        Ok(())
    }
}
