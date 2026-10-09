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
        let (content, encoding) = if let Some(copy) = self.copies.get_mut(path) {
            if extension(Path::new(&copy.name))? != "py" {
                return Err("語法檢查只支援PY工作副本。".into());
            }
            copy.python_checked_revision = None;
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
