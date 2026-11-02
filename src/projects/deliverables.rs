//! 需求清單保存模型對使用者要求的拆解；完成狀態由 Broker 的副本及成果紀錄產生。
//! 不接受模型自填「已保存」，也不讓一次更新悄悄移除先前承諾的項目。
use crate::{jobs, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub description: String,
    /// file：實體檔案；answer：最終文字回答。後者內容正確性仍需人工／模型驗收。
    pub kind: String,
    pub target: Option<String>,
    pub blocked_reason: Option<String>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Plan {
    pub items: Vec<Item>,
    #[serde(default)]
    pub conditions_revision: u64,
}
impl Plan {
    /// 採增量更新。相同需求 ID 的類型及描述保持不變，避免用改名掩蓋缺交項目。
    pub fn update(&mut self, items: &[Item], conditions_revision: u64) -> AppResult<()> {
        if items.is_empty() || items.len() > 20 {
            return Err("交付清單每次1–20項。".into());
        }
        let mut next = self.clone();
        if self.conditions_revision != conditions_revision
            && self
                .items
                .iter()
                .any(|old| !items.iter().any(|item| item.id == old.id))
        {
            return Err("條件變更後需一次核對所有既有交付項目，未完成項目不可省略。".into());
        }
        let mut ids = BTreeSet::new();
        for item in items {
            jobs::validate_id(&item.id)?;
            if !ids.insert(&item.id)
                || item.description.trim().is_empty()
                || item.description.chars().count() > 300
                || !["file", "answer"].contains(&item.kind.as_str())
                || item
                    .target
                    .as_ref()
                    .is_some_and(|s| s.is_empty() || s.len() > 1000)
                || item
                    .blocked_reason
                    .as_ref()
                    .is_some_and(|s| s.trim().is_empty() || s.chars().count() > 500)
                || (item.kind == "answer" && item.target.is_some())
            {
                return Err(
                    "交付項目格式無效：描述1–300字，類型file／answer，未完成原因1–500字。".into(),
                );
            }
            if let Some(old) = next.items.iter_mut().find(|old| old.id == item.id) {
                if (old.kind != item.kind || old.description != item.description)
                    && self.conditions_revision == conditions_revision
                {
                    return Err(
                        "既有需求不可直接改名或改類型；條件變更後重新核對，或另加一項。".into(),
                    );
                }
                *old = item.clone();
            } else {
                next.items.push(item.clone());
            }
        }
        if next.items.len() > 20 {
            return Err("每任務最多20項交付要求。".into());
        }
        next.conditions_revision = conditions_revision;
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn condition_changes_require_all_items_and_fail_without_partial_mutation() {
        let mut plan = Plan::default();
        let items: Vec<_> = (0..20)
            .map(|index| Item {
                id: format!("item-{index}"),
                description: format!("成果{index}"),
                kind: "file".into(),
                target: None,
                blocked_reason: None,
            })
            .collect();
        plan.update(&items, 0).unwrap();
        assert!(plan.update(&items[..1], 1).is_err());
        assert_eq!(plan.conditions_revision, 0);
        plan.update(&items, 1).unwrap();
        let extra = Item {
            id: "extra".into(),
            description: "額外".into(),
            kind: "answer".into(),
            target: None,
            blocked_reason: None,
        };
        assert!(plan.update(&[extra], 1).is_err());
        assert_eq!(plan.items.len(), 20);
        assert!(serde_json::from_value::<Item>(
            serde_json::json!({"id":"fake","description":"偽造","kind":"file",
            "target":null,"blocked_reason":null,"status":"saved"})
        )
        .is_err());
    }
    #[test]
    fn updates_cannot_drop_or_rename_unfinished_requirements() {
        let mut plan = Plan::default();
        let item = Item {
            id: "word".into(),
            description: "Word報告".into(),
            kind: "file".into(),
            target: None,
            blocked_reason: None,
        };
        plan.update(std::slice::from_ref(&item), 0).unwrap();
        let mut other = item.clone();
        other.id = "excel".into();
        other.description = "Excel報告".into();
        plan.update(&[other], 0).unwrap();
        assert_eq!(plan.items.len(), 2);
        let mut renamed = item;
        renamed.kind = "answer".into();
        assert!(plan.update(&[renamed], 0).is_err());
    }
}
