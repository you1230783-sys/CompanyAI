//! 指令範本只產生文字；匯入格式不接受權限、程式或自動送出設定。
use crate::{jobs, storage, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub const MAX_IMPORT: usize = 1_048_576;
const MAX_TEXT: usize = 16_000;
const FILE: &str = "templates.dpapi";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    pub title: String,
    pub category: String,
    pub body: String,
    #[serde(default)]
    pub favorite: bool,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub text: Text,
}
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Store {
    pub entries: Vec<Entry>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Exchange {
    format: String,
    items: Vec<Text>,
}

fn bounded(value: &str, max: usize) -> bool {
    value.encode_utf16().count() <= max
        && !value
            .chars()
            .any(|c| c.is_control() && !['\n', '\r', '\t'].contains(&c))
}
impl Text {
    pub fn validate(&self) -> AppResult<()> {
        if self.title.trim().is_empty()
            || !bounded(&self.title, 100)
            || !bounded(&self.category, 60)
            || self.body.trim().is_empty()
            || !bounded(&self.body, MAX_TEXT)
        {
            return Err("範本需有標題及內容；標題最多100字、分類60字、內容16,000字。".into());
        }
        self.parameters().map(|_| ())
    }
    /// 使用明確的 {{參數}} 語法；單次替換，參數值中的新語法不再展開。
    pub fn parameters(&self) -> AppResult<Vec<String>> {
        let mut names = Vec::new();
        let mut rest = self.body.as_str();
        while let Some((before, tail)) = rest.split_once("{{") {
            if before.contains("}}") {
                return Err("範本參數的大括號未配對。".into());
            }
            let (name, after) = tail.split_once("}}").ok_or("範本參數缺少 }}。")?;
            if name.is_empty()
                || name.chars().count() > 40
                || !name.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                return Err("參數名稱限1至40個文字、數字或底線，例如 {{部門}}。".into());
            }
            if !names.iter().any(|n| n == name) {
                names.push(name.to_owned());
            }
            if names.len() > 20 {
                return Err("每個範本最多20種參數。".into());
            }
            rest = after;
        }
        if rest.contains("}}") {
            return Err("範本參數的大括號未配對。".into());
        }
        Ok(names)
    }
    pub fn render(&self, values: &BTreeMap<String, String>) -> AppResult<String> {
        self.validate()?;
        let names = self.parameters()?;
        if values.len() != names.len()
            || names.iter().any(|n| !values.contains_key(n))
            || values.values().any(|v| !bounded(v, MAX_TEXT))
        {
            return Err("請填寫此範本的全部參數，不能新增其他參數。".into());
        }
        let mut output = String::new();
        let mut rest = self.body.as_str();
        while let Some((before, tail)) = rest.split_once("{{") {
            let (name, after) = tail.split_once("}}").ok_or("範本參數格式錯誤。")?;
            output.push_str(before);
            output.push_str(&values[name]);
            if !bounded(&output, MAX_TEXT) {
                return Err("填入參數後超過16,000字。".into());
            }
            rest = after;
        }
        output.push_str(rest);
        if !bounded(&output, MAX_TEXT) {
            return Err("填入參數後超過16,000字。".into());
        }
        Ok(output)
    }
}
impl Store {
    pub fn load(root: &Path) -> AppResult<Self> {
        let path = root.join(FILE);
        let meta = match fs::metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(_) => return Err("無法讀取範本檔，既有資料已保留。".into()),
        };
        if meta.len() > 8_000_000 {
            return Err("範本檔案過大。".into());
        }
        let data = fs::read(path).map_err(|_| "無法讀取範本檔。")?;
        let store: Self = serde_json::from_slice(&storage::protect(&data, false)?)
            .map_err(|_| "範本檔損毀，請保留原檔並聯絡維護者。")?;
        store.validate()?;
        Ok(store)
    }
    fn validate(&self) -> AppResult<()> {
        if self.entries.len() > 100 {
            return Err("最多保存100個範本。".into());
        }
        let mut ids = BTreeSet::new();
        for entry in &self.entries {
            jobs::validate_id(&entry.id)?;
            if !ids.insert(&entry.id) {
                return Err("範本識別碼重複。".into());
            }
            entry.text.validate()?;
        }
        Ok(())
    }
    pub fn save(&self, root: &Path) -> AppResult<()> {
        self.validate()?;
        let data = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        storage::atomic_write(&root.join(FILE), &storage::protect(&data, true)?)
    }
    pub fn get(&self, id: &str) -> AppResult<&Entry> {
        self.entries
            .iter()
            .find(|e| e.id == id)
            .ok_or("找不到範本，請重新開啟清單。".into())
    }
    pub fn upsert(&mut self, id: Option<&str>, text: Text) -> AppResult<()> {
        text.validate()?;
        if let Some(id) = id {
            self.get(id)?;
            self.entries
                .iter_mut()
                .find(|e| e.id == id)
                .ok_or("找不到範本。")?
                .text = text;
        } else {
            if self.entries.len() >= 100 {
                return Err("最多保存100個範本。".into());
            }
            self.entries.push(Entry {
                id: jobs::new_id()?,
                text,
            });
        }
        Ok(())
    }
    /// 全部驗證成功才合併；外部識別碼不能覆蓋本機內容。
    pub fn import(&mut self, json: &str) -> AppResult<()> {
        if json.len() > MAX_IMPORT {
            return Err("匯入檔最多1 MiB。".into());
        }
        let exchange: Exchange = serde_json::from_str(json)
            .map_err(|_| "匯入格式錯誤；僅接受範本文字欄位，不接受執行或權限設定。")?;
        if exchange.format != "lmai-templates-v1"
            || exchange.items.is_empty()
            || self.entries.len() + exchange.items.len() > 100
        {
            return Err("範本格式版本不符、沒有內容或超過100個。".into());
        }
        let mut next = self.clone();
        for text in exchange.items {
            next.upsert(None, text)?;
        }
        *self = next;
        Ok(())
    }
    pub fn export(&self, id: &str) -> AppResult<String> {
        serde_json::to_string_pretty(&Exchange {
            format: "lmai-templates-v1".into(),
            items: vec![self.get(id)?.text.clone()],
        })
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_store_roundtrip_and_corruption_preserves_file() {
        let root =
            std::env::temp_dir().join(format!("LM_AI_templates_{}", jobs::new_id().unwrap()));
        let mut store = Store::default();
        store.upsert(None, text("private-fixture-body")).unwrap();
        store.save(&root).unwrap();
        let bytes = fs::read(root.join(FILE)).unwrap();
        assert!(!bytes.windows(20).any(|b| b == b"private-fixture-body"));
        assert_eq!(
            Store::load(&root).unwrap().entries[0].text.body,
            "private-fixture-body"
        );
        fs::write(root.join(FILE), b"corrupt-fixture").unwrap();
        assert!(Store::load(&root).is_err());
        assert_eq!(fs::read(root.join(FILE)).unwrap(), b"corrupt-fixture");
        fs::remove_file(root.join(FILE)).unwrap();
        fs::remove_dir(root).unwrap();
    }
    fn text(body: &str) -> Text {
        Text {
            title: "摘要".into(),
            category: "文件".into(),
            body: body.into(),
            favorite: false,
        }
    }
    #[test]
    fn substitutions_are_literal_single_pass_and_exact() {
        let template = text("{{內容}} / {{內容}}");
        let values = BTreeMap::from([("內容".into(), "{{新參數}} <script>run()</script>".into())]);
        assert_eq!(
            template.render(&values).unwrap(),
            "{{新參數}} <script>run()</script> / {{新參數}} <script>run()</script>"
        );
        assert!(template.render(&BTreeMap::new()).is_err());
        assert!(text("{{bad-name}}").validate().is_err());
        assert!(text("{{未關閉").validate().is_err());
        assert!(text("多餘}}").validate().is_err());
    }
    #[test]
    fn import_rejects_privileges_and_is_atomic() {
        let mut store = Store::default();
        store.upsert(None, text("原本")).unwrap();
        for input in [
            r#"{"format":"lmai-templates-v1","items":[],"auto_run":true}"#,
            r#"{"format":"lmai-templates-v1","items":[{"title":"ok","category":"","body":"a","permissions":[] }]}"#,
            r#"{"format":"lmai-templates-v1","items":[{"title":"ok","category":"","body":"a"},{"title":"","category":"","body":"a"}]}"#,
        ] {
            assert!(store.import(input).is_err());
            assert_eq!(store.entries.len(), 1);
        }
        let exported = store.export(&store.entries[0].id).unwrap();
        store.import(&exported).unwrap();
        assert_ne!(store.entries[0].id, store.entries[1].id);
    }
    #[test]
    fn utf16_and_expansion_limits_are_enforced() {
        assert!(text(&"😀".repeat(8001)).validate().is_err());
        assert!(text("{{a}}{{a}}")
            .render(&BTreeMap::from([("a".into(), "x".repeat(8001))]))
            .is_err());
    }
}
