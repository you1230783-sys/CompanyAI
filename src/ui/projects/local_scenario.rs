//! 明確命令列啟用的本機情境測試。使用正式App／runner／WebView2，不偽造模型工具。
//! 所有文件與歷史都留在 LocalTest 剛建立的獨立目錄；必要同意不自動代答。
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::ui) struct Scenario {
    requests: Vec<String>,
    #[serde(default)]
    files: std::collections::BTreeMap<String, String>,
    #[serde(skip)]
    started: bool,
    #[serde(skip)]
    next: usize,
}
impl Scenario {
    pub(in crate::ui) fn load(path: &std::path::Path, root: &std::path::Path) -> AppResult<Self> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        if bytes.len() > 1_000_000 {
            return Err("情境檔過大。".into());
        }
        let value: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if value.requests.is_empty()
            || value.requests.len() > 8
            || value
                .requests
                .iter()
                .any(|r| r.is_empty() || r.len() > 16000)
        {
            return Err("情境需1–8項有界要求。".into());
        }
        // WebView2會持有profile目錄；測試專案與app資料分開，沿用正式外部專案界線。
        let name = root
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("本機測試目錄名稱無效。")?;
        let workspace = root.with_file_name(format!("{name}-project"));
        std::fs::create_dir(&workspace).map_err(|e| e.to_string())?;
        let mut store = projects::Store::load(root)?;
        store.projects.first_mut().ok_or("本機專案不存在。")?.root = workspace.clone();
        store.save(root)?;
        let _guard = projects::files::pin(&workspace)?;
        for (name, text) in &value.files {
            if projects::files::relative(name)?.components().count() != 1 {
                return Err("合成情境文件只接受單一檔名。".into());
            }
            projects::text::validate(text)?;
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(workspace.join(name))
                .map_err(|e| e.to_string())?;
            file.write_all(text.as_bytes())
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
        }
        Ok(value)
    }

    /// None代表仍在等待真實模型；Some代表測試流程已結束，成果另由獨立程式核對。
    pub(in crate::ui) fn tick(&mut self, app: &mut App) -> Option<AppResult<()>> {
        if !app.ready || !app.logged_in() || app.models.is_none() {
            return None;
        }
        if let Some(run) = &app.projects.running {
            if run.pending_cad.is_some()
                || run.pending_file.is_some()
                || run.pending_outlook.is_some()
                || run.pending_chart.is_some()
            {
                return Some(self.finish(app, Err("情境需要用戶確認，未自動代答。".into())));
            }
            return None;
        }
        if self.started {
            let message = app.messages.last();
            if message.is_none_or(|m| {
                m.role != "assistant" || m.project_paused || m.content.starts_with("需要你的補充：")
            }) || app.projects.error.is_some()
            {
                return Some(
                    self.finish(app, Err("模型任務未完成；保留原始對話與工作日誌。".into())),
                );
            }
            let completed = (|| -> AppResult<bool> {
                let id = message
                    .and_then(|m| m.request_id.as_deref())
                    .ok_or("缺少本機執行ID。")?;
                crate::jobs::validate_id(id)?;
                let bytes =
                    std::fs::read(app.root.join("project-runs").join(format!("{id}.dpapi")))
                        .map_err(|e| e.to_string())?;
                let record: serde_json::Value =
                    serde_json::from_slice(&crate::storage::protect(&bytes, false)?)
                        .map_err(|e| e.to_string())?;
                Ok(record["result"]["Ok"].is_string())
            })();
            if !matches!(completed, Ok(true)) {
                return Some(self.finish(
                    app,
                    Err("上輪原生任務未成功，不繼續下一個合成要求。".into()),
                ));
            }
        }
        if self.next == self.requests.len() {
            return Some(self.finish(app, Ok(())));
        }
        let result = (|| {
            if !self.started {
                let project = app
                    .projects
                    .store
                    .projects
                    .first()
                    .ok_or("本機測試專案不存在。")?
                    .id
                    .clone();
                app.new_project_chat(&project)?;
                self.started = true;
            }
            let mut messages = app.messages.clone();
            messages.push(Message::user(&self.requests[self.next]));
            app.begin_project_chat(messages)?;
            self.next += 1;
            crate::storage::atomic_write(
                &app.root.join("scenario-progress.json"),
                &serde_json::to_vec_pretty(
                    &json!({"submitted":self.next,"total":self.requests.len()}),
                )
                .map_err(|e| e.to_string())?,
            )
        })();
        match result {
            Ok(()) => None,
            Err(error) => Some(self.finish(app, Err(error))),
        }
    }

    fn finish(&self, app: &App, result: AppResult<()>) -> AppResult<()> {
        let record = json!({"version":env!("CARGO_PKG_VERSION"),"result":if result.is_ok(){"FINISHED_REQUIRES_REVIEW"}else{"STOPPED"},
            "submitted":self.next,"total":self.requests.len(),"error":result.as_ref().err(),"status":app.projects.status,"messages":app.messages});
        crate::storage::atomic_write(
            &app.root.join("scenario-result.json"),
            &serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
        )?;
        result
    }
}
