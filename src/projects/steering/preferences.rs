//! 可選偏好不阻塞 worker。答案與補充指示在同一把鎖及同一份加密檔內提交，
//! 避免 UI 顯示已回答卻未進入下一輪。登入、Outlook 同意及必要資訊不走此入口。
use super::*;
use serde_json::{json, Value};

#[derive(Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub question: String,
    pub options: Vec<String>,
    pub default: String,
    pub answer: Option<String>,
    pub state: String,
}
impl Inbox {
    /// 每輪固定提供最近偏好，避免工具歷史整理後再次詢問同題。
    /// 只讀狀態，不替使用者回答，也不將預設變成授權或已量測的條件。
    pub fn model_context(&self) -> AppResult<Value> {
        let questions = self.questions()?;
        let recent: Vec<_> = questions
            .iter()
            .rev()
            .take(8)
            .rev()
            .map(|q| {
                json!({
                    "question_id":q.id,"question":q.question.chars().take(180).collect::<String>(),
                    "question_truncated":q.question.chars().count()>180,
                    "state":q.state,"default":q.default,
                    "answer":q.answer.as_ref().map(|a| a.chars().take(240).collect::<String>()),
                    "answer_truncated":q.answer.as_ref().is_some_and(|a| a.chars().count()>240),
                })
            })
            .collect();
        Ok(
            json!({"total":questions.len(),"omitted":questions.len().saturating_sub(recent.len()),
            "pending":questions.iter().filter(|q| q.state=="pending").count(),"recent":recent,
            "rule":"不要重問已列出的可選問題。未回答時依已公告預設處理可先做的工作，不因此停住；預設不是使用者回答，不能當成實測條件或寫入授權。節錄不取代原始工具紀錄與使用者指示。"}),
        )
    }

