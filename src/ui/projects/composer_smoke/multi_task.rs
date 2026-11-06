//! 受控 Running 搭配正式控制器，驗證雙任務路由；不冒充公司模型驗收。
use super::*;

impl App {
    pub(super) fn verify_two_project_tasks(&mut self) -> AppResult<Vec<&'static str>> {
        let (chat_a, id_a) = self.composer_fixture()?;
        let project = self.projects.store.conversations[&chat_a].clone();
        let id_b = crate::jobs::new_id()?;
        let chat_b = self.archive.insert(vec![Message::user("第二項工作")])?;
        self.projects
            .store
            .conversations
            .insert(chat_b.clone(), project);
        let cancel_b = Arc::new(AtomicBool::new(false));
        self.projects.running.insert(
            chat_b.clone(),
            Running {
                status: "B 的進度".into(),
                resource_wait: Default::default(),
                received_reply: false,
                cad_read_progress: None,
                pending_cad: None,
                pending_file: None,
                pending_outlook: None,
                instructions: projects::steering::Inbox::open(&self.root, &id_b)?,
                pending_chart: None,
                id: id_b.clone(),
                conversation: chat_b.clone(),
                activity: vec![],
                charts: vec![],
                analysis: None,
                started: crate::unix_now(),
                cancel: cancel_b.clone(),
            },
        );
        let cancel_a = self.projects.run(&id_a).ok_or("缺少 A")?.cancel.clone();
        verify(
            self.project_slots_full() && self.project_tasks().len() == 2,
            "兩個獨立任務卡片",
        )?;
        let before = self.archive.conversations.len();
        verify(
            self.begin_project_for("third".into(), vec![], None, None)
                .unwrap_err()
                .contains("2 個"),
            "第三項工作在建立 worker 前拒絕",
        )?;
        verify(
            self.archive.conversations.len() == before,
            "拒絕第三項不改歷史",
        )?;

        let (reply_a, receiver_a) = mpsc::channel();
        let (reply_b, receiver_b) = mpsc::channel();
        self.project_event(ProjectEvent::FileBusy(
            id_a.clone(),
            "file_a".into(),
            "A 的 Word".into(),
            reply_a,
        ))?;
        self.project_event(ProjectEvent::FileBusy(
            id_b.clone(),
            "file_b".into(),
            "B 的 Excel".into(),
            reply_b,
        ))?;
        self.active_id = Some(chat_b.clone());
        verify(
            self.project_state()["file_busy"]["request_id"] == "file_b",
            "只顯示目前對話確認卡",
        )?;
        verify(
            self.project_command(ProjectCommand::FileReady {
                conversation: chat_a.clone(),
                run_id: id_a.clone(),
                request_id: "file_a".into(),
                retry: true,
            })
            .is_err(),
            "切到 B 後拒絕 A 舊卡片",
        )?;
        verify(
            receiver_a.try_recv().is_err() && receiver_b.try_recv().is_err(),
            "過期卡片沒有交付任何授權",
        )?;
        self.project_command(ProjectCommand::FileReady {
            conversation: chat_b.clone(),
            run_id: id_b.clone(),
            request_id: "file_b".into(),
            retry: true,
        })?;
        verify(
            receiver_b.try_recv() == Ok(true) && receiver_a.try_recv().is_err(),
            "B 確認只送到 B",
        )?;

