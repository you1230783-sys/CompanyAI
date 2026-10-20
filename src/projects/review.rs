//! 唯讀內容比較的固定快照；比較來源由 broker 產生，不接受模型提供的舊內容。
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub name: String,
    pub path: String,
    pub before_revision: Option<String>,
    pub after_revision: String,
    pub before: Option<String>,
    pub after: String,
    pub office: bool,
    pub draft: bool,
}

impl Snapshot {
    /// Office JSON 只轉換為可審閱的區塊；保留標籤及格式，不能當作實際頁面排版。
    pub fn display(&self) -> Value {
        fn document(text: &str, office: bool) -> Value {
            if office {
                if let Ok(snapshot) = serde_json::from_str::<super::office::Snapshot>(text) {
                    let blocks: Vec<_> = snapshot.blocks.iter().map(|block| {
                        let format = block.format.as_u64()
                            .and_then(|i| snapshot.formats.get(i as usize))
                            .unwrap_or(&block.format);
                        json!({"label":block.label,"text":block.text,"kind":block.kind,"format":format})
                    }).collect();
                    return json!({"blocks":blocks,"structure":snapshot.structure,"scope":snapshot.scope});
                }
            }
            if text.is_empty() {
                return json!({"blocks":[]});
            }
            json!({"blocks":text.split('\n').enumerate().map(|(i,line)|json!({"label":format!("第 {} 行",i+1),"text":line.trim_end_matches('\r'),"format":null})).collect::<Vec<_>>()})
        }
        json!({"name":self.name,"path":self.path,"before_revision":self.before_revision,
            "after_revision":self.after_revision,"before":self.before.as_ref().map(|s|document(s,self.office)),
            "after":document(&self.after,self.office),"draft":self.draft,"office":self.office,
            "notice":"建立副本時與選定版本的內容快照；不會重新讀取或覆寫目前原檔。Office 為內容比較，並非頁面排版預覽。"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn office_comparison_resolves_format_indices_and_retains_structure() {
        let office = json!({"scope":"Excel","structure":{"sheets":["A"]},"formats":[{"bold":true}],
            "blocks":[{"id":"cell1","label":"A!B2","kind":"number","text":"42","format":0}]})
        .to_string();
        let snapshot = Snapshot {
            name: "a.xlsx".into(),
            path: "out/a.xlsx".into(),
            before_revision: None,
            after_revision: "r1".into(),
            before: None,
            after: office,
            office: true,
            draft: false,
        };
        let display = snapshot.display();
        assert!(display["before"].is_null());
        assert_eq!(display["after"]["blocks"][0]["format"]["bold"], true);
        assert_eq!(display["after"]["blocks"][0]["label"], "A!B2");
        assert_eq!(display["after"]["structure"]["sheets"][0], "A");
    }
}
