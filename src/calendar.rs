//! 日期依據統一來自 Windows 本機時鐘；不從週別、文件日期或模型記憶反推今天。
use chrono::{Datelike, Duration, NaiveDate};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;

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