        let worker_a = std::thread::current().id();
        let worker_b = std::thread::spawn(|| std::thread::current().id())
            .join()
            .map_err(|_| "測試執行緒中止")?;
        self.project_event(ProjectEvent::ResourceWait(
            id_b.clone(),
            worker_a,
            Some("等待 Word".into()),
        ))?;
        self.project_event(ProjectEvent::ResourceWait(
            id_b.clone(),
            worker_b,
            Some("等待 Excel".into()),
        ))?;
        self.project_event(ProjectEvent::ResourceWait(id_b.clone(), worker_a, None))?;
        verify(
            self.project_state()["status"] == "等待 Excel",
            "平行子工具解除一個等待不清除另一個",
        )?;
        self.project_event(ProjectEvent::ResourceWait(id_b.clone(), worker_b, None))?;
        self.project_command(ProjectCommand::StopTask {
            conversation: chat_a.clone(),
            run_id: id_a.clone(),
        })?;
        verify(
            cancel_a.load(Ordering::Relaxed) && !cancel_b.load(Ordering::Relaxed),
            "停止 A 不停止 B",
        )?;
        verify(
            self.project_command(ProjectCommand::StopTask {
                conversation: chat_b.clone(),
                run_id: id_a.clone(),
            })
            .is_err(),
            "停止按鈕綁定對話與 run",
        )?;
        self.projects.running.remove(&chat_a);
        self.verify_schedule_slot_accounting()?;
        verify(
            !self.project_slots_full() && self.projects.active(Some(&chat_b)),
            "A 結束釋放名額，B 繼續",
        )?;
        // 以真正存在的兩個 Running 檢查 panic 隔離入口，不觸發程序全域 panic hook。
        let mut a = self.projects.running.remove(&chat_b).ok_or("缺少 B")?;
        a.id = id_a;
        a.conversation = chat_a.clone();
        a.cancel = cancel_a.clone();
        cancel_a.store(false, Ordering::Relaxed);
        self.projects.running.insert(chat_a, a);
        self.projects.running.insert(
            chat_b.clone(),
            Running {
                status: String::new(),
                resource_wait: Default::default(),
                received_reply: false,
                cad_read_progress: None,
                pending_cad: None,
                pending_file: None,
                pending_outlook: None,
                instructions: projects::steering::Inbox::open(&self.root, &id_b)?,
                pending_chart: None,
                id: id_b,
                conversation: chat_b,
                activity: vec![],
                charts: vec![],
                analysis: None,
                started: crate::unix_now(),
                cancel: cancel_b.clone(),
            },
        );
        self.projects.quarantine();
        verify(
            cancel_a.load(Ordering::Relaxed)
                && cancel_b.load(Ordering::Relaxed)
                && !self.projects.is_running(),
            "全域保護取消兩項並撤銷確認",
        )?;
        // 再走正式啟動／Finished 路徑，驗證不是只有手工填入兩個 Running 的表面狀態。
        let (real_a, _) = self.composer_fixture()?;
        self.projects.running.clear();
        let project = self.projects.store.conversations[&real_a].clone();
        let real_b = self.archive.insert(vec![Message::user("並行 B")])?;
        self.projects
            .store
            .conversations
            .insert(real_b.clone(), project);
        self.begin_project_for(real_a.clone(), vec![Message::user("並行 A")], None, None)?;
        self.begin_project_for(real_b.clone(), vec![Message::user("並行 B")], None, None)?;
        verify(self.projects.count() == 2, "兩個真實 runner 已各自啟動")?;
        let run_a = self
            .projects
            .current(Some(&real_a))
            .ok_or("缺少真實 A")?
            .id
            .clone();
        self.projects.cancel_one(&real_a, &run_a)?;
        verify(
            !self
                .projects
                .current(Some(&real_b))
                .ok_or("缺少真實 B")?
                .cancel
                .load(Ordering::Relaxed),
            "真實 B 沒有共用 A 取消旗標",
        )?;
        self.await_composer_worker()?;
        Ok(vec!["M11: two conversation slots and cards; third rejected before worker/history mutation",
            "M11: pending file cards and replies bound to active conversation/run/request",
            "M11: per-thread resource wait, independent stop, stale stop rejected, quarantine cancels both",
            "M11: schedule shares second slot and execution-bound stop cannot stop replacement",
            "M11: two real runner threads started on loopback fixture, separate cancellation, both Finished awaited"])
    }
}
