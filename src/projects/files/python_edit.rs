//! Python試用編輯沿用文字副本。固定檢查器只編譯原文，不載入或執行使用者模組。
use super::*;

impl Broker {
    pub(super) fn check_python(
        &mut self,
        path: &str,
        revision: &str,
        cancel: &AtomicBool,
        worker: &mut Worker,
    ) -> AppResult<Value> {
        let canonical = self.code_copy_id(path).unwrap_or_else(|| path.into());
        let path = canonical.as_str();
        let (content, encoding) = if let Some(copy) = self.copies.get_mut(path) {
            if extension(Path::new(&copy.name))? != "py" {
                return Err("語法檢查只支援PY工作副本。".into());
            }
            (copy.text.clone(), copy.encoding)
        } else {
            if extension(Path::new(path))? != "py" {
                return Err("語法檢查只支援專案PY檔案。".into());
            }
            read_cancel(&self.project, path, cancel, Some(worker), None)?
        };
        if text::revision(&content) != revision {
            return Err("Python版本已改變，請重新讀取再檢查。".into());
        }
        let bytes = text::encode(&content, encoding)?;
        if self
            .copies
            .get(path)
            .is_some_and(|c| c.python_checked_revision.as_deref() == Some(revision))
        {
            return Ok(
                json!({"path":path,"revision":revision,"syntax_valid":true,"source_executed":false,"reused":true,"notice":"同一版本語法已通過；請依需求提交功能測試，不重複檢查。"}),
            );
        }
        let response = super::super::python::execute(
            include_str!("../python/check_source.py"),
            json!([{"name":"source","kind":"text","text":content,"encoded_hex":super::super::python::hex(&bytes)}]),
            cancel,
        )?;
        let mut summary = response["summary"].clone();
        if !summary["syntax_valid"].is_boolean() || summary["source_executed"] != false {
            return Err("Python檢查器回傳格式不符，未記錄通過。".into());
        }
        if summary["syntax_valid"] == true {
            if let Some(copy) = self.copies.get_mut(path) {
                copy.python_checked_revision = Some(revision.into());
            }
        }
        summary["path"] = json!(path);
        summary["revision"] = json!(revision);
        summary["notice"] = json!("僅驗證此版本於內建Python的語法、編碼及編譯範圍規則；未執行程式，未驗證邏輯、外部套件或其他Python版本。修改後需重查。" );
        Ok(summary)
    }
}
