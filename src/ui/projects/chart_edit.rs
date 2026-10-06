//! 完成圖表的使用者編輯與匯出；以原生歷史索引取圖，不接受前端重新提交資料點。
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::ui) struct Target {
    pub conversation: String,
    pub message_index: usize,
    pub request_id: String,
    pub chart_index: usize,
}
impl App {
    pub(super) fn chart_edit_command(&mut self, command: &ProjectCommand) -> AppResult<bool> {
        let (target, style, save) = match command {
            ProjectCommand::ChartCustomize { target, style } => (target, style, false),
            ProjectCommand::ChartSave { target, style } => (target, style, true),
            _ => return Ok(false),
        };
        if !self.logged_in()
            || self.versions.blocked()
            || self.active_id.as_deref() != Some(&target.conversation)
        {
            return Err("請在已登入的原專案對話操作圖表。".into());
        }
        let project = self
            .projects
            .store
            .project_for(&target.conversation)
            .ok_or("此對話已不屬於專案。")?
            .clone();
        let message = self
            .messages
            .get(target.message_index)
            .filter(|m| m.request_id.as_deref() == Some(&target.request_id))
            .ok_or("圖表所在訊息已變更，請重新開啟。")?;
        let chart = message
            .project_charts
            .get(target.chart_index)
            .ok_or("找不到原始圖表。")?
            .clone();
        if let Some(style) = style {
            style.validate(&chart)?;
        }
        if !save {
            let mut archive = self.archive.clone();
            let chat = archive
                .conversations
                .iter_mut()
                .find(|c| c.id == target.conversation)
                .ok_or("對話不存在。")?;
            let message = chat
                .messages
                .get_mut(target.message_index)
                .filter(|m| m.request_id.as_deref() == Some(&target.request_id))
                .ok_or("保存的訊息已變更。")?;
            if let Some(style) = style {
                message
                    .project_chart_styles
                    .insert(target.chart_index, style.clone());
            } else {
                message.project_chart_styles.remove(&target.chart_index);
            }
            history::save(&self.root, &archive)?;
            self.messages = archive
                .conversations
                .iter()
                .find(|c| c.id == target.conversation)
                .ok_or("對話不存在。")?
                .messages
                .clone();
            self.archive = archive;
            return Ok(true);
        }
        let (reply, receive) = mpsc::channel();
        self.view
            .export_custom_chart_png(&chart, style.as_ref(), reply)?;
        let tx = self.tx.clone();
        let conversation = target.conversation.clone();
        // WebView callback 在UI執行緒；等待及檔案I/O放到背景，避免阻塞操作。
        thread::spawn(move || {
            let result = receive
                .recv_timeout(Duration::from_secs(60))
                .map_err(|_| "圖片繪製逾時，尚未儲存。".to_owned())
                .and_then(|r| r)
                .and_then(|url| projects::charts::png::decode_url(&url))
                .and_then(|bytes| projects::files::save_user_chart(&project, &bytes));
            let _ = tx.send(Event::Project(ProjectEvent::UserChartSaved(
                conversation,
                result,
            )));
        });
        Ok(true)
    }
}
