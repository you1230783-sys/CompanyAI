//! 隱藏 EXE 自檢：以受控事件驗證正式控制器、DPAPI 與晚到回覆。
//! HTTP 傳輸另由 jobs 的 loopback 測試涵蓋；此處不連線公司服務。
use super::*;

fn verify(ok: bool, case: &str) -> AppResult<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("任務追蹤自檢失敗：{case}"))
    }
}

impl App {
    pub(in crate::ui) fn verify_task_tracking(&mut self) -> AppResult<()> {
        if !self.smoke {
            return Err("任務追蹤自檢僅供 self-check 使用。".into());
        }
        let original_root = self.root.clone();
        self.root = original_root.join(format!("tracking-{}", jobs::new_id()?));
        let result = self.task_tracking_cases();
        self.work = Default::default();
        self.archive = Default::default();
        self.active_id = None;
        self.messages.clear();
        self.root = original_root;
        let cases = result?;
        storage::atomic_write(
            &self.root.join("task-tracking-verification.json"),
            &serde_json::to_vec_pretty(&json!({
                "version":env!("CARGO_PKG_VERSION"), "result":"PASS", "cases":cases,
                "transport":"controlled events; loopback transport tested separately",
                "company_service_tested":false
            }))
            .map_err(|error| error.to_string())?,
        )
    }

    fn tracking_fixture(&mut self) -> AppResult<String> {
        self.work = Default::default();
        self.work.store.principal_id = "tracking_fixture".into();
        self.archive = Default::default();
        let id = jobs::new_id()?;
        let mut user = Message::user("請分析測試附件");
        user.request_id = Some(id.clone());
        let conversation = self.archive.insert(vec![user])?;
        self.active_id = Some(conversation.clone());
        self.messages = self.archive.conversations[0].messages.clone();
        let task = serde_json::from_value(json!({
            "request_id":id, "conversation_id":conversation,
            "request":{"messages":[{"role":"user","content":"測試"}]},
            "mode":"stream", "title":"追蹤自檢", "created_at":0, "remote":null, "applied":false
        }))
        .map_err(|error| error.to_string())?;
        self.work.store.tasks.push(task);
        self.work_save()?;
        Ok(id)
    }

    fn tracking_error(&mut self, id: &str, status: u32, healthy: bool) -> AppResult<()> {
        let remote_id = self.work.store.tasks[0]
            .remote
            .as_ref()
            .map(|remote| remote.task_id.clone());
        self.work_event(WorkEvent::TaskPoll {
            id: id.into(),
            remote_id,
            result: Err(jobs::polling::PollFailure {
                http_status: Some(status),
                message: format!("HTTP {status}：追蹤測試錯誤"),
            }),
            models_healthy: healthy,
        })
    }

    fn tracking_status(id: &str) -> AppResult<TaskStatus> {
        serde_json::from_value(json!({
            "task_id":"remote_fixture", "client_request_id":id, "state":"running"
        }))
        .map_err(|error| error.to_string())
    }

    fn task_tracking_cases(&mut self) -> AppResult<Vec<&'static str>> {
        let mut cases = vec![];
        let id = self.tracking_fixture()?;
        self.work.streams.insert(id.clone());
        verify(
            !self.work.store.tasks[0].can_poll(true),
            "等待 POST 不預先 GET",
        )?;
        self.work_event(WorkEvent::Submitted(
            id.clone(),
            Err(jobs::submission::SubmitFailure::rejected(
                "HTTP 413：附件文件 ID 總數超過 20。",
            )),
        ))?;
        let restored = WorkStore::load(&self.root, &self.config, "tracking_fixture")?;
        verify(
            !restored.tasks[0].active() && restored.tasks[0].message.contains("HTTP 413"),
            "413 停止與 DPAPI 保存",
        )?;
        self.work_event(WorkEvent::Submitted(id.clone(), Ok(())))?;
        self.work_event(WorkEvent::Update(
            id.clone(),
            jobs::StreamUpdate::Delta("晚到文字".into()),
        ))?;
        self.work_event(WorkEvent::Polled(
            id,
            Ok(Self::tracking_status(&restored.tasks[0].request_id)?),
        ))?;
        verify(
            !self.work.store.tasks[0].active()
                && self.work.store.tasks[0].message.contains("HTTP 413"),
            "晚到事件不能覆蓋原始拒絕",
        )?;
        cases.push(
            "pending POST has no GET; 413 stops, survives DPAPI reload and ignores late events",
        );

        let id = self.tracking_fixture()?;
        self.work.store.tasks[0].partial = "已收到的答案片段".into();
        for _ in 0..2 {
            self.tracking_error(&id, 404, false)?;
        }
        verify(self.work.store.tasks[0].active(), "前兩次404保留追蹤")?;
        self.tracking_error(&id, 404, true)?;
        let restored = WorkStore::load(&self.root, &self.config, "tracking_fixture")?;
        verify(
            !restored.tasks[0].active()
                && self.archive.conversations[0].messages[1].content == "已收到的答案片段",
            "第三次404保存片段並停止",
        )?;
        cases.push("third identical 404 with healthy models stops; partial answer and stopped state persist");

        let id = self.tracking_fixture()?;
        for _ in 0..3 {
            self.tracking_error(&id, 503, false)?;
        }
        verify(self.work.store.tasks[0].active(), "網站異常不誤停止")?;
        self.tracking_error(&id, 503, true)?;
        verify(
            !self.work.store.tasks[0].active(),
            "網站恢復後可停止持續錯誤任務",
        )?;
        cases.push("unhealthy models prevent stopping; subsequent healthy check permits stopping");

        let id = self.tracking_fixture()?;
        for status in [404, 500, 404] {
            self.tracking_error(&id, status, true)?;
        }
        verify(self.work.store.tasks[0].active(), "交替錯誤不算三次相同")?;
        self.work_event(WorkEvent::TaskPoll {
            id: id.clone(),
            remote_id: None,
            result: Ok(Self::tracking_status(&id)?),
            models_healthy: false,
        })?;
        for _ in 0..2 {
            self.tracking_error(&id, 404, true)?;
        }
        verify(self.work.store.tasks[0].active(), "成功查詢清除前次計數")?;
        cases.push("changing errors and successful GET reset the consecutive failure count");

        let id = self.tracking_fixture()?;
        self.work_event(WorkEvent::Update(
            id.clone(),
            jobs::StreamUpdate::Status(Box::new(Self::tracking_status(&id)?)),
        ))?;
        for _ in 0..3 {
            self.work_event(WorkEvent::TaskPoll {
                id: id.clone(),
                remote_id: None,
                result: Err(jobs::polling::PollFailure {
                    http_status: Some(404),
                    message: "舊路由404".into(),
                }),
                models_healthy: true,
            })?;
        }
        verify(
            self.work.store.tasks[0].active() && self.work.poll_failures.is_empty(),
            "忽略晚到by-request404",
        )?;
        cases.push("SSE task ID supersedes late by-request errors without counting them");
        Ok(cases)
    }
}
