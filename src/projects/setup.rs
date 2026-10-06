//! 使用者主動建立專案／週報素材目錄。這些操作不公告為模型工具。
use super::{files, Project};
use crate::AppResult;
use chrono::{Datelike, Duration, NaiveDate};
use std::{
    fs,
    path::{Component, Path, PathBuf, Prefix},
};

/// 只接受磁碟或 UNC 檔案路徑；Windows 回傳的延伸前綴轉回一般路徑。
/// 不接受裝置命名空間或上層跳轉，UNC 的 server/share 不可省略。
pub fn normal_path(path: &Path) -> AppResult<PathBuf> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("請使用完整資料夾路徑，不可包含上層跳轉。".into());
    }
    let mut parts = path.components();
    let Some(Component::Prefix(prefix)) = parts.next() else {
        return Err("缺少磁碟或網路分享位置。".into());
    };
    let mut result = match prefix.kind() {
        Prefix::Disk(disk) | Prefix::VerbatimDisk(disk) => {
            PathBuf::from(format!("{}:\\", disk as char))
        }
        Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share)
            if !server.is_empty() && !share.is_empty() =>
        {
            PathBuf::from(format!(
                "\\\\{}\\{}\\",
                server.to_string_lossy(),
                share.to_string_lossy()
            ))
        }
        _ => return Err("不接受裝置路徑或不完整的網路分享路徑。".into()),
    };
    for part in parts {
        match part {
            Component::RootDir => (),
            Component::Normal(name) => result.push(name),
            _ => return Err("資料夾路徑包含不支援的元件。".into()),
        }
    }
    Ok(result)
}

/// 讀取 Windows 本機日期，與 Outlook／使用者桌面時區一致，不依 UTC 猜週次。
pub fn local_date() -> AppResult<NaiveDate> {
    let mut time = windows_sys::Win32::Foundation::SYSTEMTIME::default();
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time) };
    NaiveDate::from_ymd_opt(time.wYear.into(), time.wMonth.into(), time.wDay.into())
        .ok_or_else(|| "無法取得目前日期。".into())
}

/// 回傳所選日期那一週的週一；跨月、跨年沿用日曆運算。
pub fn monday(today: NaiveDate) -> NaiveDate {
    today - Duration::days(today.weekday().num_days_from_monday().into())
}

/// 以原子 create_dir 避免重名覆寫；父目錄不存在時不擅自改存其他位置。
pub fn create_unique(parent: &Path, stem: &str) -> AppResult<PathBuf> {
    let parent = normal_path(parent)?;
    let _guards = files::pin(&parent)?;
    if !parent.is_dir() || files::relative(stem)?.components().count() != 1 {
        return Err("無法確認要建立資料夾的位置或名稱。".into());
    }
    for number in 1..=10000 {
        let name = if number == 1 {
            stem.into()
        } else {
            format!("{stem}_{number}")
        };
        let path = parent.join(name);
        match fs::create_dir(&path) {
            Ok(()) => {
                files::validate_root(&path)?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("無法建立資料夾，請確認網路連線及寫入權限：{error}")),
        }
    }
    Err("同名資料夾過多，請改用其他位置。".into())
}

/// 使用目前（含公司重新導向）的 Known Folder，不拼接 C 槽或預設使用者路徑。
pub fn create_default(location: &str) -> AppResult<PathBuf> {
    use windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_Desktop, FOLDERID_Downloads, SHGetKnownFolderPath, KF_FLAG_DEFAULT},
    };
    let id = match location {
        "downloads" => &FOLDERID_Downloads,
        "desktop" => &FOLDERID_Desktop,
        _ => return Err("位置只能選擇下載或桌面。".into()),
    };
    let value = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }
        .map_err(|e| format!("無法取得 Windows 資料夾位置：{e}"))?;
    let path = unsafe { value.to_string() };
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    let parent = PathBuf::from(path.map_err(|e| e.to_string())?);
    create_unique(
        &parent,
        &format!("LM_AI專案資料夾_{}", local_date()?.format("%Y%m%d")),
    )
}

/// 建立本次週報專用目錄；同一秒重複建立時仍不覆寫。
pub fn create_weekly(project: &Project) -> AppResult<PathBuf> {
    files::validate_root(&project.root)?;
    let mut time = windows_sys::Win32::Foundation::SYSTEMTIME::default();
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time) };
    create_unique(
        &project.root,
        &format!(
            "週報生成使用資料_{:04}{:02}{:02}_{:02}{:02}{:02}",
            time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
        ),
    )
}

