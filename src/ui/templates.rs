//! 範本的保存與草稿操作；任何範本文字都不能直接呼叫聊天或工具。
use super::*;
use crate::templates::{Store, Text};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum TemplateCommand {
    State,
    Save {
        id: Option<String>,
        text: Text,
    },
    Delete {
        id: String,
    },
    Import {
        json: String,
    },
    PreviewExport {
        id: String,
    },
    Export {
        preview_id: String,
    },
    Use {
        id: String,
        values: BTreeMap<String, String>,
        conversation: Option<String>,
    },
}
pub(super) struct TemplateRuntime {
    store: AppResult<Store>,
    export: Option<(String, String)>,
}
impl TemplateRuntime {
    pub(super) fn load(root: &Path) -> Self {
        Self {
            store: Store::load(root),
            export: None,
        }
    }
}
impl App {
    pub(super) fn template_command(&mut self, command: TemplateCommand) -> AppResult<()> {
        let result = (|| {
            let mut next = self.templates.store.as_ref().map_err(Clone::clone)?.clone();
            match command {
                TemplateCommand::State => (),
                TemplateCommand::Save { id, text } => {
                    next.upsert(id.as_deref(), text)?;
                    next.save(&self.root)?;
                    self.templates.store = Ok(next);
                }
                TemplateCommand::Delete { id } => {
                    next.get(&id)?;
                    next.entries.retain(|e| e.id != id);
                    next.save(&self.root)?;
                    self.templates.store = Ok(next);
                }
                TemplateCommand::Import { json } => {
                    next.import(&json)?;
                    next.save(&self.root)?;
                    self.templates.store = Ok(next);
                }
                TemplateCommand::PreviewExport { id } => {
                    let text = next.export(&id)?;
                    let preview_id = crate::jobs::new_id()?;
                    self.view.post(
                        &json!({"type":"template_export","preview_id":preview_id,"text":text}),
                    )?;
                    self.templates.export = Some((preview_id, text));
                }
                TemplateCommand::Export { preview_id } => {
                    let (id, text) = self.templates.export.as_ref().ok_or("請先預覽匯出內容。")?;
                    if id != &preview_id {
                        return Err("匯出預覽已過期。".into());
                    }
                    if self.save_json_file(
                        "匯出指令範本",
                        "LM_AI_Templates.json",
                        text.as_bytes(),
                    )? {
                        self.toast("範本已匯出。分享前請確認沒有私人資料。");
                    }
                    self.templates.export = None;
                }
                TemplateCommand::Use {
                    id,
                    values,
                    conversation,
                } => {
                    if conversation != self.active_id
                        || self.busy != "none"
                        || self.projects.active(self.active_id.as_deref())
                    {
                        return Err("對話已切換或正在處理，請回到目標對話再加入草稿。".into());
                    }
                    let text = next.get(&id)?.text.render(&values)?;
                    self.append_draft_text(&text)?;
                    self.view.post(&json!({"type":"template_applied"}))?;
                }
            }
            let entries: Vec<_> = self.templates.store.as_ref().map_err(Clone::clone)?.entries.iter()
                .map(|e| json!({"id":e.id,"text":e.text,"parameters":e.text.parameters().unwrap_or_default()})).collect();
            self.view
                .post(&json!({"type":"template_state","entries":entries}))
        })();
        if let Err(error) = &result {
            self.view
                .post(&json!({"type":"template_error","message":error}))?;
        }
        result
    }
    /// 保留使用者原草稿；儲存失敗時回復記憶體，不顯示已套用。
    pub(super) fn append_draft_text(&mut self, text: &str) -> AppResult<()> {
        let original = self.draft.clone();
        let combined = if original.is_empty() {
            text.to_owned()
        } else {
            format!("{original}\n\n{text}")
        };
        if combined.encode_utf16().count() > 16_000 {
            return Err("合併後的草稿超過16,000字，請先縮短內容。".into());
        }
        self.set_draft(combined);
        if let Err(error) = self.preserve_draft() {
            self.set_draft(original);
            return Err(error);
        }
        self.publish();
        Ok(())
    }
}
