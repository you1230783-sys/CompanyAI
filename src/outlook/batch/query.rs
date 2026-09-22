//! 跨 Exchange／PST 的有界唯讀查詢。只使用 Outlook 已載入的 Store，絕不掃描磁碟。
use super::*;
use std::{
    collections::{HashSet, VecDeque},
    time::{Duration as StdDuration, Instant},
};

const MAX_FOLDERS: usize = 500;
const MAX_ITEMS: usize = 10_000;
const MAX_SECONDS: u64 = 30;

/// 舊版訊息未指定 scope 時保留收件匣範圍；目前介面預設明確傳入 current_folder。
/// AllStores 保留既有查詢能力，但不再提供使用者介面選項。
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchScope {
    #[default]
    Inbox,
    CurrentFolder,
    AllStores,
}

struct Candidate {
    received: f64,
    mail: Mail,
}

/// 各資料夾依時間遞減提供候選，但合併後仍須重新排序，不能先到先取 50 封。
fn retain_latest(candidates: &mut Vec<Candidate>, candidate: Candidate) -> bool {
    candidates.push(candidate);
    candidates.sort_by(|a, b| b.received.total_cmp(&a.received));
    if candidates.len() > MAX_MAILS {
        candidates.pop();
        true
    } else {
        false
    }
}

struct Search<'a> {
    cancel: &'a AtomicBool,
    started: Instant,
    folders: usize,
    items: usize,
    limited: bool,
    failures: usize,
    first_error: Option<String>,
    queue: VecDeque<IDispatch>,
    visited: HashSet<(String, String)>,
    seen_mail: HashSet<(String, String)>,
    excluded: HashSet<(String, String)>,
    candidates: Vec<Candidate>,
    truncated: bool,
}

fn count(collection: &IDispatch) -> AppResult<i32> {
    i32::try_from(&get(collection, "Count", &mut [])?)
        .map_err(|_| "無法讀取 Outlook 集合數量。".into())
}

fn item(collection: &IDispatch, index: i32) -> AppResult<IDispatch> {
    object(&get(collection, "Item", &mut [VARIANT::from(index)])?)
}

fn folder_key(folder: &IDispatch) -> AppResult<(String, String)> {
    Ok((
        text(folder, "StoreID", 4096)?,
        text(folder, "EntryID", 4096)?,
    ))
}