    pub fn questions(&self) -> AppResult<Vec<Question>> {
        Ok(self
            .state
            .lock()
            .map_err(|_| "偏好狀態無法存取。")?
            .questions
            .clone())
    }
    pub fn ask(
        &self,
        id: &str,
        question: &str,
        options: &[String],
        default: &str,
    ) -> AppResult<String> {
        if question.trim().is_empty()
            || question.chars().count() > 1000
            || !(2..=4).contains(&options.len())
            || options
                .iter()
                .any(|s| s.trim().is_empty() || s.chars().count() > 200)
            || !options.iter().any(|s| s == default)
        {
            return Err("偏好需有簡短問題、2–4個選項及其中一個預設值。".into());
        }
        self.change(|state| {
            if let Some(q) = state
                .questions
                .iter()
                .find(|q| q.question == question && q.options == options && q.default == default)
            {
                return Ok(q.id.clone());
            }
            if state.closed || state.questions.len() >= 20 {
                return Err("本次可選問題已結束或達20項，請沿用已公告預設。".into());
            }
            state.questions.push(Question {
                id: id.into(),
                question: question.into(),
                options: options.to_vec(),
                default: default.into(),
                answer: None,
                state: "pending".into(),
            });
            Ok(id.into())
        })
    }
    pub fn answer(&self, id: &str, answer: &str) -> AppResult<()> {
        if answer.trim().is_empty() || answer.chars().count() > 1000 {
            return Err("回答需為1–1000字。".into());
        }
        self.change(|state| {
            if state.closed {
                return Err("任務已交付，請在完成圖表的編輯器調整，或另送補充。".into());
            }
            let q = state
                .questions
                .iter_mut()
                .find(|q| q.id == id && q.state == "pending")
                .ok_or("問題已回答或已採用預設。")?;
            q.answer = Some(answer.into());
            q.state = "answered".into();
            state.entries.push(Instruction {
                local_time: Some(crate::calendar::local_timestamp()),
                id: jobs::new_id()?,
                text: format!(
                    "使用者回答可選問題「{}」：{}。請更新受影響的成果。",
                    q.question, answer
                ),
                status: "pending".into(),
            });
            Ok(())
        })
    }
    /// 完成前給模型一次明確的預設決策清單；不再等待使用者，也不冒稱使用者已回答。
    pub(super) fn defaults(state: &mut State) -> AppResult<()> {
        let mut defaults = Vec::new();
        for q in &mut state.questions {
            if q.state == "pending" {
                q.state = "default".into();
                defaults.push(format!("{}：{}", q.question, q.default));
            }
        }
        if !defaults.is_empty() {
            state.entries.push(Instruction{local_time:Some(crate::calendar::local_timestamp()),id:jobs::new_id()?,text:format!("桌面可選偏好已依事先公告的預設處理（不是使用者回答）：{}。完成必要成果並在交付中說明；不要再等待或重問同題。",defaults.join("；")),status:"pending".into()});
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preference_context_keeps_state_after_reload_and_bounds_text() {
        let root =
            std::env::temp_dir().join(format!("lmai-pref-context-{}", jobs::new_id().unwrap()));
        let inbox = Inbox::open(&root, "run").unwrap();
        assert_eq!(inbox.model_context().unwrap()["total"], 0);
        inbox
            .ask(
                "q",
                "視圖如何安排",
                &["正側視圖".into(), "正視圖".into()],
                "正側視圖",
            )
            .unwrap();
        let pending = inbox.model_context().unwrap();
        assert_eq!(pending["recent"][0]["state"], "pending");
        assert_eq!(pending["recent"][0]["default"], "正側視圖");
        assert!(pending["recent"][0]["answer"].is_null());
        inbox.answer("q", "其他你安排，門縫沒量仍待確認").unwrap();
        let restored = Inbox::open(&root, "run").unwrap();
        let answered = restored.model_context().unwrap();
        assert_eq!(answered["pending"], 0);
        assert_eq!(
            answered["recent"][0]["answer"],
            "其他你安排，門縫沒量仍待確認"
        );
        for index in 0..10 {
            let id = format!("q{index}");
            let question = format!("{index}{}", "問".repeat(900));
            restored
                .ask(&id, &question, &["甲".into(), "乙".into()], "甲")
                .unwrap();
            restored.answer(&id, &"答".repeat(900)).unwrap();
        }
        let bounded = restored.model_context().unwrap();
        assert_eq!(bounded["recent"].as_array().unwrap().len(), 8);
        assert_eq!(bounded["omitted"], 3);
        assert_eq!(bounded["recent"][0]["question_truncated"], true);
        assert_eq!(bounded["recent"][0]["answer_truncated"], true);
        assert!(bounded.to_string().len() < 20000);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pending_preferences_finish_with_defaults_and_late_answers_do_not_change_results() {
        let root =
            std::env::temp_dir().join(format!("lmai-preference-{}", jobs::new_id().unwrap()));
        let inbox = Inbox::open(&root, "run").unwrap();
        inbox
            .ask("q", "圖例位置", &["右側".into(), "下方".into()], "右側")
            .unwrap();
        assert!(inbox.boundary(false).unwrap().is_empty());
        assert_eq!(inbox.boundary(true).unwrap().len(), 1);
        assert_eq!(inbox.questions().unwrap()[0].state, "default");
        assert!(inbox.boundary(true).unwrap().is_empty());
        assert!(inbox.answer("q", "下方").is_err());
    }
    #[test]
    fn reply_is_delivered_once_and_survives_reopen() {
        let root =
            std::env::temp_dir().join(format!("lmai-preference-{}", jobs::new_id().unwrap()));
        let inbox = Inbox::open(&root, "run").unwrap();
        let options = vec!["缺值".into(), "零".into()];
        let id = inbox.ask("q", "Y值", &options, "缺值").unwrap();
        assert_eq!(inbox.ask("other", "Y值", &options, "缺值").unwrap(), id);
        inbox.answer(&id, "零").unwrap();
        let reopened = Inbox::open(&root, "run").unwrap();
        assert_eq!(
            reopened.questions().unwrap()[0].answer.as_deref(),
            Some("零")
        );
        assert_eq!(reopened.boundary(false).unwrap().len(), 1);
        assert!(reopened.boundary(false).unwrap().is_empty());
        assert!(reopened.boundary(true).unwrap().is_empty());
    }
}
