//! 主視窗的session操作；由父模組保留共用狀態與既有介面。
use super::*;

impl App {
    pub(super) fn refresh_services(&mut self) {
        self.refresh_work_capabilities();
        if self.services_loading || self.smoke {
            return;
        }
        self.services_loading = true;
        self.last_services = Instant::now();
        let (config, session, tx, generation) = (
            self.config.clone(),
            self.session.clone(),
            self.tx.clone(),
            self.generation,
        );
        crate::reliability::spawn(move || {
            let version = service::fetch_version(&config);
            let models = service::fetch_models(&config, session.as_ref());
            let _ = tx.send(Event::Services(generation, version, models));
        });
    }
    pub(super) fn start_notifications(&mut self) {
        if self.versions.blocked() {
            return;
        }
        self.start_site();
        self.socket_cancel.store(true, Ordering::Relaxed);
        self.socket_cancel = Arc::new(AtomicBool::new(false));
        let Some(session) = self.session.clone().filter(|s| s.valid_for(&self.config)) else {
            return;
        };
        self.inbox = notifications::load(&self.root, &session)
            .unwrap_or_else(|_| Inbox::for_session(&session));
        self.fetch_events();
        let (config, tx, cancel, generation) = (
            self.config.clone(),
            self.tx.clone(),
            self.socket_cancel.clone(),
            self.generation,
        );
        crate::reliability::spawn(move || {
            let Ok(url) = config.endpoint(notifications::SOCKET_PATH) else {
                return;
            };
            let mut backoff = 5;
            while !cancel.load(Ordering::Relaxed) && session.valid_for(&config) {
                let _ = transport::watch_notifications(
                    &url,
                    &session.access_token,
                    &cancel,
                    |connected| {
                        let _ = tx.send(Event::Socket(generation, connected));
                    },
                );
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                // 即時通道失敗仍定期補查；退避避免持續請求尚未部署的路由。
                for _ in 0..backoff * 10 {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                backoff = (backoff * 2).min(60);
            }
        });
    }
    pub(super) fn fetch_events(&mut self) {
        if !self.logged_in() || self.smoke {
            return;
        }
        if self.notifications_loading {
            self.notifications_pending = true;
            return;
        }
        self.notifications_loading = true;
        self.last_events = Instant::now();
        let (config, session, cursor, tx, generation) = (
            self.config.clone(),
            self.session.clone(),
            self.inbox.cursor.clone(),
            self.tx.clone(),
            self.generation,
        );
        if let Some(session) = session {
            crate::reliability::spawn(move || {
                let result = notifications::fetch_page(&config, &session, cursor.as_deref());
                let _ = tx.send(Event::Events(generation, result));
            });
        }
    }
    pub(super) fn logout(&mut self) -> AppResult<()> {
        self.schedules.cancel();
        self.document_export.cancel();
        self.workspace_import = None;
        self.handoff = None;
        self.screenshot_command(screenshot::ScreenshotCommand::Cancel)?;
        self.projects.cancel();
        #[cfg(feature = "vnc")]
        self.vnc.disable();
        self.mail_flow.cancel.store(true, Ordering::Relaxed);
        self.selection_popup.enabled.store(false, Ordering::Relaxed);
        self.selection_popup.update(None);
        selection::unregister(self.window);
        self.hotkey = None;
        let _ = self.stop_recording();
        self.site = site::SiteRuntime::default();
        self.mail_flow = mail_batch::MailRuntime::default();
        self.cancelled.store(true, Ordering::Relaxed);
        self.socket_cancel.store(true, Ordering::Relaxed);
        self.generation += 1;
        self.login_operation += 1;
        self.session = None;
        self.work = work::WorkRuntime::default();
        self.models = None;
        self.grant = None;
        self.services_loading = false;
        self.notifications_loading = false;
        self.notifications_pending = false;
        self.inbox = Inbox::default();
        self.mail = None;
        self.mail_busy = false;
        self.busy = "none";
        self.new_chat();
        self.notification_status = "登入後同步網站通知".into();
        // 歷史屬於 Windows 使用者；下一次登入從空白對話開始，避免隱性沿用舊內容。
        if !self.demo {
            storage::clear_session(&self.root)?;
        }
        Ok(())
    }
    pub(super) fn begin_chat(&mut self, text: String, action: &str) -> AppResult<()> {
        if !self.can_send() {
            return Err("請確認登入、可用模型與版本狀態後再送出。".into());
        }
        if text.trim().is_empty() && self.work.store.drafts(self.active_id.as_deref()).is_empty() {
            return Err("請輸入文字或加入附件。".into());
        }
        let text = if text.trim().is_empty() {
            "請分析附件內容。".to_string()
        } else {
            text
        };
        let content = match action {
            "send" | "mail" => text.clone(),
            "translate" => {
                format!("請將以下文字翻譯為繁體中文；若原文為中文則翻譯成英文：\n\n{text}")
            }
            "summarize" => format!("請用繁體中文整理以下內容的重點：\n\n{text}"),
            "polish" => format!("請保留原意並潤飾以下文字：\n\n{text}"),
            "explain" => format!("請用繁體中文、容易理解的方式解釋以下內容與本次附件。先簡述核心意思，再說明重要概念，必要時用例子輔助。論文請區分研究問題、方法、結果與限制；專有名詞請說明定義及使用情境；程式碼請說明用途、流程、輸入輸出與重要注意事項，不執行或修改程式。只依提供的內容說明，缺少資料或不確定處請明示，不捏造背景。若使用者指定讀者程度或重點，優先遵循：\n\n{text}"),
            _ => return Err("不支援的聊天操作。".into()),
        };
        let mut messages = self.messages.clone();
        messages.push(Message::user(&content));
        let project_chat = self
            .active_id
            .as_deref()
            .and_then(|id| self.projects.store.project_for(id))
            .is_some();
        if project_chat && messages.len() >= 1000 {
            return Err("此專案對話已達 500 輪，請新增專案對話。".into());
        }
        if self
            .active_id
            .as_deref()
            .and_then(|id| self.projects.store.project_for(id))
            .is_some()
        {
            if action == "mail" {
                return Err("請在 Outlook 助理或一般對話分析郵件，專案對話只使用專案資料。".into());
            }
            self.begin_project_chat(messages)
        } else {
            self.begin_work_chat(messages, action)
        }
    }
    pub(super) fn read_mail(&mut self, body: bool) -> AppResult<()> {
        if self.mail_busy {
            return Ok(());
        }
        let expected = if body {
            Some(
                self.mail
                    .as_ref()
                    .ok_or("請先讀取郵件基本資訊。")?
                    .entry_id
                    .clone(),
            )
        } else {
            None
        };
        self.mail_busy = true;
        if !body {
            self.mail = None;
        }
        let (tx, generation, demo) = (self.tx.clone(), self.generation, self.demo);
        crate::reliability::spawn(move || {
            let result = if demo {
                Ok(outlook::demo_mail(body))
            } else {
                outlook::read_selected(expected.as_deref(), body)
            };
            let _ = tx.send(Event::Mail(generation, result));
        });
        Ok(())
    }
}
