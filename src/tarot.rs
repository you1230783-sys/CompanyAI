//! 本機牌局：洗牌時決定78張牌的順序與方向，點選只揭示既定結果。
//! 不連線、不讀公司資料；只有明確按下解析才由一般聊天送出文字。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use windows_sys::Win32::Security::Cryptography::{
    BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
};

#[derive(Clone, Deserialize, Serialize)]
pub struct Card {
    pub image: String,
    pub name: String,
    pub upright: String,
    pub reversed: String,
}
pub fn cards() -> &'static [Card] {
    static CARDS: std::sync::OnceLock<Vec<Card>> = std::sync::OnceLock::new();
    CARDS.get_or_init(|| {
        serde_json::from_str(include_str!("../ui/tarot-data.json")).expect("bundled tarot cards")
    })
}
/// 同一副牌的已完成閱讀；未洗牌前持續佔用其牌背位置。
#[derive(Clone)]
struct Reading {
    count: usize,
    picked: Vec<usize>,
    question: String,
}
#[derive(Default)]
pub struct Round {
    id: String,
    count: usize,
    deck: Vec<(usize, bool)>,
    picked: Vec<usize>,
    history: Vec<Reading>,
    question: String,
    /// 解析延續同一個普通聊天；洗牌或該聊天刪除後重新建立。
    pub conversation: Option<String>,
    /// 防止雙擊解析重複送出同一輪；繼續抽牌時重設。
    pub sent: bool,
}
fn random_below(bound: u32) -> AppResult<u32> {
    loop {
        let mut bytes = [0u8; 4];
        if unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                4,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        } < 0
        {
            return Err("無法取得本機隨機數，尚未洗牌。".into());
        }
        let value = u32::from_le_bytes(bytes);
        // 拒絕尾端不完整區間，避免取餘數造成偏差。
        let limit = u64::from(u32::MAX) + 1;
        if u64::from(value) < limit - limit % u64::from(bound) {
            return Ok(value % bound);
        }
    }
}
impl Round {
    pub fn shuffle(&mut self, count: usize, reversed: bool) -> AppResult<()> {
        if !matches!(count, 1 | 3) {
            return Err("請選單張或三張牌。".into());
        }
        let mut deck = Vec::with_capacity(78);
        for index in 0..78 {
            deck.push((index, reversed && random_below(2)? == 1));
        }
        for index in (1..78).rev() {
            deck.swap(index, random_below(index as u32 + 1)? as usize);
        }
        *self = Self {
            id: crate::jobs::new_id()?,
            count,
            deck,
            picked: vec![],
            sent: false,
            ..Default::default()
        };
        Ok(())
    }
    /// 只有明確洗牌會重設牌堆；續問保留順序、正逆位及所有用過的位置。
    pub fn continue_reading(&mut self, id: &str, count: usize) -> AppResult<()> {
        if self.id != id || self.count == 0 || self.picked.len() != self.count {
            return Err("請先抽齊本輪牌，再繼續提問。".into());
        }
        let used = self.history.iter().map(|r| r.picked.len()).sum::<usize>() + self.picked.len();
        if !matches!(count, 1 | 3) || used + count > 78 {
            return Err("剩餘牌數不足，請改選單張或重新洗牌。".into());
        }
        let next_id = crate::jobs::new_id()?;
        self.history.push(Reading {
            count: self.count,
            picked: std::mem::take(&mut self.picked),
            question: std::mem::take(&mut self.question),
        });
        self.count = count;
        self.id = next_id;
        self.sent = false;
        Ok(())
    }
    pub fn record_submission(&mut self, question: &str, conversation: Option<String>) {
        self.question = question.trim().into();
        self.conversation = conversation;
        self.sent = true;
    }
    pub fn pick(&mut self, id: &str, position: usize) -> AppResult<()> {
        if self.id != id || self.deck.len() != 78 {
            return Err("牌局已變更，請使用目前牌桌。".into());
        }
        if position >= 78
            || self.picked.contains(&position)
            || self.history.iter().any(|r| r.picked.contains(&position))
            || self.picked.len() >= self.count
        {
            return Err("這張牌已選過，或已抽足本次張數。".into());
        }
        self.picked.push(position);
        Ok(())
    }
    fn position(count: usize, index: usize) -> &'static str {
        if count == 1 {
            "本次指引"
        } else {
            ["現況", "阻礙", "建議"][index]
        }
    }
    pub fn state(&self) -> Value {
        let chosen = self.reveal(self.count, &self.picked);
        let history: Vec<_> = self
            .history
            .iter()
            .enumerate()
            .map(|(i, r)| {
                json!({
                    "number":i+1,"question":r.question,"chosen":self.reveal(r.count,&r.picked)
                })
            })
            .collect();
        let used: Vec<_> = self
            .history
            .iter()
            .flat_map(|r| r.picked.iter())
            .chain(self.picked.iter())
            .copied()
            .collect();
        // 未選牌的身份不傳到前端；關閉／重繪牌桌不會洗牌。
        json!({"id":self.id,"count":self.count,"chosen":chosen,"sent":self.sent,
            "history":history,"used":used,"remaining":78-used.len(),"number":history.len()+1})
    }
    fn reveal(&self, count: usize, picked: &[usize]) -> Vec<Value> {
        picked.iter().enumerate().map(|(index, position)| {
            let (card, reversed) = self.deck[*position];
            let card = &cards()[card];
            json!({"position":Self::position(count,index),"slot":position,"name":card.name,"image":card.image,
                "reversed":reversed,"meaning":if reversed {&card.reversed} else {&card.upright}})
        }).collect()
    }

    pub fn prompt(&self, id: &str, question: &str) -> AppResult<String> {
        if self.id != id || self.picked.len() != self.count || self.count == 0 || self.sent {
            return Err("請完成抽牌；同一牌局只送出一次解析。".into());
        }
        let question = question.trim();
        if question.is_empty() || question.chars().count() > 3000 {
            return Err("請填寫1–3000字的問題。".into());
        }
        let mut prompt = format!(
            "使用者問題：{question}\n\n使用者已在本機從標準78張塔羅牌自行選取{}張牌，結果如下：\n",
            self.count
        );
        for (index, position) in self.picked.iter().enumerate() {
            let (card, reversed) = self.deck[*position];
            prompt.push_str(&format!(
                "{}. {}：{}（{}）\n",
                index + 1,
                Self::position(self.count, index),
                cards()[card].name,
                if reversed { "逆位" } else { "正位" }
            ));
        }
        if !self.history.is_empty() {
            prompt.push_str("\n以下是同一副牌先前的閱讀紀錄，續問未重新洗牌且不重複抽牌：\n");
            for (number, reading) in self.history.iter().enumerate() {
                // 保留所有牌面；問題只附首輪與最近三輪，避免多輪時反覆擴張提示。
                let previous: String = if number == 0 || number + 3 >= self.history.len() {
                    reading.question.chars().take(200).collect()
                } else {
                    "較早問題詳見對話及牌桌紀錄".into()
                };
                prompt.push_str(&format!("第{}輪問題摘要：{}\n", number + 1, previous));
                for card in self.reveal(reading.count, &reading.picked) {
                    prompt.push_str(&format!(
                        "{}：{}（{}）\n",
                        card["position"].as_str().unwrap_or(""),
                        card["name"].as_str().unwrap_or(""),
                        if card["reversed"] == true {
                            "逆位"
                        } else {
                            "正位"
                        }
                    ));
                }
            }
            prompt.push_str("請將本輪與先前牌面連結解讀；問題摘要可能省略細節，勿自行補造。\n");
        }
        prompt.push_str("\n請依據以上牌面與位置，以繁體中文解析問題可能的含義，區分各牌與整體關聯，提出可反思的問題及具體建議。沿用實際抽出的牌，不重新抽牌；以象徵與可能性解讀，不把牌義當成已證實的事實或必然預測。");
        Ok(prompt)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuation_keeps_deck_history_and_never_reuses_cards_until_shuffle() {
        let mut round = Round::default();
        round.shuffle(1, true).unwrap();
        let fixed = round.deck.clone();
        let initial = round.id.clone();
        for slot in 0..78 {
            let id = round.id.clone();
            round.pick(&id, slot).unwrap();
            if slot > 0 {
                assert!(round.pick(&initial, slot).is_err());
                assert!(round
                    .prompt(&id, "下一步？")
                    .unwrap()
                    .contains("先前的閱讀紀錄"));
            }
            round.record_submission("同一個工作主題", Some("ordinary-chat".into()));
            if slot < 77 {
                round.continue_reading(&id, 1).unwrap();
            }
        }
        assert_eq!(round.deck, fixed);
        assert_eq!(round.state()["remaining"], 0);
        assert_eq!(round.conversation.as_deref(), Some("ordinary-chat"));
        let id = round.id.clone();
        assert!(round.continue_reading(&id, 1).is_err());
        round.shuffle(3, false).unwrap();
        assert_eq!(round.state()["history"], json!([]));
        assert!(round.conversation.is_none());
        assert_eq!(round.state()["remaining"], 78);
    }

    #[test]
    fn unfinished_and_stale_continuation_preserve_existing_reading() {
        let mut round = Round::default();
        round.shuffle(3, true).unwrap();
        let id = round.id.clone();
        round.pick(&id, 5).unwrap();
        assert!(round.continue_reading(&id, 1).is_err());
        round.pick(&id, 6).unwrap();
        round.pick(&id, 7).unwrap();
        assert!(round.continue_reading("stale", 1).is_err());
        round.continue_reading(&id, 1).unwrap();
        let next = round.id.clone();
        assert!(round.pick(&next, 5).is_err());
        assert_eq!(round.state()["remaining"], 75);
    }

    #[test]
    fn complete_deck_is_fixed_between_picks_and_prompt_uses_actual_cards() {
        assert_eq!(cards().len(), 78);
        let unique: std::collections::BTreeSet<_> = cards().iter().map(|c| &c.name).collect();
        assert_eq!(unique.len(), 78);
        let mut round = Round::default();
        round.shuffle(3, true).unwrap();
        let deck = round.deck.clone();
        let id = round.id.clone();
        assert_eq!(
            deck.iter()
                .map(|v| v.0)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            78
        );
        assert!(round.prompt(&id, "問題").is_err());
        for slot in [77, 0, 38] {
            round.pick(&id, slot).unwrap();
        }
        assert_eq!(round.deck, deck);
        assert!(round.pick(&id, 0).is_err());
        let prompt = round.prompt(&id, "接下來如何規劃？").unwrap();
        for slot in [77, 0, 38] {
            assert!(prompt.contains(&cards()[deck[slot].0].name));
        }
        assert!(round.prompt("stale", "問題").is_err());
        round.sent = true;
        assert!(round.prompt(&id, "問題").is_err());
        round.shuffle(1, false).unwrap();
        assert!(round.deck.iter().all(|v| !v.1));
    }
}
