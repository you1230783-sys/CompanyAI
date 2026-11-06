//! 主視窗的events操作；由父模組保留共用狀態與既有介面。
use super::*;

impl App {
    pub(super) fn event(&mut self, event: Event) -> AppResult<()> {
        if self.versions.blocked()
            && !matches!(
                event,
                Event::Project(..)
                    | Event::DocumentExport(..)
                    | Event::Services(..)
                    | Event::UpdateReady(..)
                    | Event::Grant(..)
                    | Event::Login(..)
            )
        {
            // 停止以舊版繼續串接自動郵件／附件流程，已提交的伺服器任務留給新版恢復。
            return Ok(());
        }
        match event {
            Event::DocumentExport(id, generation, result) => {
                self.export_event(id, generation, result)?
            }
            Event::Screenshot(id, result) => self.screenshot_event(id, result)?,
            Event::Site(generation, event) if generation == self.generation => {
                self.site_event(event)?
            }
            Event::MailBatch(generation, operation, event)
                if generation == self.generation && operation == self.mail_flow.operation =>
            {
                self.mail_batch_event(event)?
            }
            Event::Work(generation, event) if generation == self.generation => {
                self.work_event(event)?
            }
            #[cfg(feature = "vnc")]
            Event::VncSearch(id, result) => self.vnc_search_result(id, result)?,
            #[cfg(feature = "vnc")]
            Event::VncSync(id, result) => self.vnc_sync_result(id, result),
            Event::Services(generation, version, models) if generation == self.generation => {
                self.services_loading = false;
                self.version_status = match self.versions.apply(version) {
                    Ok(()) => {
                        let info = self.versions.known.as_ref().ok_or("版本狀態錯誤。")?;
                        if info.required() {
                            "此版本已停止支援，請下載更新".into()
                        } else if info.available() {
                            format!("可更新至 {}", info.latest_version)
                        } else {
                            "目前為最新版本".into()
                        }
                    }
                    Err(error) => {
                        if self.versions.blocked() {
                            format!("請更新至已確認的最低版本；{error}")
                        } else {
                            format!("暫時無法檢查版本，允許使用；{error}")
                        }
                    }
                };
                match models {
                    Ok(c) => {
                        self.config.model =
                            c.models[c.selected_index(&self.config.model)].id.clone();
                        self.models = Some(c);
                        if self.work.capability_model != self.config.model {
                            self.refresh_work_capabilities();
                        }
                    }
                    Err(e) => {
                        self.models = None;
                        self.fail(e);
                    }
                }
                if self.update_ready.as_ref().is_some_and(|ready| {
                    self.versions.known.as_ref().is_none_or(|info| {
                        service::version_number(&ready.artifact.version).ok()
                            < service::version_number(&info.latest_version).ok()
                    })
                }) {
                    self.update_ready = None;
                    self.update_status = "網站已發布不同版本，請重新同意下載。".into();
                }
                if self.versions.blocked() {
                    self.versions.save(&self.root)?;
                    self.mail_flow.cancel.store(true, Ordering::Relaxed);
                    // 登入是未登入時唯一可用入口；背景版本檢查不得中止正在進行的授權。
                    if self.busy != "login" {
                        self.cancelled.store(true, Ordering::Relaxed);
                    }
                    self.socket_cancel.store(true, Ordering::Relaxed);
                    self.selection_popup.update(None);
                    self.selection_popup.enabled.store(false, Ordering::Relaxed);
                    self.fail(self.version_status.clone());
                    self.toast("目前版本需更新，請由設定下載新版");
                }
            }
            Event::Grant(id, result) if id == self.login_operation => match result {
                Ok(grant) => {
                    let browser = self.config.verification_url(grant.browser_url())?;
                    self.grant = Some(grant.clone());
                    let (config, tx, cancel) =
                        (self.config.clone(), self.tx.clone(), self.cancelled.clone());
                    crate::reliability::spawn(move || {
                        let _ = tx.send(Event::Login(
                            id,
                            auth::wait_for_login(&config, &grant, &cancel),
                        ));
                    });
                    self.status = "請在瀏覽器完成登入授權".into();
                    open_browser(browser.as_str())?;
                }
                Err(e) => {
                    self.busy = "none";
                    return Err(e);
                }
            },
            Event::Login(id, result) if id == self.login_operation => {
                self.busy = "none";
                self.grant = None;
                let session = result?;
                if !self.demo {
                    storage::save_session(&self.root, &session)?;
                }
                self.generation += 1;
                self.session = Some(session);
                if let Err(error) = self.apply_hotkey(self.config.hotkey.clone()) {
                    self.toast(&error);
                }
                self.selection_popup
                    .enabled
                    .store(self.config.selection_icon && !self.smoke, Ordering::Relaxed);
                self.work = work::WorkRuntime::default();
                self.services_loading = false;
                self.notifications_loading = false;
                self.new_chat();
                self.status = "登入成功".into();
                self.refresh_services();
                self.start_notifications();
            }
            Event::Project(event) => self.project_event(event)?,
            Event::SmokePng(result) => {
                if self.smoke {
                    self.smoke_result = Some(result);
                }
            }
            Event::UpdateReady(result) => {
                self.update_busy = false;
                match result {
                    Ok(plan) => {
                        if self.versions.known.as_ref().is_none_or(|info| {
                            service::version_number(&plan.artifact.version).ok()
                                < service::version_number(&info.latest_version).ok()
                        }) {
                            self.update_status = "下載期間版本資訊已變更，請重新下載。".into();
                            return Ok(());
                        }
                        // 清單可能比版本 API 更新；下載期間發現更高的簽署版本亦可接受。
                        if let Some(info) = &mut self.versions.known {
                            info.latest_version = plan.artifact.version.clone();
                        }
                        self.update_status = if plan.artifact.kind == "exe" {
                            format!("{} 獨立 EXE 已下載並驗證。請開啟下載資料夾，從系統托盤離開 LM_AI 後手動更換檔案；個人資料會保留。", plan.artifact.version)
                        } else {
                            format!(
                                "{} 安裝包已下載並驗證。請點擊更新入口繼續；退出不會自動安裝。",
                                plan.artifact.version
                            )
                        };
                        let auto_install = plan.artifact.kind == "nsis"
                            && self.update_consent.as_deref()
                                == Some(plan.artifact.version.as_str());
                        self.update_ready = Some(plan);
                        if auto_install {
                            self.update_status = "更新已驗證，正在保存並重新啟動 LM_AI…".into();
                            self.apply_ready_update()?;
                        } else {
                            self.update_consent = None;
                            self.toast(&self.update_status.clone());
                        }
                    }
                    Err(e) => {
                        self.update_status = e;
                    }
                }
            }
            Event::SelectionRect(rect) => {
                self.selection_popup.update(
                    if self.logged_in() && self.busy == "none" && !self.versions.blocked() {
                        rect
                    } else {
                        None
                    },
                );
            }
            // 登出前啟動的擷取不得寫入下一次登入的草稿。
            Event::Capture(generation, result)
                if generation == self.generation && self.logged_in() =>
            {
                self.busy = "none";
                let text = result?;
                if self.config.always_new_chat {
                    self.new_chat();
                }
                let joined = if self.draft.trim().is_empty() {
                    text
                } else {
                    format!("{}\n\n{text}", self.draft)
                };
                if joined.encode_utf16().count() > 16_000 {
                    return Err("目前草稿加上選字太長，原草稿已保留。".into());
                }
                self.set_draft(joined);
                self.focus_draft = true;
                self.status = "剪貼簿文字已帶入，確認後再送出".into();
                // 先將草稿送到介面，前景切換成功與否都不影響已讀取的文字。
                self.publish();
                if show_window(self.window) {
                    self.view.focus();
                } else {
                    self.status = "文字已帶入；請點工作列的 LM_AI 查看草稿".into();
                }
            }
            Event::Mail(generation, result) if generation == self.generation => {
                self.mail_busy = false;
                self.mail = Some(result?);
                self.status = "已讀取郵件預覽，尚未傳給 AI".into();
            }
            Event::Socket(generation, connected) if generation == self.generation => {
                self.poll_work(true);
                self.fetch_site(true);
                if connected {
                    self.notification_status = "即時通知已連線".into();
                }
                self.fetch_events();
            }
            Event::AllRead(generation, failures) if generation == self.generation => {
                self.notification_status = if failures == 0 {
                    "本機已讀／清除完成，已同步網站已讀狀態".into()
                } else {
                    format!("本機操作已完成；{failures} 則網站已讀同步失敗，可再次按全部已讀重試。")
                };
            }
            Event::Events(generation, result) if generation == self.generation => {
                self.notifications_loading = false;
                match result {
                    Ok(mut page) => {
                        // 使用者正在 App 時靜默接收，不加未讀標示或系統氣泡。
                        if self.is_foreground() {
                            for event in &mut page.events {
                                event.read_at = Some(notifications::now_text());
                            }
                        }
                        let more = page.has_more;
                        let mut inbox = self.inbox.clone();
                        // 任務完成提示由結果落盤後統一發出，避免 REST 事件與任務輪詢各提示一次。
                        let added = page
                            .events
                            .iter()
                            .filter(|e| {
                                e.visible_ai()
                                    && !crate::projects::events::internal(
                                        &self.root,
                                        e.resource_id.as_deref(),
                                    )
                                    && !e.kind.starts_with("chat.")
                                    && !e.kind.starts_with("attachment.")
                                    && !self.inbox.events.iter().any(|old| old.id == e.id)
                                    && e.read_at.is_none()
                                    && !e.expired()
                            })
                            .count();
                        inbox.merge(page)?;
                        notifications::save(&self.root, &inbox)?;
                        self.inbox = inbox;
                        self.notification_status = "通知已同步；即時通道與定期補查並行".into();
                        if added > 0 && self.config.notification_popups && !self.is_foreground() {
                            self.work.tray_restore_only = false;
                            self.work.task_balloon = false;
                            self.desktop_notice(&format!("收到 {added} 則新通知，點一下查看。"));
                        }
                        if more || self.notifications_pending {
                            self.notifications_pending = false;
                            self.fetch_events();
                        }
                    }
                    Err(e) => {
                        self.notifications_pending = false;
                        self.notification_status = e;
                    }
                }
            }
            Event::Read(generation, id, result) if generation == self.generation => {
                result?;
                let mut inbox = self.inbox.clone();
                if let Some(event) = inbox.events.iter_mut().find(|e| e.id == id) {
                    event.read_at = Some(notifications::now_text());
                }
                notifications::save(&self.root, &inbox)?;
                self.inbox = inbox;
            }
            _ => {} // 登出或重新登入前的工作，不得更新新登入工作階段。
        }
        Ok(())
    }
    pub(super) fn tick(&mut self) {
        if crate::reliability::faulted() {
            self.fault_tick();
            return;
        }
        self.tick_pet();
        self.tick_reminders();
        self.tick_schedules();
        let mut changed = false;
        // 先處理到期，再接受 UI 命令或背景結果，不能因本機工作忙碌而繼續使用過期授權。
        if !self.smoke && self.session.is_some() && !self.logged_in() && self.busy != "login" {
            if let Err(error) = self.batch_login_expired() {
                self.fail(error);
            }
            let _ = self.logout();
            self.fail("登入已到期，請重新登入。".into());
            changed = true;
        }
        if !self.smoke {
            match self.batch_tick() {
                Ok(updated) => changed |= updated,
                Err(error) => {
                    self.fail(error);
                    changed = true;
                }
            }
        }
        if self.logged_in() && !self.versions.blocked() {
            changed |= self.site_tick();
            if !self.smoke {
                match self.tick_recovered_retries() {
                    Ok(updated) => changed |= updated,
                    Err(error) => {
                        self.fail(error);
                        changed = true;
                    }
                }
            }
            match self.mail_batch_tick() {
                Ok(updated) => changed |= updated,
                Err(e) => {
                    self.mail_flow.phase = "idle";
                    self.mail_flow.status =
                        format!("自動流程已暫停：{e} 附件保留，可在對話手動送出。");
                    changed = true;
                }
            }
        }
        if self.suppress_hotkey && crate::hotkey::pressed_modifiers() == 0 {
            self.suppress_hotkey = false;
        }
        if self
            .recording_since
            .is_some_and(|start| start.elapsed() > Duration::from_secs(15))
        {
            if let Err(e) = self.stop_recording() {
                self.fail(e);
            }
            self.toast("錄製已逾時，原快捷鍵維持不變");
            changed = true;
        }
        while let Ok(message) = self.commands.try_recv() {
            changed = true;
            match serde_json::from_str::<Command>(&message) {
                Ok(command) => {
                    if let Err(e) = self.command(command) {
                        self.fail(e);
                    }
                }
                Err(_) => self.fail("介面操作格式不正確。".into()),
            }
        }
        while let Ok(event) = self.rx.try_recv() {
            changed = true;
            if let Err(e) = self.event(event) {
                self.fail(e);
            }
        }
        if !self.smoke {
            if self.logged_in() && !self.versions.blocked() {
                self.poll_work(false);
            }
            if self.last_events.elapsed() > Duration::from_secs(60) {
                self.last_events = Instant::now();
                if !self.versions.blocked() {
                    self.fetch_events();
                }
                changed = true;
            }
            if self.last_services.elapsed() > Duration::from_secs(300) {
                self.refresh_services();
                changed = true;
            }
        }
        // 自我檢查開始後由前端管理虛構資料，避免主程式的空白狀態覆蓋測試畫面。
        if !self.smoke {
            self.project_attention_tick();
        }
        if changed && !self.smoke {
            self.publish();
        }
    }
}
