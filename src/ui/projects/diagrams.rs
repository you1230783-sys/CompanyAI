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
        if let ProjectCommand::DiagramSource { target, node_id } = command {
            if !self.logged_in()
                || self.versions.blocked()
                || self.active_id.as_deref() != Some(&target.conversation)
            {
                return Err("請在原專案對話查看來源。".into());
            }
            let project = self
                .projects
                .store
                .project_for(&target.conversation)
                .ok_or("此對話不是專案。")?
                .clone();
            let diagram = self
                .messages
                .get(target.message_index)
                .filter(|m| m.request_id.as_deref() == Some(&target.request_id))
                .and_then(|m| m.project_diagrams.get(target.diagram_index))
                .ok_or("圖形訊息已變更。")?;
            diagram.validate()?;
            let origin = diagram
                .node_sources
                .iter()
                .find(|n| n.node_id == *node_id)
                .ok_or("此節點沒有檔案來源；描述節點不提供檔案導覽。")?
                .clone();
            let tx = self.tx.clone();
            let conversation = target.conversation.clone();
            thread::spawn(move || {
                let result = (|| {
                    let changed = projects::files::fingerprint(&project, &origin.path)
                        .map(|h| h != origin.source_sha256)
                        .unwrap_or(true);
                    let text = if !changed {
                        if let (Some(a), Some(b)) = (origin.first_line, origin.last_line) {
                            let (content, _) = projects::files::read(&project, &origin.path)?;
                            content
                                .split_inclusive('\n')
                                .enumerate()
                                .skip(a - 1)
                                .take(b - a + 1)
                                .map(|(i, line)| format!("{}: {}", i + 1, line))
                                .collect::<String>()
                                .chars()
                                .take(6000)
                                .collect()
                        } else {
                            origin.excerpt.clone()
                        }
                    } else {
                        origin.excerpt.clone()
                    };
                    serde_json::to_string(&json!({"source":origin,"changed":changed,"text":text,
                        "notice":if changed{"來源已變更或無法讀取；顯示的是原閱讀摘錄，節點待重新核對，不以舊行號導向新內容。"}else{"唯讀來源預覽；版本相符不代表AI語意已驗收。"}})).map_err(|e|e.to_string())
                })();
                let _ = tx.send(Event::Project(ProjectEvent::DiagramSource(
                    conversation,
                    result,
                )));
            });
            return Ok(true);
        }
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
                    &projects::files::conversation_folder_name(&conversation)?,
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
