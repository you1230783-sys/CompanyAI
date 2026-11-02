//! 成果中心由使用者開啟，只讀本專案索引；較慢的校驗與打包在背景執行。
use super::*;
impl App {
    pub(super) fn artifact_command(&mut self, command: &ProjectCommand) -> AppResult<bool> {
        if let ProjectCommand::ArtifactAction {
            conversation,
            request_id,
            id,
            label,
        } = command
        {
            if !self.logged_in()
                || self.versions.blocked()
                || self.active_id.as_deref() != Some(conversation)
            {
                return Err("請在原專案對話選擇成果。".into());
            }
            let project = self
                .projects
                .store
                .project_for(conversation)
                .ok_or("目前不是專案對話。")?
                .clone();
            let conversation = conversation.clone();
            let request_id = request_id.clone();
            let id = id.clone();
            let label = label.clone();
            let tx = self.tx.clone();
            thread::spawn(move || {
                let result = (|| {
                    let memory = projects::memory::Memory::open(project, &conversation)?;
                    let data = if let Some(label) = label {
                        memory.select_artifact(&id, &label)?;
                        json!({"selection_saved":true})
                    } else {
                        memory.artifact_preview(&id)?
                    };
                    serde_json::to_string(&data).map_err(|e| e.to_string())
                })();
                let _ = tx.send(Event::Project(ProjectEvent::ArtifactCenter(
                    conversation,
                    request_id,
                    result,
                )));
            });
            return Ok(true);
        }
        let ProjectCommand::ArtifactCenter {
            conversation,
            request_id,
            offset,
            export_ids,
        } = command
        else {
            return Ok(false);
        };
        if !self.logged_in()
            || self.versions.blocked()
            || self.active_id.as_deref() != Some(conversation)
        {
            return Err("請在原專案對話查看成果中心。".into());
        }
        let project = self
            .projects
            .store
            .project_for(conversation)
            .ok_or("目前不是專案對話。")?
            .clone();
        let conversation = conversation.clone();
        let request_id = request_id.clone();
        let tx = self.tx.clone();
        let ids = export_ids.clone();
        let offset = *offset;
        if ids.len() > 100 || offset > 5000 {
            return Err("成果選取或位置超過範圍。".into());
        }
        thread::spawn(move || {
            let result = (|| {
                let memory = projects::memory::Memory::open(project.clone(), &conversation)?;
                let data = if ids.is_empty() {
                    memory.artifact_center(offset)?
                } else {
                    let path = memory.export_artifacts(&ids)?;
                    json!({"exported":path,"notice":"交付包已逐檔校驗並讀回。來源需更新的舊成果會保留版本提醒，不自動重算。"})
                };
                serde_json::to_string(&data).map_err(|e| e.to_string())
            })();
            let _ = tx.send(Event::Project(ProjectEvent::ArtifactCenter(
                conversation,
                request_id,
                result,
            )));
        });
        Ok(true)
    }
}