impl Search<'_> {
    /// 限時在 COM 呼叫間檢查；Outlook 正在等待對話框時不能強行中斷該次呼叫。
    fn can_continue(&mut self) -> AppResult<bool> {
        check_cancel(self.cancel)?;
        if self.started.elapsed() >= StdDuration::from_secs(MAX_SECONDS) || self.items >= MAX_ITEMS
        {
            self.limited = true;
        }
        Ok(!self.limited)
    }

    fn failure(&mut self, error: String) {
        self.failures += 1;
        if self.first_error.is_none() {
            self.first_error = Some(error);
        }
    }

    fn enqueue(&mut self, folder: IDispatch) {
        if self.queue.len() + self.folders < MAX_FOLDERS {
            self.queue.push_back(folder);
        } else {
            // 仍處理已排入的資料夾，但告知使用者有其他資料夾未被搜尋。
            self.truncated = true;
        }
    }

    fn enqueue_children(&mut self, parent: &IDispatch) -> AppResult<()> {
        let children = object(&get(parent, "Folders", &mut [])?)?;
        for index in 1..=count(&children)? {
            if !self.can_continue()? {
                break;
            }
            if self.queue.len() + self.folders >= MAX_FOLDERS {
                self.truncated = true;
                break;
            }
            match item(&children, index) {
                Ok(child) => self.enqueue(child),
                Err(error) => self.failure(error),
            }
        }
        Ok(())
    }

    /// 以 Outlook 的資料夾 ID 排除系統資料夾，不以中文／英文名稱猜測。
    /// PST 不一定有全部預設資料夾；不存在者正常略過，不建立任何新資料夾。
    fn exclude_system_folders(&mut self, store: &IDispatch) -> AppResult<()> {
        // Deleted Items、Outbox、Sent Mail、Drafts、Junk、Sync Issues。
        for kind in [3i32, 4, 5, 16, 23, 20] {
            if !self.can_continue()? {
                break;
            }
            match get(store, "GetDefaultFolder", &mut [VARIANT::from(kind)]) {
                Ok(value) => {
                    if let Ok(folder) = object(&value) {
                        self.excluded.insert(folder_key(&folder)?);
                    }
                }
                // 找不到預設資料夾時 Outlook 回傳 Nothing；真正呼叫失敗則不可默默當成不存在。
                Err(error) => self.failure(format!(
                    "無法確認部分系統資料夾的排除範圍，結果可能包含該資料夾。{error}"
                )),
            }
        }
        Ok(())
    }

    fn scan_folder(
        &mut self,
        folder: &IDispatch,
        start: NaiveDate,
        end: NaiveDate,
        unread: bool,
    ) -> AppResult<()> {
        // 行事曆等資料夾不讀 Items，但其子資料夾仍由外層遞迴檢查。
        if i32::try_from(&get(folder, "DefaultItemType", &mut [])?)
            .map_err(|_| "無法辨識資料夾的預設項目類型。")?
            != 0
        {
            return Ok(());
        }
        let mut items = object(&get(folder, "Items", &mut [])?)?;
        if unread {
            items = object(&get(
                &items,
                "Restrict",
                &mut [VARIANT::from("[UnRead] = True")],
            )?)?;
        }
        // COM 參數逆序。數字 OLE DATE 在本機比較，避開地區日期字串解析差異。
        get(
            &items,
            "Sort",
            &mut [VARIANT::from(true), VARIANT::from("[ReceivedTime]")],
        )?;
        let mut matched = 0;
        for index in 1..=count(&items)? {
            if !self.can_continue()? {
                break;
            }
            self.items += 1;
            let mail = match item(&items, index) {
                Ok(mail) => mail,
                Err(error) => {
                    self.failure(error);
                    continue;
                }
            };
            let outcome = (|| -> AppResult<bool> {
                if i32::try_from(&get(&mail, "Class", &mut [])?)
                    .map_err(|_| "無法辨識郵件類型。")?
                    != 43
                {
                    return Ok(false);
                }
                let received = received_time(&mail)?;
                let day = received_date(received)?;
                if day < start {
                    return Ok(true);
                }
                if day > end {
                    return Ok(false);
                }
                let key = (
                    text(folder, "StoreID", 4096)?,
                    text(&mail, "EntryID", 4096)?,
                );
                if !self.seen_mail.insert(key) {
                    return Ok(false);
                }
                matched += 1;
                // 單一資料夾第 51 封後不可能進入全域前 50，無須讀取更多基本資訊。
                if matched > MAX_MAILS {
                    self.truncated = true;
                    return Ok(true);
                }
                self.truncated |= retain_latest(
                    &mut self.candidates,
                    Candidate {
                        received,
                        mail: snapshot(&mail)?,
                    },
                );
                Ok(false)
            })();
            match outcome {
                Ok(true) => break,
                Ok(false) => (),
                Err(error) => self.failure(error),
            }
        }
        Ok(())
    }
}

