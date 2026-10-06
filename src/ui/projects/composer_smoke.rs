//! EXE 自檢中的原生排程回歸。只用獨立暫存資料與 loopback 測試服務。
//!
//! 舊任務終態由測試送入；接續則走正式控制器、DPAPI 與真實 runner 執行緒，
//! 隨即取消並等待 Finished。此處不把模擬終態當成公司模型或 Office 的整合驗收。
use super::{composer::ComposeMode, *};

fn verify(ok: bool, case: &str) -> AppResult<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("原生傳送排程自檢失敗：{case}"))
    }
}

impl App {
    /// 只有 --self-check 建立的隱藏視窗會呼叫，不在正式帳號或資料目錄執行。
    pub(in crate::ui) fn verify_project_composer(&mut self) -> AppResult<()> {
        if !self.smoke {
            return Err("排程自檢僅供 self-check 模式使用。".into());
        }
        let original_root = self.root.clone();
        let original_config = self.config.clone();
        self.root = original_root.join(format!("composer-{}", crate::jobs::new_id()?));
        let result = self.composer_smoke_cases();
        self.projects.cancel();
        // PNG 自檢還會繼續跑事件迴圈；先清除測試登入與草稿，避免排入背景同步，
        // 或讓 App::drop 把測試歷史寫回共用的 UI 自檢目錄。
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
        let report = json!({
            "version":env!("CARGO_PKG_VERSION"), "result":"PASS", "cases":cases,
            "prior_worker_completion":"controlled event",
            "next_worker":"real runner, cancelled and awaited",
            "company_service_tested":false
        });
        std::fs::write(
            self.root.join("composer-verification.json"),
            serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    fn composer_smoke_cases(&mut self) -> AppResult<Vec<&'static str>> {
        let server = crate::demo::DemoServer::start()?;
        self.config = server.config();
        self.config.model = "fast".into();
        self.config.notification_popups = false;
        self.session = Some(crate::storage::Session {
            access_token: "composer-fixture-not-a-real-token".into(),
            expires_at: crate::unix_now() + 3600,
            binding: self.config.binding()?,
        });
        self.versions = Default::default();
        self.history_error = None;
        self.busy = "none";
        self.models = Some(crate::service::ModelCatalog {
            models: vec![crate::service::ModelOption {
                id: "fast".into(),
                label: "自檢".into(),
                description: String::new(),
            }],
            default_model: Some("fast".into()),
        });
        let mut cases = vec![];

        let (chat, old) = self.composer_fixture()?;
        let instruction = self.compose_project_message(
            &chat,
            &old,
            ComposeMode::NextTurn,
            None,
            "請先確認 10:03 的上料事件。",
        )?;
        verify(
            self.supplement_inbox(&chat, &old)?
                .entries()?
                .iter()
                .any(|e| e.id == instruction)
                && self.composer_queued(&chat).is_none()
                && self
                    .compose_project_message(&chat, "stale", ComposeMode::NextTurn, None, "過期")
                    .is_err(),
            "下一輪補充與過期 run 拒絕",
        )?;
        cases.push("native supplement persistence and stale run rejection");

        let queued = self.compose_project_message(
            &chat,
            &old,
            ComposeMode::AfterTask,
            None,
            "完成後整理週報",
        )?;
        let saved = history::load(&self.root)?;
        verify(
            saved
                .conversations
                .iter()
                .find(|c| c.id == chat)
                .and_then(|c| c.project_queued.as_ref())
                .is_some_and(|q| q.id == queued)
                && self.start_queued_project(&chat, &queued).is_err()
                && self
                    .compose_project_message(&chat, &old, ComposeMode::AfterTask, None, "重複")
                    .is_err(),
            "先落盤、單筆排程、舊 worker 尚未結束不得啟動",
        )?;
        cases.push("durable queue, duplicate rejection and no overlapping worker");

        // 保留另一對話的一則問題，能抓到誤用目前畫面觸發標題工作的回歸。
        let other = self.archive.insert(vec![Message::user("另一個對話")])?;
        self.active_id = Some(other.clone());
        self.messages = vec![Message::user("另一個對話")];
        self.draft = "另一個尚未送出的草稿".into();
        self.project_event(ProjectEvent::Finished(
            "stale".into(),
            chat.clone(),
            Ok("過期".into()),
        ))?;
        verify(
            self.projects.running.as_ref().is_some_and(|r| r.id == old),
            "忽略過期 Finished",
        )?;
        self.project_event(ProjectEvent::Progress(
            old.clone(),
            "AI 進度筆記：已確認上料事件。".into(),
        ))?;
        self.project_event(ProjectEvent::Finished(
            old,
            chat.clone(),
            Ok("原任務完成".into()),
        ))?;
        verify(
            self.projects
                .running
                .as_ref()
                .is_some_and(|r| r.id == queued && r.conversation == chat)
                && self.composer_queued(&chat).is_none()
                && self.active_id.as_ref() == Some(&other)
                && self.draft == "另一個尚未送出的草稿"
                && self.messages[0].content == "另一個對話"
                && !self.work.store.tasks.iter().any(|t| t.title_generation),
            "Finished 接續原對話，不影響其他草稿或產生錯誤標題",
        )?;
        let messages = &self
            .archive
            .conversations
            .iter()
            .find(|c| c.id == chat)
            .ok_or("缺少原對話")?
            .messages;
        verify(
            messages.iter().any(|m| {
                m.content == "原任務完成"
                    && m.project_activity
                        .iter()
                        .any(|s| s.starts_with("AI 進度筆記："))
            }),
            "進度紀錄與最終正文分開保存",
        )?;
        verify(
            messages.last().is_some_and(|m| {
                m.request_id.as_ref() == Some(&queued) && m.content == "完成後整理週報"
            }),
            "排程原子轉為使用者訊息",
        )?;
        self.await_composer_worker()?;
        cases.push("completion starts original conversation, preserves other draft and archives progress separately");

        let (chat, old) = self.composer_fixture()?;
        let queued = self.compose_project_message(
            &chat,
            &old,
            ComposeMode::Interrupt,
            None,
            "停止後的新任務",
        )?;
        verify(
            self.projects
                .running
                .as_ref()
                .is_some_and(|r| r.id == old && r.cancel.load(Ordering::Relaxed)),
            "先停止原 worker",
        )?;
        self.project_event(ProjectEvent::Finished(
            old,
            chat.clone(),
            Err("已停止".into()),
        ))?;
        verify(
            self.projects
                .running
                .as_ref()
                .is_some_and(|r| r.id == queued),
            "停止終態後才啟動下一任務",
        )?;
        self.await_composer_worker()?;
        cases.push("interrupt persists first and starts next worker only after Finished");

        let (chat, old) = self.composer_fixture()?;
        let queued =
            self.compose_project_message(&chat, &old, ComposeMode::AfterTask, None, "取消排程")?;
        verify(
            self.cancel_queued_project(&chat, "stale").is_err(),
            "拒絕取消錯誤排程",
        )?;
        self.cancel_queued_project(&chat, &queued)?;
        self.project_event(ProjectEvent::Finished(old, chat.clone(), Ok("完成".into())))?;
        verify(
            self.projects.running.is_none() && self.composer_queued(&chat).is_none(),
            "取消後不接續",
        )?;
        cases.push("queue cancellation does not start a new worker");

        let (chat, old) = self.composer_fixture()?;
        let queued = self.compose_project_message(
            &chat,
            &old,
            ComposeMode::Interrupt,
            None,
            "手動停止保留",
        )?;
        self.project_command(ProjectCommand::Stop)?;
        self.project_event(ProjectEvent::Finished(
            old,
            chat.clone(),
            Err("已停止".into()),
        ))?;
        verify(
            self.projects.running.is_none()
                && self.composer_queued(&chat).is_some_and(|q| !q.auto_start),
            "手動停止保留而不自動送出",
        )?;
        self.config.model = "changed-model".into();
        verify(
            self.start_queued_project(&chat, &queued).is_err(),
            "模型綁定",
        )?;
        self.config.model = "fast".into();
        self.work.store.principal_id = "changed-account".into();
        verify(
            self.start_queued_project(&chat, &queued).is_err(),
            "帳號綁定",
        )?;
        self.work.store.principal_id = "composer-account".into();
        let original_project = self
            .projects
            .store
            .conversations
            .remove(&chat)
            .ok_or("缺少專案")?;
        verify(
            self.start_queued_project(&chat, &queued).is_err(),
            "專案綁定",
        )?;
        self.projects
            .store
            .conversations
            .insert(chat.clone(), original_project);
        self.start_queued_project(&chat, &queued)?;
        self.await_composer_worker()?;
        cases.push(
            "manual stop holds queue; account, model and project bindings; manual retry succeeds",
        );

        for result in [
            Err("受控失敗".into()),
            Ok("需要你的補充：請指定事件".into()),
        ] {
            let (chat, old) = self.composer_fixture()?;
            self.compose_project_message(&chat, &old, ComposeMode::AfterTask, None, "保留文字")?;
            self.project_event(ProjectEvent::Finished(old, chat.clone(), result))?;
            verify(
                self.projects.running.is_none()
                    && self.composer_queued(&chat).is_some_and(|q| !q.auto_start),
                "失敗／等待補充不自動接續",
            )?;
        }
        cases.push("failure and waiting-for-user preserve pending text without starting");

        let (chat, old) = self.composer_fixture()?;
        self.compose_project_message(&chat, &old, ComposeMode::AfterTask, None, "重啟保留")?;
        self.projects.running = None;
        self.archive = history::load(&self.root)?;
        self.recover_project_history()?;
        verify(
            self.projects.running.is_none()
                && self
                    .composer_queued(&chat)
                    .is_some_and(|q| !q.auto_start && q.text == "重啟保留"),
            "重啟不自動送出",
        )?;
        cases.push("reloaded encrypted history disables automatic start after restart");

        let (chat, old) = self.composer_fixture()?;
        let data_root = self.root.clone();
        let blocked = data_root.join("blocked-root");
        std::fs::write(&blocked, b"fixture").map_err(|e| e.to_string())?;
        self.root = blocked;
        let rejected = self
            .compose_project_message(&chat, &old, ComposeMode::Interrupt, None, "不可遺失")
            .is_err();
        self.root = data_root;
        verify(
            rejected
                && self.composer_queued(&chat).is_none()
                && self
                    .projects
                    .running
                    .as_ref()
                    .is_some_and(|r| !r.cancel.load(Ordering::Relaxed)),
            "保存失敗不得停止舊任務",
        )?;
        self.projects.running = None;
        cases.push("history write failure leaves old worker and queue unchanged");
        let (chat, old) = self.composer_fixture()?;
        let (reply, receiver) = mpsc::channel();
        let folders = vec![
            crate::outlook::privacy::Choice {
                id: "root".into(),
                parent: None,
                name: "本機信箱".into(),
                depth: 0,
                selected: true,
            },
            crate::outlook::privacy::Choice {
                id: "private".into(),
                parent: Some("root".into()),
                name: "私人".into(),
                depth: 1,
                selected: true,
            },
        ];
        self.project_event(ProjectEvent::OutlookConsent(
            old.clone(),
            "picker".into(),
            folders,
            reply,
        ))?;
        let selection = |conversation: &str, ids: Vec<String>| ProjectCommand::OutlookConsent {
            conversation: conversation.into(),
            run_id: old.clone(),
            request_id: "picker".into(),
            allow: true,
            selected: ids,
        };
        verify(
            self.project_command(selection("wrong", vec!["root".into()]))
                .is_err(),
            "資料夾授權不可跨對話",
        )?;
        verify(
            self.project_command(selection(&chat, vec!["unknown".into()]))
                .is_err(),
            "未知資料夾不可授權",
        )?;
        verify(
            self.project_command(selection(&chat, vec!["private".into()]))
                .is_err(),
            "未勾選祖先的資料夾不可授權",
        )?;
        verify(
            receiver.try_recv().is_err(),
            "拒絕偽造選擇時不消耗待確認請求",
        )?;
        self.project_command(selection(&chat, vec!["root".into()]))?;
        verify(
            receiver.try_recv().ok() == Some(Some(vec!["root".into()])),
            "原生確認只交接勾選範圍",
        )?;
        verify(
            self.projects
                .running
                .as_ref()
                .is_some_and(|r| r.pending_outlook.is_none()),
            "確認後移除待處理請求",
        )?;
        self.projects.running = None;
        cases.push("native Outlook folder selection rejects stale identity and unknown or hidden-parent ids");
        // 週報準備只建資料夾；取消、缺少確認與過期對話不可啟動模型。
        let (chat, _) = self.composer_fixture()?;
        self.projects.running = None;
        let request = crate::jobs::new_id()?;
        self.project_command(ProjectCommand::WeeklyPrepare {
            conversation: chat.clone(),
            request_id: request.clone(),
        })?;
        let project = self
            .projects
            .store
            .project_for(&chat)
            .ok_or("缺少週報專案")?
            .clone();
        let count = || {
            std::fs::read_dir(&project.root)
                .map(|v| v.count())
                .unwrap_or(0)
        };
        let created = count();
        self.project_command(ProjectCommand::WeeklyPrepare {
            conversation: chat.clone(),
            request_id: request.clone(),
        })?;
        verify(count() == created, "重複準備不重建資料夾")?;
        let submit = |conversation: String, confirmed| ProjectCommand::WeeklySubmit {
            conversation,
            request_id: request.clone(),
            start: "2026-10-05".into(),
            end: "2026-10-06".into(),
            notes: "請改做 W40，保留使用者要求".into(),
            confirmed,
        };
        verify(
            self.project_command(submit(chat.clone(), false)).is_err(),
            "週報缺少最後確認不可啟動",
        )?;
        verify(
            self.project_command(submit("other-chat".into(), true))
                .is_err(),
            "週報跨對話不可送出",
        )?;
        verify(self.projects.running.is_none(), "准备資料夾未啟動 worker")?;
        self.project_command(submit(chat.clone(), true))?;
        verify(
            self.messages
                .last()
                .is_some_and(|m| m.content.contains("W40") && m.content.contains("ISO 週別")),
            "原生週報訊息帶日期週別與補充",
        )?;
        verify(
            self.project_command(submit(chat, true)).is_err(),
            "已送出週報不可重播",
        )?;
        self.await_composer_worker()?;
        verify(count() >= created, "啟動取消均保留素材資料夾")?;
        cases.push("native weekly folder preparation, confirmation, conversation binding, dated prompt and one-time start");
        Ok(cases)
    }

    /// 每案建立新的歷史與授權目錄；Running 是受控舊任務，不持有 Office 或檔案。
    fn composer_fixture(&mut self) -> AppResult<(String, String)> {
        verify(self.projects.running.is_none(), "上一案 worker 已釋放")?;
        self.archive = Default::default();
        self.projects = Default::default();
        self.work = Default::default();
        self.inbox = Default::default();
        self.work.mode = "background".into();
        self.work.store.principal_id = "composer-account".into();
        self.work.caps = Some(crate::jobs::Capabilities {
            contract_version: 1,
            principal_id: "composer-account".into(),
            execution_modes: vec!["background".into()],
            attachments: Default::default(),
        });
        let old = crate::jobs::new_id()?;
        let folder = self.root.join(&old);
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let project = self.projects.store.add("排程測試", folder)?;
        let mut question = Message::user("原任務");
        question.request_id = Some(old.clone());
        let chat = self.archive.insert(vec![question.clone()])?;
        self.projects
            .store
            .conversations
            .insert(chat.clone(), project);
        self.active_id = Some(chat.clone());
        self.messages = vec![question];
        self.projects.running = Some(Running {
            pending_file: None,
            pending_outlook: None,
            pending_chart: None,
            instructions: projects::steering::Inbox::open(&self.root, &old)?,
            id: old.clone(),
            conversation: chat.clone(),
            activity: vec![],
            charts: vec![],
            started: crate::unix_now(),
            cancel: Arc::new(AtomicBool::new(false)),
        });
        history::save(&self.root, &self.archive)?;
        Ok((chat, old))
    }

    fn composer_queued(&self, chat: &str) -> Option<&history::QueuedProjectMessage> {
        self.archive
            .conversations
            .iter()
            .find(|c| c.id == chat)
            .and_then(|c| c.project_queued.as_ref())
    }

    /// 新 worker 走正式取消／Finished 路徑；等待有界，逾時讓自檢失敗。
    fn await_composer_worker(&mut self) -> AppResult<()> {
        self.projects.cancel();
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.projects.running.is_some() && Instant::now() < deadline {
            if let Ok(Event::Project(event)) = self.rx.recv_timeout(Duration::from_millis(50)) {
                self.project_event(event)?;
            }
        }
        verify(
            self.projects.running.is_none(),
            "取消後等待真實 runner 結束",
        )
    }
}
