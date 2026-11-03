//! 離線牌桌的原生入口；解析才沿用一般聊天，絕不送入目前專案或夾帶舊附件。
use super::*;
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum TarotCommand {
    State,
    Shuffle { count: usize, reversed: bool },
    Pick { id: String, position: usize },
    Continue { id: String, count: usize },
    Analyze { id: String, question: String },
}
impl App {
    pub(super) fn tarot_command(&mut self, command: TarotCommand) -> AppResult<()> {
        let result = (|| {
            match command {
                TarotCommand::State => (),
                TarotCommand::Shuffle { count, reversed } => self.tarot.shuffle(count, reversed)?,
                TarotCommand::Continue { id, count } => self.tarot.continue_reading(&id, count)?,
                TarotCommand::Pick { id, position } => self.tarot.pick(&id, position)?,
                TarotCommand::Analyze { id, question } => {
                    let prompt = self.tarot.prompt(&id, &question)?;
                    if !self.can_send() {
                        return Err("抽牌可離線使用；AI解析請先登入並選擇可用模型。".into());
                    }
                    if !self.work.store.drafts(None).is_empty() {
                        return Err("請先處理尚未歸入對話的附件，再送出塔羅解析。".into());
                    }
                    let previous = self
                        .tarot
                        .conversation
                        .as_ref()
                        .and_then(|id| self.archive.conversations.iter().find(|c| &c.id == id))
                        .cloned();
                    if let Some(chat) = &previous {
                        // 續問不可覆蓋這個聊天裡使用者另寫的草稿。
                        if !chat.draft.trim().is_empty() {
                            return Err("塔羅對話中還有未送出的草稿，請先處理後再解析。".into());
                        }
                        if self.work.store.pending(Some(&chat.id))
                            || !self.work.store.drafts(Some(&chat.id)).is_empty()
                        {
                            return Err(
                                "上一輪塔羅對話仍在處理或含待送附件，請完成後再解析。".into()
                            );
                        }
                    }
                    self.preserve_draft()?;
                    self.active_id = previous.as_ref().map(|c| c.id.clone());
                    self.messages = previous.map(|c| c.messages).unwrap_or_default();
                    self.set_draft(prompt.clone());
                    self.focus_draft = true;
                    // 與按下普通聊天傳送相同；沒有新增技能或使用專案代理。
                    let before = self.work.store.tasks.len();
                    let result = self.begin_chat(prompt, "send");
                    // 已保存任務即鎖定本牌局；提交失聯也不能再產生第二份相同問題。
                    if result.is_ok() || self.work.store.tasks.len() > before {
                        self.tarot
                            .record_submission(&question, self.active_id.clone());
                    }
                    result?;
                    self.view.post(&json!({"type":"tarot_sent"}))?;
                }
            }
            self.view
                .post(&json!({"type":"tarot_state","state":self.tarot.state()}))
        })();
        if let Err(error) = &result {
            let _ = self
                .view
                .post(&json!({"type":"tarot_error","message":error}));
        }
        result
    }
}
