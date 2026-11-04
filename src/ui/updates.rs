//! 主視窗的updates操作；由父模組保留共用狀態與既有介面。
use super::*;

impl App {
    /// 下載前一次告知關閉與重新開啟；同意只適用當時看到的版本。
    pub(super) fn confirm_update(&mut self) -> AppResult<bool> {
        let info = self.versions.known.as_ref().ok_or("尚未取得版本資訊。")?;
        if !info.available() {
            return Ok(false);
        }
        let version = info.latest_version.clone();
        let text = format!("是否要自動下載更新並安裝 {version}？\n\n提示：會暫時關閉 LM_AI，並在安裝完成後再次開啟，不會重新啟動電腦。正在執行的本機操作將停止。\n\n若此版本僅提供獨立 EXE，下載後會顯示手動更換方式。") ;
        if unsafe {
            MessageBoxW(
                self.window,
                wide(&text).as_ptr(),
                wide("更新 LM_AI").as_ptr(),
                MB_YESNO | MB_DEFBUTTON2 | MB_ICONQUESTION,
            )
        } != IDYES
        {
            return Ok(false);
        }
        self.update_consent = Some(version);
        Ok(true)
    }
    pub(super) fn start_update_download(&mut self) -> AppResult<()> {
        if self.update_busy || self.update_ready.is_some() {
            return Ok(());
        }
        let info = self
            .versions
            .known
            .as_ref()
            .ok_or("尚未取得版本資訊，請先重新整理服務。")?;
        if !info.available() {
            self.update_status = "目前已是最新版本。".into();
            return Ok(());
        }
        let latest = info.latest_version.clone();
        if !self.confirm_update()? {
            return Ok(());
        }
        let config = self.config.clone();
        let tx = self.tx.clone();
        self.update_busy = true;
        self.update_status = "正在背景下載並驗證更新…".into();
        crate::reliability::spawn(move || {
            let _ = tx.send(Event::UpdateReady(crate::deployment::download(
                &config, &latest,
            )));
        });
        Ok(())
    }
    pub(super) fn apply_ready_update(&mut self) -> AppResult<()> {
        // 更新時取消本機操作，伺服器持久任務會保留供新版恢復；先保存草稿。
        let Some(ready) = &self.update_ready else {
            return Ok(());
        };
        let latest = self
            .versions
            .known
            .as_ref()
            .ok_or("尚未取得版本資訊。")?
            .latest_version
            .clone();
        crate::deployment::validate_artifact(&self.config, &ready.artifact, &latest)?;
        if ready.artifact.kind == "exe" {
            // 手動更新僅開啟檔案位置；不退出、不執行新版，也不解除強制版本門檻。
            crate::deployment::show_download(ready)?;
            let message = format!(
                "新版位於：\n{}\n\n請從系統托盤選「離開」，再將新版 LM_AI.exe 複製到您原本使用的位置，取代舊檔後重新開啟。聊天、登入與偏好資料會保留。",
                ready.path.display()
            );
            unsafe {
                MessageBoxW(
                    self.window,
                    wide(&message).as_ptr(),
                    wide("手動更新 LM_AI").as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            return Ok(());
        }
        if self.update_consent.as_deref() != Some(ready.artifact.version.as_str()) {
            return Err("更新版本已改變，請重新點擊下載箭頭確認。".into());
        }
        self.preserve_draft()?;
        crate::deployment::launch_update(
            self.update_ready.as_ref().ok_or("更新檔不存在。")?,
            &self.config,
            &latest,
        )?;
        self.update_busy = true;
        self.projects.cancel();
        self.mail_flow.cancel.store(true, Ordering::Relaxed);
        self.cancelled.store(true, Ordering::Relaxed);
        unsafe {
            PostMessageW(self.window, WM_CLOSE, 1, 0);
        }
        Ok(())
    }
}
