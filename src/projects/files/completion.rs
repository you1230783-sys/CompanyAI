//! Broker的completion操作；由父模組保留共用狀態與既有介面。
use super::*;

impl Broker {
    pub fn finish(&self, artifacts: &[String]) -> AppResult<Vec<String>> {
        self.finish_cancellable(artifacts, &AtomicBool::new(false))
    }
    /// Office 交付讀回也要遵守取消，不能在使用者取消後繼續逐檔啟動 Office。
    pub fn finish_cancellable(
        &self,
        artifacts: &[String],
        cancel: &AtomicBool,
    ) -> AppResult<Vec<String>> {
        if self.reading.enabled && !self.reading.pending.is_empty() {
            return Err("尚有上一批閱讀成果未保存，請先record_reading_notes再交付。".into());
        }
        let unique: std::collections::BTreeSet<_> = artifacts.iter().collect();
        self.verify_report_values(cancel)?;
        if self
            .workflow
            .as_ref()
            .is_some_and(|a| !a.blocked && a.next < a.template.steps.len())
        {
            return Err("工作流程仍有未執行步驟，請完成或明確回報中止原因。".into());
        }
        if unique.len() != artifacts.len() || artifacts.len() != self.copies.len() {
            return Err(
                "交付清單必須列出本次所有工作副本且不可重複；不需要的未發布副本請先捨棄。".into(),
            );
        }
        let mut paths = Vec::new();
        for id in artifacts {
            let copy = self.copies.get(id).ok_or("完成清單含未知副本。")?;
            if copy.saved_revision.as_deref() != Some(text::revision(&copy.text).as_str()) {
                return Err("成果尚未成功儲存最新版本，不能交付。".into());
            }
            if copy.name.to_ascii_lowercase().ends_with(".py") {
                copy.code_review.ready(&text::revision(&copy.text))?;
            }
            let path = copy.paths.last().ok_or("成果尚未儲存。")?;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("成果檢查已取消。".into());
            }
            if read_cancel(&self.project, path, cancel, None, None)?.0 != copy.text {
                return Err("成果交付前已變更，請重新確認。".into());
            }
            paths.push(path.clone());
        }
        if self
            .copies
            .values()
            .any(|copy| copy.saved_revision.as_deref() != Some(text::revision(&copy.text).as_str()))
        {
            return Err("仍有未儲存的工作副本，請先儲存或捨棄。".into());
        }
        for dataset in &self.datasets {
            crate::projects::datasets::load(
                &self.project,
                &dataset.path,
                &dataset.revision,
                cancel,
            )?;
            paths.push(dataset.path.clone());
        }
        for artifact in &self.cad_artifacts {
            self.verify_cad_artifact(artifact)?;
            paths.push(artifact.path.clone());
        }
        for artifact in &self.comparison_artifacts {
            use sha2::{Digest, Sha256};
            let bytes = crate::projects::artifacts::bytes(&self.project, &artifact.path)?;
            if format!("{:x}", Sha256::digest(&bytes)) != artifact.sha256 {
                return Err("CAD比較報告已變更，不能交付。".into());
            }
            paths.push(artifact.path.clone());
        }
        for artifact in &self.python_artifacts {
            self.verify_python_artifact(artifact)?;
            paths.push(artifact.path.clone());
        }
        // PNG 已由匯出工具發布，不是文字工作副本；自動併入交付清單並重新核對。
        for diagram in &self.diagrams {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("成果檢查已取消。".into());
            }
            diagram.validate()?;
            if read_cancel(&self.project, &diagram.path, cancel, None, None)?.0 != diagram.source {
                return Err("Mermaid 原始碼成果已變更，不能交付。".into());
            }
            paths.push(diagram.path.clone());
        }
        for export in &self.diagram_exports {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("成果檢查已取消。".into());
            }
            self.verify_diagram_export(export)?;
            paths.push(export.path.clone());
        }
        for export in &self.chart_exports {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("成果檢查已取消。".into());
            }
            self.verify_chart_export(export)?;
            paths.push(export.path.clone());
        }
        let paths = self.primary_delivery_paths(paths);
        self.verify_deliverables(&paths)?;
        Ok(paths)
    }
}