/// 日期及素材位置由原生層產生；使用者明確週次／日期要求優先於預設範圍。
pub fn weekly_prompt(
    folder: &str,
    today: NaiveDate,
    start: &str,
    end: &str,
    notes: &str,
) -> AppResult<String> {
    files::relative(folder)?;
    let from = NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|_| "週報開始日期不正確。")?;
    let to = NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|_| "週報結束日期不正確。")?;
    if from > to || (to - from).num_days() > 366 || notes.encode_utf16().count() > 12000 {
        return Err("請確認週報日期順序、範圍不超過一年，補充內容不超過 12000 字元。".into());
    }
    Ok(format!("請生成 Word 週報並交付檔案。\n\
        目前 Windows 本機日期：{today}；ISO 週別：{}-W{:02}。\n\
        預設／畫面所選週報日期（含首尾）：{from} 至 {to}。使用者若明確要求上週、W40 或其他日期／週次，優先依該要求換算，必要時確認年份；不得把今天的週別直接當成指定週別。\n\
        本次參考資料位於專案相對資料夾：{folder:?}。先載入 weekly-update 並列出此資料夾，優先以其中上週週報作為格式參考；沒有參考週報則建立簡單週報，不杜撰既有格式。素材不足時可使用已授權郵件，沒有新資料就明確說明。避免遍歷其他不相關的專案舊資料，來源檔案唯讀。\n\
        讀取 Outlook 前仍需使用者勾選資料夾。檢查本地 PST 收信、Exchange／OST 線上收件匣（含子資料夾）及寄件備份；只用勾選範圍。先標題與本機前文比對，再讀最新信及必要補充；本機比對最多 1000 封、AI 內文最多 50 封，不需用滿額度。\n\
        保留來源、作者與日期；舊週報是格式及背景，不能當作本週成果。依 PDF／Office／文字素材及郵件整理已完成、進行中、待辦與待確認事項。資料足夠就製作並儲存 Word 週報，不反覆查回相同操作。\n\
        使用者補充（可空白）：\n\
        {}", today.iso_week().year(), today.iso_week().week(), notes.trim()))
}

/// Outlook 助理與週報共用郵件工具；這個入口只改變任務目標。
pub fn outlook_prompt(today: NaiveDate, start: &str, end: &str, notes: &str) -> AppResult<String> {
    let from = NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|_| "開始日期不正確。")?;
    let to = NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|_| "結束日期不正確。")?;
    if from > to || (to - from).num_days() > 366 || notes.chars().count() > 1000 {
        return Err("請確認日期順序、範圍不超過一年，補充最多1000字。".into());
    }
    Ok(format!("請擔任 Outlook 助理，查看使用者勾選的信件，整理需要我處理、回覆或追蹤的事項。\n目前本機日期：{today}，ISO週別：{}-W{:02}；畫面選定日期（含首尾）：{from}至{to}。補充若明確指定上週、W40或其他日期，依明確要求換算。\n先載入 outlook-research，再經原有資料夾勾選授權；涵蓋本地PST、線上收件匣及子資料夾、寄件備份，只挑相關範圍。優先同串最新信，必要時用本機前文比對補讀；最多1000封本機比對、50封AI內文，不讀滿額度。\n輸出待辦表：需要處理的事項、對方要求、期限（僅有明確證據才填日期）、已知狀態與建議下一步；附主旨、日期、寄件者、mail_id。區分明確要求與推測，不把未讀當未完成，也不因有回覆就認定完成；無法確認則列待確認。僅標題不能當作讀過內文。不寄信、不修改信件、不自動建立或刪除文件；使用者後續可要求加入週報。\n使用者補充：\n{}", today.iso_week().year(), today.iso_week().week(), notes.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outlook_prompt_preserves_dates_and_mail_boundaries() {
        let today = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();
        let prompt = outlook_prompt(today, "2026-12-28", "2027-01-01", "請改做 W40").unwrap();
        for expected in [
            "2026-W53",
            "W40",
            "outlook-research",
            "1000",
            "50",
            "不寄信",
            "只挑相關範圍",
        ] {
            assert!(prompt.contains(expected));
        }
        assert!(outlook_prompt(today, "2027-01-02", "2027-01-01", "").is_err());
        assert!(outlook_prompt(today, "2026-01-01", "2027-01-03", "").is_err());
        assert!(outlook_prompt(today, "2027-01-01", "2027-01-01", &"字".repeat(1001)).is_err());
    }
    #[test]
    fn paths_and_week_defaults_are_explicit() {
        assert_eq!(
            normal_path(Path::new(r"\\?\UNC\server\share\專案")).unwrap(),
            PathBuf::from(r"\\server\share\專案")
        );
        assert_eq!(
            normal_path(Path::new(r"F:\Downloads\專案")).unwrap(),
            PathBuf::from(r"F:\Downloads\專案")
        );
        for bad in [r"\\.\pipe\name", r"C:\a\..\b", r"relative\path"] {
            assert!(normal_path(Path::new(bad)).is_err());
        }
        let today = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();
        assert_eq!(monday(today).to_string(), "2026-12-28");
        let prompt = weekly_prompt("素材", today, "2026-12-28", "2027-01-01", "改做 W40").unwrap();
        assert!(prompt.contains("2026-W53"));
        assert!(prompt.contains("改做 W40"));
        assert!(weekly_prompt("../外部", today, "2026-12-28", "2027-01-01", "").is_err());
        assert!(weekly_prompt("素材", today, "2027-01-02", "2027-01-01", "").is_err());
    }
}
