//! 簡短識別碼只供查閱來源；內部冪等鍵仍使用完整 ID，避免把查詢別名當成寫入操作。
use super::*;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct References {
    /// 以完整 ID 為鍵，序號一經分配就隨 checkpoint 保存，不因封存或排序改變。
    ids: BTreeMap<String, String>,
    next: u64,
}

impl References {
    pub fn register(&mut self, id: &str) -> String {
        if let Some(alias) = self.ids.get(id) {
            return alias.clone();
        }
        let alias = loop {
            self.next += 1;
            let candidate = format!("op_{:04}", self.next);
            if candidate != id
                && !self.ids.contains_key(&candidate)
                && !self.ids.values().any(|value| value == &candidate)
            {
                break candidate;
            }
        };
        self.ids.insert(id.into(), alias.clone());
        alias
    }

    pub fn resolve(&self, value: &str) -> AppResult<String> {
        let canonical = self.ids.iter().find(|(_, alias)| alias.as_str() == value);
        if let Some((id, _)) = canonical {
            // 舊版允許任意合法操作名；若碰巧與別名相同，拒絕猜測其指向。
            if self.ids.contains_key(value) && id != value {
                return Err("此識別碼有歧義，請重新列出操作索引並使用明確的來源 ID。".into());
            }
            return Ok(id.clone());
        }
        Ok(value.into())
    }

    pub fn alias(&self, id: &str) -> String {
        self.ids.get(id).cloned().unwrap_or_else(|| id.into())
    }

    /// 只轉換筆記的結構欄位，絕不改動原文、摘要或程式碼中的相同字串。
    pub fn project(&self, value: &mut Value) {
        match value {
            Value::Array(items) => items.iter_mut().for_each(|v| self.project(v)),
            Value::Object(fields) => {
                for (key, value) in fields {
                    if key == "operation_id" {
                        if let Some(id) = value.as_str() {
                            *value = json!(self.alias(id));
                        }
                    } else {
                        self.project(value);
                    }
                }
            }
            _ => (),
        }
    }
}

impl Broker {
    /// 待整理階段只允許保存／補查筆記，非同步圖片路徑也必須遵守同一入口守衛。
    pub(in crate::projects) fn reading_gate(&self, tool: &Tool) -> Option<Value> {
        if self.reading.enabled
            && !self.reading.pending.is_empty()
            && !matches!(
                tool,
                Tool::RecordReadingNotes { .. }
                    | Tool::ReadReadingNotes { .. }
                    | Tool::ReadWorkLog { .. }
                    | Tool::LoadSkill { .. }
                    | Tool::ReadSkillGuide { .. }
            )
        {
            Some(
                json!({"ok":false,"error":"請先record_reading_notes保存pending來源，包含使用者要求相關事實、數字／單位、推論及缺項，再處理下一來源。"}),
            )
        } else {
            None
        }
    }

    pub(in crate::projects) fn project_reading_references(&self, value: &mut Value) {
        self.references.project(value);
    }

    pub(in crate::projects) fn reading_context(&self) -> Option<Value> {
        self.reading.context().map(|mut value| {
            self.references.project(&mut value);
            value
        })
    }

    pub(in crate::projects) fn reference_id(&self, id: &str) -> String {
        self.references.alias(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_survive_restore_and_never_guess_hash_prefixes() {
        let mut refs = References::default();
        let first = "12345678aaaaaaaaaaaaaaaaaaaaaaaa";
        let second = "12345678bbbbbbbbbbbbbbbbbbbbbbbb";
        assert_eq!(refs.register(first), "op_0001");
        assert_eq!(refs.register(second), "op_0002");
        let mut restored: References = serde_json::from_value(json!(refs)).unwrap();
        assert_eq!(restored.register(first), "op_0001");
        assert_eq!(restored.resolve("op_0002").unwrap(), second);
        assert_eq!(restored.resolve("12345678").unwrap(), "12345678");
        assert_eq!(restored.register("third"), "op_0003");
        let mut note = json!({"operation_id":first,"summary":first});
        restored.project(&mut note);
        assert_eq!(note["operation_id"], "op_0001");
        assert_eq!(note["summary"], first);
        restored.register("op_0001");
        assert!(restored.resolve("op_0001").is_err());
    }
}