pub(super) fn list(
    app: &IDispatch,
    period: &str,
    unread: bool,
    scope: SearchScope,
    cancel: &AtomicBool,
) -> AppResult<MailList> {
    let end = today()?;
    let start = cutoff(end, period)?;
    let mut search = Search {
        cancel,
        started: Instant::now(),
        folders: 0,
        items: 0,
        limited: false,
        failures: 0,
        first_error: None,
        queue: VecDeque::new(),
        visited: HashSet::new(),
        seen_mail: HashSet::new(),
        excluded: HashSet::new(),
        candidates: Vec::new(),
        truncated: false,
    };
    check_cancel(cancel)?;
    let namespace = object(&get(app, "GetNamespace", &mut [VARIANT::from("MAPI")])?)?;
    let label = match scope {
        SearchScope::Inbox => {
            let folder = object(&get(
                &namespace,
                "GetDefaultFolder",
                &mut [VARIANT::from(6i32)],
            )?)?;
            let path = text(&folder, "FolderPath", 4096)?;
            search.enqueue(folder);
            format!("預設收件匣及子資料夾：{path}")
        }
        SearchScope::CurrentFolder => {
            let explorer = object(&get(app, "ActiveExplorer", &mut [])?)
                .map_err(|_| "請先在 Outlook 郵件清單切換到要查詢的資料夾。")?;
            let folder = object(&get(&explorer, "CurrentFolder", &mut [])?)?;
            let path = text(&folder, "FolderPath", 4096)?;
            search.enqueue(folder);
            format!("目前資料夾及子資料夾：{path}")
        }
        SearchScope::AllStores => {
            let stores = object(&get(&namespace, "Stores", &mut [])?)?;
            for index in 1..=count(&stores)? {
                if !search.can_continue()? {
                    break;
                }
                if index as usize > MAX_FOLDERS {
                    search.truncated = true;
                    break;
                }
                let result = (|| -> AppResult<()> {
                    let store = item(&stores, index)?;
                    search.exclude_system_folders(&store)?;
                    search.enqueue(object(&get(&store, "GetRootFolder", &mut [])?)?);
                    Ok(())
                })();
                if let Err(error) = result {
                    search.failure(error);
                }
            }
            "所有已載入信箱與本機資料檔（含子資料夾）".into()
        }
    };
    while let Some(folder) = search.queue.pop_front() {
        if !search.can_continue()? {
            break;
        }
        search.folders += 1;
        let result = (|| -> AppResult<()> {
            let key = folder_key(&folder)?;
            if search.excluded.contains(&key) || !search.visited.insert(key) {
                return Ok(());
            }
            if matches!(scope, SearchScope::AllStores) {
                // 搜尋資料夾是虛擬結果，略過以避免重複掃描及重新引入已排除的郵件。
                let accessor = object(&get(&folder, "PropertyAccessor", &mut [])?)?;
                let kind = get(
                    &accessor,
                    "GetProperty",
                    &mut [VARIANT::from(
                        "http://schemas.microsoft.com/mapi/proptag/0x36010003",
                    )],
                )?;
                if i32::try_from(&kind).map_err(|_| "無法辨識 Outlook 資料夾類型。")? == 2
                {
                    return Ok(());
                }
            }
            // 子資料夾列舉失敗仍嘗試目前資料夾，並保留不完整提示。
            if let Err(error) = search.enqueue_children(&folder) {
                search.failure(error);
            }
            if search.can_continue()? {
                search.scan_folder(&folder, start, end, unread)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            search.failure(error);
        }
    }
    check_cancel(cancel)?;
    let mut notice = format!(
        "已檢查 {} 個資料夾、{} 個項目。",
        search.folders, search.items
    );
    if search.limited || search.truncated {
        notice.push_str("結果已截斷；每批最多 50 封，查詢上限為 500 個資料夾、10,000 個項目或 30 秒。可改查目前資料夾以縮小範圍。");
    }
    if let Some(error) = search.first_error {
        notice.push_str(&format!(
            "有 {} 次讀取失敗，結果不完整。首個錯誤：{error}",
            search.failures
        ));
    }
    if search.candidates.is_empty() {
        notice.push_str("此範圍內未取得符合條件的郵件；請確認資料檔已在 Outlook 開啟，並檢查查詢範圍與同步狀態。");
    }
    Ok(MailList {
        mails: search
            .candidates
            .into_iter()
            .map(|candidate| candidate.mail)
            .collect(),
        scope: label,
        truncated: search.truncated || search.limited || search.failures > 0,
        notice,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_folder_can_replace_all_earlier_results_and_keeps_time_of_day() {
        let mut candidates = Vec::new();
        for index in 0..120 {
            let received = 46_282.0 + index as f64 / 200.0;
            let mail = Mail {
                id: index.to_string(),
                folder: "測試".into(),
                preview: demo_mail(false),
                store_id: index.to_string(),
            };
            assert_eq!(
                retain_latest(&mut candidates, Candidate { received, mail }),
                index >= 50
            );
        }
        assert_eq!(candidates.len(), 50);
        assert_eq!(candidates[0].mail.id, "119");
        assert_eq!(candidates[49].mail.id, "70");
        assert!(candidates
            .windows(2)
            .all(|pair| pair[0].received >= pair[1].received));
    }

    #[test]
    fn calendar_ranges_cross_month_and_year_without_parsing_display_dates() {
        let day = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(
            cutoff(day, "three_days").unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 30).unwrap()
        );
        assert_eq!(
            cutoff(day, "week").unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 29).unwrap()
        );
        assert_eq!(
            received_date(46_283.999).unwrap(),
            NaiveDate::from_ymd_opt(2026, 9, 18).unwrap()
        );
        assert!(serde_json::from_str::<SearchScope>("\"disk_path\"").is_err());
    }
}

#[cfg(test)]
#[path = "query_tests.rs"]
mod com_tests;
