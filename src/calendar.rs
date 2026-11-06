//! 日期依據統一來自 Windows 本機時鐘；不從週別、文件日期或模型記憶反推今天。
use chrono::{Datelike, Duration, NaiveDate};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;

/// 顯示用時間在事件發生時擷取並隨歷史保存；不加入模型上下文。
pub fn local_timestamp() -> String {
    let mut local = windows_sys::Win32::Foundation::SYSTEMTIME::default();
    unsafe { GetLocalTime(&mut local) };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        local.wYear, local.wMonth, local.wDay, local.wHour, local.wMinute, local.wSecond
    )
}

/// 舊版活動是字串，新版保留事件的本機時間；讀取舊紀錄不補造時間。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Activity {
    Retry {
        text: String,
        at: String,
        retry: RetryActivity,
    },
    Timed {
        text: String,
        at: String,
        #[serde(default)]
        model_reply: bool,
    },
    Legacy(String),
}

/// 一次等待的識別與截止時間。前端可從時鐘重算倒數，原生只在狀態變更時發送。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RetryActivity {
    pub id: String,
    pub attempt: usize,
    pub limit: usize,
    pub continuous: bool,
    pub reason: String,
    pub total_seconds: u64,
    pub due_at_millis: u64,
    pub waiting: bool,
}

impl Activity {
    pub fn retry(retry: RetryActivity, text: &str) -> Self {
        Self::Retry {
            text: text.into(),
            at: local_timestamp(),
            retry,
        }
    }

    pub fn retry_info(&self) -> Option<&RetryActivity> {
        match self {
            Self::Retry { retry, .. } => Some(retry),
            _ => None,
        }
    }

    /// 結果更新原活動列。保存時可略過尚未結束的倒數，它可從 checkpoint 還原。
    pub(crate) fn apply(&self, history: &mut Vec<Self>, live: bool) {
        if let Some(retry) = self.retry_info() {
            if retry.waiting && !live {
                return;
            }
            if let Some(previous) = history
                .iter_mut()
                .find(|entry| entry.retry_info().is_some_and(|entry| entry.id == retry.id))
            {
                *previous = self.clone();
                return;
            }
        } else if history
            .last()
            .is_some_and(|last| last.as_str() == self.as_str())
        {
            return;
        }
        history.push(self.clone());
    }
    /// 明確標記已收到模型內容；排隊、讀檔及連線訊息不可切換成工作動作。
    pub fn model_reply() -> Self {
        Self::Timed {
            text: "已收到AI回覆，開始處理。".into(),
            at: local_timestamp(),
            model_reply: true,
        }
    }
    pub fn is_model_reply(&self) -> bool {
        matches!(
            self,
            Self::Timed {
                model_reply: true,
                ..
            }
        )
    }
    /// 提供純文字給既有狀態比對；时间另留在序列化事件中。
    pub fn as_str(&self) -> &str {
        self
    }
}
impl std::ops::Deref for Activity {
    type Target = str;
    fn deref(&self) -> &str {
        match self {
            Self::Retry { text, .. } | Self::Timed { text, .. } | Self::Legacy(text) => text,
        }
    }
}
impl From<String> for Activity {
    fn from(text: String) -> Self {
        Self::Timed {
            text,
            at: local_timestamp(),
            model_reply: false,
        }
    }
}
impl From<&str> for Activity {
    fn from(text: &str) -> Self {
        text.to_owned().into()
    }
}
impl PartialEq<str> for Activity {
    fn eq(&self, other: &str) -> bool {
        &**self == other
    }
}

/// 回傳 Windows 本機時區的公曆日期；失敗不以 UTC 或模型日期猜補。
pub fn today() -> crate::AppResult<NaiveDate> {
    let mut local = windows_sys::Win32::Foundation::SYSTEMTIME::default();
    unsafe { GetLocalTime(&mut local) };
    // 日期無效時明確回報，不猜日期或退回模型內建時鐘。
    NaiveDate::from_ymd_opt(local.wYear.into(), local.wMonth.into(), local.wDay.into())
        .ok_or_else(|| "無法取得有效的 Windows 本機日期。".into())
}

/// 日期只有每日變動，避免時間戳每秒打散請求的可重用前綴。
/// 任務已確定的日期範圍保持不變，跨午夜續跑不能把原本的今天偷偷換成新的一天。
pub fn context(started: Option<&str>) -> crate::AppResult<String> {
    Ok(context_at(today()?, started))
}
fn context_at(date: NaiveDate, started: Option<&str>) -> String {
    let monday = date - Duration::days(date.weekday().num_days_from_monday().into());
    let sunday = monday + Duration::days(6);
    let weekday =
        ["一", "二", "三", "四", "五", "六", "日"][date.weekday().num_days_from_monday() as usize];
    let week = date.iso_week();
    let origin = started
        .map(|d| {
            format!(
                "本任務最初提問日期：{d}；最初要求中的今天依此日解讀，已確定範圍不隨續跑跨日變動。"
            )
        })
        .unwrap_or_default();
    format!("桌面日期依據（Windows 本機時區）：今天={date}，星期{weekday}；ISO週={}-W{:02}；本週一至日={monday}～{sunday}。{origin}使用者明確指定日期優先；文件日期與下週日期不能當今天，不以模型內建日期猜測。", week.year(), week.week())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thursday_is_current_week_not_next_week() {
        let text = context_at(NaiveDate::from_ymd_opt(2026, 10, 8).unwrap(), None);
        assert!(text.contains("今天=2026-10-08，星期四"));
        assert!(text.contains("2026-10-05～2026-10-11"));
        assert!(!text.contains("2026-10-15"));
    }
    #[test]
    fn iso_year_and_midnight_keep_original_task_date() {
        let text = context_at(
            NaiveDate::from_ymd_opt(2027, 1, 1).unwrap(),
            Some("2026-12-31"),
        );
        assert!(text.contains("2026-W53"));
        assert!(text.contains("2026-12-28～2027-01-03"));
        assert!(text.contains("最初提問日期：2026-12-31"));
    }
}
