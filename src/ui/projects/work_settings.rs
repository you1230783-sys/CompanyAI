//! 偏好與條件只接受目前對話的桌面操作；保存成功才回傳新版本。
use super::*;
use projects::work_settings::{Preferences, Settings};

impl App {
    pub(super) fn work_settings_command(&mut self, command: &ProjectCommand) -> AppResult<bool> {
        let ProjectCommand::WorkSettings {
            conversation,
            request_id,
            expected,
            personal,
            project,
            conditions,
        } = command
        else {
            return Ok(false);
        };
        if !self.logged_in()
            || self.versions.blocked()
            || self.active_id.as_deref() != Some(conversation)
        {
            return Err("請在已登入的原對話編輯工作偏好。".into());
        }
        let result: AppResult<serde_json::Value> = (|| {
            let mut settings = Settings::load(&self.root)?;
            let fingerprint = projects::text::revision(
                &serde_json::to_string(&settings).map_err(|e| e.to_string())?,
            );
            let project_id = self
                .projects
                .store
                .project_for(conversation)
                .map(|p| p.id.clone());
            let writing = personal.is_some() || project.is_some() || conditions.is_some();
            if writing {
                if expected.as_deref() != Some(fingerprint.as_str()) {
                    return Err("偏好或條件已變更，請重新開啟後再修改。".into());
                }
                if let Some(value) = personal {
                    value.validate()?;
                    settings.personal = value.clone();
                }
                if let Some(value) = project {
                    let id = project_id
                        .as_ref()
                        .ok_or("只有專案對話可以保存專案偏好。")?;
                    value.validate()?;
                    if value == &Preferences::default() {
                        settings.projects.remove(id);
                    } else {
                        settings.projects.insert(id.clone(), value.clone());
                    }
                }
                if let Some(value) = conditions {
                    if project_id.is_none() {
                        return Err("條件卡僅適用專案對話。".into());
                    }
                    settings.set_conditions(conversation, value.revision, &value.text)?;
                }
                settings.save(&self.root)?;
            }
            let effective = settings.snapshot(project_id.as_deref().unwrap_or(""), conversation);
            let fingerprint = projects::text::revision(
                &serde_json::to_string(&settings).map_err(|e| e.to_string())?,
            );
            Ok(
                json!({"personal":settings.personal,"project":project_id.as_ref().and_then(|id|settings.projects.get(id)).cloned().unwrap_or_default(),
                "conditions":effective.conditions,"effective":effective.preferences,"is_project":project_id.is_some(),"fingerprint":fingerprint,"saved":writing}),
            )
        })();
        self.view.post(&json!({"type":"work_settings","conversation":conversation,"request_id":request_id,"data":result.as_ref().ok(),"error":result.as_ref().err()}))?;
        Ok(true)
    }
}
