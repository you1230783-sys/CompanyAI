//! 對話分支只交換本機歷史；不呼叫模型、工具或搬移授權。
use super::*;

/// 目前版本的識別。舊格式自然視為第1版，仍沿用網站對話歷史。
#[derive(Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub id: String,
    pub number: u32,
    pub parent: Option<String>,
    pub fork_index: Option<usize>,
    /// 分支後每次請求都使用獨立網站對話及完整選定快照，避免網站補回舊分支。
    pub snapshot: bool,
}
impl Default for BranchInfo {
    fn default() -> Self {
        Self {
            id: "original".into(),
            number: 1,
            parent: None,
            fork_index: None,
            snapshot: false,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct SavedBranch {
    pub info: BranchInfo,
    pub messages: Vec<Message>,
    pub draft: String,
}
impl Conversation {
    /// 保存舊版本再建立新提問；只保留分支點以前內容及該提問明確持有的附件識別。
    pub fn fork_message(&mut self, index: usize, text: &str) -> AppResult<()> {
        if self.alternate_branches.len() >= 31 {
            return Err("每個對話最多32個版本，請另開對話。".into());
        }
        let original = self
            .messages
            .get(index)
            .filter(|m| m.role == "user")
            .ok_or("只能編輯使用者訊息。")?;
        if text.trim().is_empty() || text.encode_utf16().count() > 16_000 {
            return Err("提問不可空白，且最多16,000字元。".into());
        }
        let id = crate::jobs::new_id()?;
        let number = self
            .alternate_branches
            .iter()
            .map(|b| b.info.number)
            .chain([self.branch.number])
            .max()
            .unwrap_or(1)
            + 1;
        let info = BranchInfo {
            id,
            number,
            parent: Some(self.branch.id.clone()),
            fork_index: Some(index),
            snapshot: true,
        };
        let mut question = Message::user(text);
        question.attachments = original.attachments.clone();
        question.retry_settings = original.retry_settings.clone();
        let mut messages = self.messages[..index].to_vec();
        messages.push(question);
        self.alternate_branches.push(SavedBranch {
            info: std::mem::replace(&mut self.branch, info),
            messages: std::mem::replace(&mut self.messages, messages),
            draft: std::mem::take(&mut self.draft),
        });
        self.updated_at = unix_now();
        Ok(())
    }
    /// 原子保存由呼叫端處理；此處只交換內容與草稿，沒有送出或重播行為。
    pub fn select_branch(&mut self, id: &str) -> AppResult<()> {
        if self.branch.id == id {
            return Ok(());
        }
        let index = self
            .alternate_branches
            .iter()
            .position(|b| b.info.id == id)
            .ok_or("找不到此對話版本。")?;
        let target = &mut self.alternate_branches[index];
        std::mem::swap(&mut self.branch, &mut target.info);
        std::mem::swap(&mut self.messages, &mut target.messages);
        std::mem::swap(&mut self.draft, &mut target.draft);
        self.updated_at = unix_now();
        Ok(())
    }
    pub(super) fn validate_branches(&self) -> AppResult<()> {
        let mut ids = std::collections::BTreeSet::new();
        let mut numbers = std::collections::BTreeSet::new();
        if self.alternate_branches.len() > 31 {
            return Err("對話版本數超過上限。".into());
        }
        for info in self
            .alternate_branches
            .iter()
            .map(|b| &b.info)
            .chain([&self.branch])
        {
            crate::jobs::validate_id(&info.id)?;
            if !ids.insert(&info.id)
                || !numbers.insert(info.number)
                || !(1..=32).contains(&info.number)
                || (info.id != "original" && !info.snapshot)
            {
                return Err("對話版本識別不正確。".into());
            }
        }
        if !ids.contains(&"original".to_string()) {
            return Err("缺少原始對話版本。".into());
        }
        for info in self
            .alternate_branches
            .iter()
            .map(|b| &b.info)
            .chain([&self.branch])
        {
            if info.id == "original" {
                if info.number != 1
                    || info.parent.is_some()
                    || info.fork_index.is_some()
                    || info.snapshot
                {
                    return Err("原始版本識別不正確。".into());
                }
            } else {
                let parent = self
                    .alternate_branches
                    .iter()
                    .map(|b| &b.info)
                    .chain([&self.branch])
                    .find(|p| Some(&p.id) == info.parent.as_ref());
                if info.fork_index.is_none_or(|i| i >= 1000)
                    || parent.is_none_or(|p| p.number >= info.number)
                {
                    return Err("分支來源或順序不正確。".into());
                }
            }
        }
        for branch in &self.alternate_branches {
            if branch.draft.encode_utf16().count() > 16_000
                || branch.messages.len() > 1000
                || branch.messages.iter().any(|m| {
                    !matches!(m.role.as_str(), "user" | "assistant") || m.content.len() > 1_048_576
                })
            {
                return Err("分支內容過大或格式不正確。".into());
            }
        }
        Ok(())
    }
}
/// 已有請求識別或任何執行紀錄便不允許原地編輯；只接受最後一則尚未送出的提問。
pub fn editable_project_draft(messages: &[Message], index: usize) -> bool {
    index.checked_add(1) == Some(messages.len())
        && messages.get(index).is_some_and(|m| {
            m.role == "user"
                && m.request_id.is_none()
                && !m.incomplete
                && !m.project_paused
                && m.project_activity.is_empty()
                && m.project_overview.is_none()
                && m.project_analysis.is_none()
                && m.project_charts.is_empty()
                && m.project_diagrams.is_empty()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edit_preserves_old_tail_and_draft_and_serializes_selected_version() {
        let mut archive = Archive::default();
        let mut question = Message::user("old question");
        question.request_id = Some("old_request".into());
        let id = archive
            .insert(vec![
                Message::user("prefix"),
                question,
                Message::assistant("OLD_TAIL".into()),
            ])
            .unwrap();
        let chat = &mut archive.conversations[0];
        chat.draft = "old draft".into();
        chat.fork_message(1, "edited").unwrap();
        assert_eq!(chat.messages.len(), 2);
        assert!(chat.messages[1].request_id.is_none());
        assert!(!serde_json::to_string(&chat.messages)
            .unwrap()
            .contains("OLD_TAIL"));
        let edited = chat.branch.id.clone();
        chat.select_branch("original").unwrap();
        assert_eq!(chat.messages[2].content, "OLD_TAIL");
        assert_eq!(chat.draft, "old draft");
        chat.select_branch(&edited).unwrap();
        archive.validate().unwrap();
        let encoded = serde_json::to_vec(&archive).unwrap();
        let restored: Archive = serde_json::from_slice(&encoded).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.conversations[0].id, id);
        assert_eq!(restored.conversations[0].branch.id, edited);
    }
    #[test]
    fn legacy_history_defaults_and_project_execution_boundary() {
        let mut archive = Archive::default();
        archive.insert(vec![Message::user("draft")]).unwrap();
        let mut value = serde_json::to_value(&archive).unwrap();
        value["conversations"][0]
            .as_object_mut()
            .unwrap()
            .remove("branch");
        value["conversations"][0]
            .as_object_mut()
            .unwrap()
            .remove("alternate_branches");
        let old: Archive = serde_json::from_value(value).unwrap();
        old.validate().unwrap();
        assert_eq!(old.conversations[0].branch.id, "original");
        let mut messages = old.conversations[0].messages.clone();
        assert!(editable_project_draft(&messages, 0));
        messages[0].request_id = Some("started".into());
        assert!(!editable_project_draft(&messages, 0));
    }
    #[test]
    fn branch_graph_rejects_cycles_and_bounds_versions_without_losing_history() {
        let mut archive = Archive::default();
        archive.insert(vec![Message::user("question")]).unwrap();
        for _ in 0..31 {
            archive.conversations[0].fork_message(0, "edit").unwrap();
        }
        archive.validate().unwrap();
        let before = serde_json::to_vec(&archive).unwrap();
        assert!(archive.conversations[0]
            .fork_message(0, "overflow")
            .is_err());
        assert_eq!(before, serde_json::to_vec(&archive).unwrap());
        archive.conversations[0].branch.parent = Some(archive.conversations[0].branch.id.clone());
        assert!(archive.validate().is_err());
        assert!(!editable_project_draft(&[], usize::MAX));
    }
}
