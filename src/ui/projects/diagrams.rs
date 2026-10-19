//! 使用者匯出只取原生歷史中的原始碼，不接受前端提供 SVG 或檔案路徑。
use super::*;
use std::path::Path;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::ui) struct Target {
    pub conversation: String,
    pub message_index: usize,
    pub request_id: String,
    pub diagram_index: usize,
}
impl App {
    pub(super) fn diagram_command(&mut self, command: &ProjectCommand) -> AppResult<bool> {
        let ProjectCommand::DiagramSave { target, format } = command else {
            return Ok(false);
        };
        if !self.logged_in()
            || self.versions.blocked()
            || self.active_id.as_deref() != Some(&target.conversation)
        {
            return Err("請在已登入的原專案對話儲存圖形。".into());
        }
        let project = self
            .projects
            .store
            .project_for(&target.conversation)
            .ok_or("此對話不屬於專案。")?
            .clone();
        let diagram = self
            .messages
            .get(target.message_index)
            .filter(|m| m.request_id.as_deref() == Some(&target.request_id))
            .and_then(|m| m.project_diagrams.get(target.diagram_index))
            .ok_or("圖形所在訊息已變更，請重新開啟。")?
            .clone();
        diagram.validate()?;
        let format = *format;
        let (reply, response) = mpsc::channel();
        if !matches!(format, projects::diagrams::Format::Mmd) {
            self.view.render_mermaid(&diagram.source, format, reply)?;
        }
        let tx = self.tx.clone();
        let conversation = target.conversation.clone();
        thread::spawn(move || {
            let result = (|| {
                let bytes = if matches!(format, projects::diagrams::Format::Mmd) {
                    diagram.source.as_bytes().to_vec()
                } else {
                    projects::diagrams::decode_rendered(
                        &response
                            .recv_timeout(Duration::from_secs(60))
                            .map_err(|_| "圖形匯出逾時。")??,
                        format,
                    )?
                };
                let stem = Path::new(&diagram.name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or("檔名無法辨識。")?;
                projects::files::diagrams::save(
                    &project,
                    &format!("{stem}.{}", format.extension()),
                    &bytes,
                )
            })();
            let _ = tx.send(Event::Project(ProjectEvent::UserDiagramSaved(
                conversation,
                result,
            )));
        });
        Ok(true)
    }
}
