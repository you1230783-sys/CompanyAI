//! Broker的chart_work操作；由父模組保留共用狀態與既有介面。
use super::*;

impl Broker {
    pub fn set_chart_chooser(
        &mut self,
        chooser: Option<crate::projects::charts::quality::Chooser>,
        deadline: std::time::Instant,
    ) {
        self.chart_chooser = chooser;
        self.chart_deadline = deadline;
    }
    pub(super) fn review_chart(
        &mut self,
        prepared: crate::projects::charts::quality::Prepared,
        cancel: &AtomicBool,
    ) -> AppResult<Option<crate::projects::charts::Chart>> {
        if self.preferences.is_some() {
            return self.review_optional_chart(prepared);
        }
        let choices = if prepared.review.groups.is_empty() {
            vec![]
        } else {
            let Some(chooser) = self.chart_chooser.as_mut() else {
                return Ok(None);
            };
            let Some(choices) = chooser(&prepared.review, cancel, self.chart_deadline)? else {
                return Ok(None);
            };
            choices
        };
        prepared.apply(&choices).map(Some)
    }
    pub(super) fn add_chart(&mut self, chart: crate::projects::charts::Chart) -> AppResult<Value> {
        chart.validate()?;
        if self.charts.len() >= 12 {
            return Err("每次任務最多 12 張圖表。".into());
        }
        if serde_json::to_vec(&self.charts)
            .map_err(|e| e.to_string())?
            .len()
            + serde_json::to_vec(&chart).map_err(|e| e.to_string())?.len()
            > 16 * 1024 * 1024
        {
            return Err("本次任務的圖表資料合計超過 16 MiB，請分成另一個任務。".into());
        }
        let data_note = chart.data_note.clone();
        self.charts.push(chart);
        Ok(
            json!({"chart_index":self.charts.len()-1,"displayed_in_conversation":true,"data_note":data_note}),
        )
    }
    pub(super) fn verify_chart_export(&self, export: &ChartExport) -> AppResult<()> {
        let path = relative(&export.path)?;
        if path.components().count() != 3
            || !path.starts_with("_AI_Output")
            || !path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("png"))
        {
            return Err("PNG 成果路徑不合法。".into());
        }
        let target = self.project.root.join(path);
        let _guards = pin(target.parent().ok_or("PNG 目錄不存在。")?)?;
        let mut file = checked_file(&target)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take((crate::projects::charts::png::MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        crate::projects::charts::png::validate(&bytes)?;
        use sha2::{Digest, Sha256};
        if format!("{:x}", Sha256::digest(&bytes)) != export.sha256 {
            return Err("PNG 成果已變更，請確認後重新匯出。".into());
        }
        Ok(())
    }
    /// CSV 只建立新成果，讀回核對成功才加入可引用索引；不覆寫原始資料。
    pub(super) fn save_dataset(
        &mut self,
        name: &str,
        table: &crate::projects::datasets::Table,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let rel = relative(name)?;
        if rel.components().count() != 1
            || rel
                .extension()
                .is_none_or(|s| !s.eq_ignore_ascii_case("csv"))
        {
            return Err("資料集名稱需為單一 CSV 檔名。".into());
        }
        if self.datasets.len() >= 30 {
            return Err("每個任務最多 30 份 CSV 資料集。".into());
        }
        let csv = table.csv()?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("CSV 匯出已取消。".into());
        }
        let _root = pin(&self.project.root)?;
        let base = self.project.root.join("_AI_Output");
        match fs::create_dir(&base) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.to_string()),
        }
        let _base = pin(&base)?;
        let folder_name = shared_output_folder(
            &base,
            &mut self.output_folder,
            self.output_conversation.as_deref(),
        )?;
        let folder = base.join(&folder_name);
        let _folder = pin(&folder)?;
        let (actual_name, mut file) = reserve_output(&folder, name)?;
        let path = format!("_AI_Output/{folder_name}/{actual_name}");
        self.published.push(path.clone());
        file.write_all(csv.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("CSV 可能已部分寫入 {path}，尚未交付：{e}"))?;
        file.rewind().map_err(|e| e.to_string())?;
        let mut readback = String::new();
        Read::by_ref(&mut file)
            .take((crate::projects::datasets::MAX_BYTES + 1) as u64)
            .read_to_string(&mut readback)
            .map_err(|e| e.to_string())?;
        if readback != csv {
            return Err(format!("CSV {path} 讀回不一致，尚未交付。"));
        }
        let reference = crate::projects::datasets::Reference {
            path,
            revision: text::revision(&csv),
            rows: table.rows.len(),
            columns: table.columns.clone(),
        };
        let result = table.summary(&reference);
        self.datasets.push(reference);
        Ok(result)
    }
    pub(super) fn export_chart_png(
        &mut self,
        index: usize,
        name: &str,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let relative_name = relative(name)?;
        if relative_name.components().count() != 1
            || !relative_name
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("png"))
        {
            return Err("請提供單一 PNG 檔名，例如「亮度趨勢.png」，不要包含資料夾。".into());
        }
        let chart = self
            .charts
            .get(index)
            .ok_or("找不到本次任務的圖表編號，請先建立圖表。")?;
        chart.validate()?;
        let chart_revision =
            text::revision(&serde_json::to_string(chart).map_err(|e| e.to_string())?);
        // 同圖、同檔名的再次要求沿用已驗證成果；任務暫停後亦保留此記錄。
        if let Some(export) = self.chart_exports.iter().find(|e| {
            e.chart_index == index && e.name == name && e.chart_revision == chart_revision
        }) {
            self.verify_chart_export(export)?;
            return Ok(
                json!({"chart_index":index,"path":export.path,"verified":true,"reused":true}),
            );
        }
        let bytes = self
            .png_renderer
            .as_mut()
            .ok_or("目前沒有可用的桌面圖表匯出器。")?(chart, cancel)?;
        crate::projects::charts::png::validate(&bytes)?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("PNG 匯出已取消，未寫入檔案。".into());
        }
        let _root = pin(&self.project.root)?;
        let base = self.project.root.join("_AI_Output");
        match fs::create_dir(&base) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.to_string()),
        }
        let _base = pin(&base)?;
        let folder_name = shared_output_folder(
            &base,
            &mut self.output_folder,
            self.output_conversation.as_deref(),
        )?;
        let folder = base.join(&folder_name);
        let _folder = pin(&folder)?;
        let (actual_name, mut file) = reserve_output(&folder, name)?;
        let path = format!("_AI_Output/{folder_name}/{actual_name}");
        // 保留可能部分寫入的路徑；失敗時明確回報，不將檔案登記為成功匯出。
        self.published.push(path.clone());
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("PNG 可能已部分寫入 {path}，尚未交付：{e}"))?;
        file.rewind().map_err(|e| e.to_string())?;
        let mut verified = Vec::new();
        Read::by_ref(&mut file)
            .take((crate::projects::charts::png::MAX_BYTES + 1) as u64)
            .read_to_end(&mut verified)
            .map_err(|e| e.to_string())?;
        if verified != bytes {
            return Err(format!("PNG {path} 讀回不一致，尚未交付。"));
        }
        use sha2::{Digest, Sha256};
        self.chart_exports.push(ChartExport {
            chart_revision,
            chart_index: index,
            name: name.into(),
            path: path.clone(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        });
        Ok(
            json!({"chart_index":index,"path":path,"verified":true,"width":crate::projects::charts::png::WIDTH,"height":crate::projects::charts::png::HEIGHT}),
        )
    }
}
