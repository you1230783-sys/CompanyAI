//! 晚間批次只保存使用者明確排入的專案要求。派送先落盤，再啟動原生專案 runner。
use crate::{jobs, storage, AppResult};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::SYSTEMTIME,
    System::{SystemInformation::GetLocalTime, Time::TzSpecificLocalTimeToSystemTime},
};

pub const START_HOUR: u32 = 21;
const START_JITTER_MINUTES: u32 = 30;
pub const END_HOUR: u32 = 7;
const NIGHT_JITTER_MINUTES: u32 = 10;
pub const OFFICE_RECHECK_MINUTES: i64 = 10;
pub const WAIT_OUTLOOK: &str = "晚間批次等待 Outlook 授權";
pub const WAIT_CAD: &str = "晚間批次等待 CAD 連線確認";
const MAX_PENDING: usize = 20;
const MAX_HISTORY: usize = 100;
const MAX_BYTES: usize = 2_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Queued,
    Running,
    OfficeWait,
    WaitingUser,
    Completed,
    Failed,
}

impl State {
    pub fn pending(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::OfficeWait)
    }
}

/// 只接受原生 runner 保存的結束分類。舊紀錄缺少細分類時保守列為未完成。
pub fn result_state(recovery: &serde_json::Value) -> State {
    match recovery["batch_state"].as_str() {
        Some("completed") => State::Completed,
        Some("waiting_user") => State::WaitingUser,
        Some("office_wait") => State::OfficeWait,
        Some(_) => State::Failed,
        None => match recovery["state"].as_str() {
            Some("completed") => State::Completed,
            Some("waiting_user") => State::WaitingUser,
            _ => State::Failed,
        },
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub conversation: String,
    pub project: String,
    pub root_identity: [u32; 3],
    pub title: String,
    /// Draft 保存原始要求，續接則只依原任務 ID 和 checkpoint，不重新送出原文。
    pub draft: Option<String>,
    pub run_id: String,
    pub plan_mode: bool,
    pub resume: bool,
    pub state: State,
    pub ready: NaiveDateTime,
    pub window: Option<NaiveDate>,
    pub detail: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Queue {
    pub entries: Vec<Entry>,
    pub seen: Option<NaiveDate>,
    #[serde(default)]
    pub notified: Option<NaiveDate>,
}

fn queue_path(root: &Path, principal: &str) -> PathBuf {
    root.join(format!("batch-{}.dpapi", super::text::revision(principal)))
}

impl Queue {
    pub fn load(root: &Path, principal: &str) -> AppResult<Self> {
        let path = queue_path(root, principal);
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => return Err(format!("無法讀取晚間批次佇列：{error}")),
        };
        if bytes.len() > MAX_BYTES {
            return Err("晚間批次佇列超過容量。".into());
        }
        let bytes = storage::protect(&bytes, false)?;
        let queue: Self = serde_json::from_slice(&bytes).map_err(|_| "無法讀取晚間批次佇列。")?;
        queue.validate()?;
        Ok(queue)
    }

    fn validate(&self) -> AppResult<()> {
        let mut ids = std::collections::BTreeSet::new();
        if self.pending_count() > MAX_PENDING
            || self.entries.len() > MAX_HISTORY + MAX_PENDING
            || self
                .entries
                .iter()
                .filter(|entry| entry.state == State::Running)
                .count()
                > 1
        {
            return Err("晚間批次佇列狀態超過限制。".into());
        }
        for entry in &self.entries {
            for id in [
                &entry.id,
                &entry.conversation,
                &entry.project,
                &entry.run_id,
            ] {
                jobs::validate_id(id)?;
            }
            if !ids.insert(&entry.id) || (!entry.resume && entry.draft.is_none()) {
                return Err("晚間批次佇列的任務或識別碼無效。".into());
            }
        }
        Ok(())
    }

    pub fn save(&self, root: &Path, principal: &str) -> AppResult<()> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() >= MAX_BYTES - 1024 {
            return Err("晚間批次佇列資料超過容量。".into());
        }
        storage::atomic_write(
            &queue_path(root, principal),
            &storage::protect(&bytes, true)?,
        )
    }

    pub fn pending_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.state.pending())
            .count()
    }

    pub fn enqueue(&mut self, entry: Entry) -> AppResult<()> {
        if self.pending_count() >= MAX_PENDING {
            return Err("晚間批次最多可排入 20 項等待或執行中的任務。".into());
        }
        if self
            .entries
            .iter()
            .any(|old| old.conversation == entry.conversation && old.state.pending())
        {
            return Err("此對話已有晚間批次任務，可到佇列修改或取消。".into());
        }
        if self.entries.len() >= MAX_HISTORY + MAX_PENDING {
            let index = self
                .entries
                .iter()
                .position(|entry| !entry.state.pending())
                .ok_or("晚間批次佇列已滿。")?;
            self.entries.remove(index);
        }
        self.entries.push(entry);
        Ok(())
    }

    pub fn editable(&self, id: &str) -> AppResult<usize> {
        self.entries
            .iter()
            .position(|entry| {
                entry.id == id && entry.state == State::Queued && entry.window.is_none()
            })
            .ok_or_else(|| "批次任務已開始或已從佇列移除。".into())
    }

    /// 單元測試中沒有其他忙碌對話的派送情境。
    #[cfg(test)]
    pub fn next_ready(&self, now: NaiveDateTime) -> Option<usize> {
        self.next_ready_for(now, |_| false)
    }

    /// 忙碌對話及等待檔案的項目暫時讓位；可執行的新草稿仍維持佇列順序。
    pub fn next_ready_for(&self, now: NaiveDateTime, busy: impl Fn(&str) -> bool) -> Option<usize> {
        if self
            .entries
            .iter()
            .any(|entry| entry.state == State::Running)
            || active_window(now).is_none()
        {
            return None;
        }
        let index = self.entries.iter().position(|entry| {
            !busy(&entry.conversation)
                && (entry.state == State::Queued
                    || (entry.state == State::OfficeWait && entry.ready <= now))
        })?;
        (self.entries[index].ready <= now).then_some(index)
    }

    /// 錯過原窗口的等待項目重新抽籤，避免下晚全部在 21:00 整同時到期。
    /// 連續開啟、跨數晚才重開，以及午夜後重開，都使用同一個窗口身分。
    pub fn roll_over(
        &mut self,
        now: NaiveDateTime,
        mut jitter: impl FnMut() -> AppResult<u32>,
    ) -> AppResult<bool> {
        let target = active_window(now).unwrap_or(now.date());
        let mut changed = false;
        for entry in &mut self.entries {
            if matches!(entry.state, State::Queued | State::OfficeWait)
                && active_window(entry.ready).is_none_or(|old| old < target)
            {
                entry.ready = at(target, START_HOUR)
                    + Duration::minutes(i64::from(jitter()? % (START_JITTER_MINUTES + 1)));
                // 若開啟時已過抽籤區間，用當晚加入規則再錯開 0～10 分鐘。
                if entry.ready < now {
                    entry.ready = ready_at(now, jitter()?);
                }
                changed = true;
            }
        }
        Ok(changed)
    }
}

fn at(date: NaiveDate, hour: u32) -> NaiveDateTime {
    date.and_hms_opt(hour, 0, 0).expect("有效的固定批次時間")
}

/// 午夜前後共用以開始日期識別的窗口。07:00 起不再啟動新任務。
pub fn active_window(now: NaiveDateTime) -> Option<NaiveDate> {
    match now.time().hour() {
        hour if hour >= START_HOUR => Some(now.date()),
        hour if hour < END_HOUR => now.date().pred_opt(),
        _ => None,
    }
}

pub fn window_end(window: NaiveDate) -> NaiveDateTime {
    at(window.succ_opt().expect("Windows 日期範圍"), END_HOUR)
}

/// 隨機值由呼叫端提供，測試可直接控制白天、夜間及跨午夜的選擇。
pub fn ready_at(now: NaiveDateTime, jitter: u32) -> NaiveDateTime {
    if let Some(window) = active_window(now) {
        let due = now + Duration::minutes(i64::from(jitter % (NIGHT_JITTER_MINUTES + 1)));
        if due < window_end(window) {
            return due;
        }
    }
    next_evening(now, jitter)
}

pub fn next_evening(now: NaiveDateTime, jitter: u32) -> NaiveDateTime {
    let date = if now.time().hour() >= START_HOUR {
        now.date().succ_opt().expect("Windows 日期範圍")
    } else {
        now.date()
    };
    at(date, START_HOUR) + Duration::minutes(i64::from(jitter % (START_JITTER_MINUTES + 1)))
}

pub fn local_now() -> AppResult<NaiveDateTime> {
    let mut time = SYSTEMTIME::default();
    unsafe { GetLocalTime(&mut time) };
    from_system_time(&time).ok_or_else(|| "無法取得有效的 Windows 本機時間。".into())
}

fn from_system_time(time: &SYSTEMTIME) -> Option<NaiveDateTime> {
    let date = NaiveDate::from_ymd_opt(time.wYear.into(), time.wMonth.into(), time.wDay.into())?;
    let clock =
        NaiveTime::from_hms_opt(time.wHour.into(), time.wMinute.into(), time.wSecond.into())?;
    Some(date.and_time(clock))
}

/// 以 Windows 當地時區將窗口結束時間換成 UTC，再和登入到期時間比較。
pub fn unix_window_end(ready: NaiveDateTime) -> AppResult<u64> {
    let end = window_end(active_window(ready).ok_or("派送時間不在晚間批次窗口。")?);
    let local = SYSTEMTIME {
        wYear: end.year() as u16,
        wMonth: end.month() as u16,
        wDay: end.day() as u16,
        wHour: end.hour() as u16,
        ..Default::default()
    };
    let mut utc = SYSTEMTIME::default();
    if unsafe { TzSpecificLocalTimeToSystemTime(std::ptr::null(), &local, &mut utc) } == 0 {
        return Err("無法換算晚間批次的登入有效期限。".into());
    }
    from_system_time(&utc)
        .and_then(|time| u64::try_from(time.and_utc().timestamp()).ok())
        .ok_or_else(|| "批次窗口的 UTC 時間無效。".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time(value: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn missed_nights_redraw_once_and_busy_conversation_yields() {
        let mut queue = Queue::default();
        let old = time("2026-11-06 21:10");
        queue.enqueue(entry("first", old)).unwrap();
        queue.enqueue(entry("second", old)).unwrap();
        let now = time("2026-11-07 21:00");
        assert!(queue.roll_over(now, || Ok(25)).unwrap());
        assert_eq!(queue.entries[0].ready, time("2026-11-07 21:25"));
        assert!(!queue.roll_over(now, || panic!("同一晚不可重抽")).unwrap());
        assert_eq!(queue.next_ready(now), None);
        assert_eq!(
            queue.next_ready_for(time("2026-11-07 21:30"), |chat| chat == "chat_first"),
            Some(1)
        );
        assert!(queue.roll_over(time("2026-11-09 23:00"), || Ok(5)).unwrap());
        assert_eq!(queue.entries[0].ready, time("2026-11-09 23:05"));
    }

    #[test]
    fn results_use_native_classification_and_unknown_pauses_are_incomplete() {
        use serde_json::json;
        for (state, expected) in [
            ("office_wait", State::OfficeWait),
            ("waiting_user", State::WaitingUser),
            ("failed", State::Failed),
            ("completed", State::Completed),
        ] {
            assert_eq!(
                result_state(&json!({"batch_state":state,"text":"任意顯示文字"})),
                expected
            );
        }
        assert_eq!(
            result_state(&json!({"state":"paused","text":"已自動重試 50 次"})),
            State::Failed
        );
        assert_eq!(
            result_state(&json!({"state":"paused","text":"時間上限"})),
            State::Failed
        );
        assert_eq!(
            result_state(&json!({"state":"paused","text":"需要你的補充："})),
            State::Failed
        );
    }

    #[test]
    fn windows_cover_midnight_and_skip_the_morning_boundary() {
        let evening = time("2026-11-06 21:00");
        assert_eq!(
            ready_at(time("2026-11-06 14:00"), 30),
            time("2026-11-06 21:30")
        );
        assert_eq!(
            ready_at(time("2026-11-06 23:59"), 10),
            time("2026-11-07 00:09")
        );
        assert_eq!(
            active_window(time("2026-11-07 06:59")),
            Some(evening.date())
        );
        assert_eq!(active_window(time("2026-11-07 07:00")), None);
        assert_eq!(
            ready_at(time("2026-11-07 06:59"), 10),
            time("2026-11-07 21:10")
        );
        assert_eq!(
            ready_at(time("2026-11-07 07:00"), 0),
            time("2026-11-07 21:00")
        );
    }

    #[test]
    fn repeated_clock_hour_keeps_the_same_night() {
        let before = time("2026-11-01 01:59");
        let repeated = time("2026-11-01 01:00");
        assert_eq!(active_window(before), active_window(repeated));
        assert_eq!(
            window_end(active_window(repeated).unwrap()),
            time("2026-11-01 07:00")
        );
    }

    fn entry(id: &str, ready: NaiveDateTime) -> Entry {
        Entry {
            id: id.into(),
            conversation: format!("chat_{id}"),
            project: "project".into(),
            root_identity: [1, 2, 3],
            title: "合成的私人草稿".into(),
            draft: Some("合成的私人草稿".into()),
            run_id: id.into(),
            plan_mode: false,
            resume: false,
            state: State::Queued,
            ready,
            window: None,
            detail: String::new(),
        }
    }

    #[test]
    fn queue_roundtrips_in_dpapi_and_enforces_single_dispatch_and_owner_isolation() {
        let root = std::env::temp_dir().join(format!("batch-test-{}", jobs::new_id().unwrap()));
        let now = time("2026-11-06 21:20");
        let mut queue = Queue::default();
        queue
            .enqueue(entry("first", time("2026-11-06 21:25")))
            .unwrap();
        queue.enqueue(entry("second", now)).unwrap();
        assert!(queue.enqueue(entry("second", now)).is_err());
        assert_eq!(queue.next_ready(now), None);
        queue.entries[0].state = State::OfficeWait;
        assert_eq!(queue.next_ready(now), Some(1));
        queue.entries[1].state = State::Running;
        assert_eq!(queue.next_ready(now), None);
        queue.save(&root, "owner-a").unwrap();
        let ciphertext = fs::read(queue_path(&root, "owner-a")).unwrap();
        assert!(!ciphertext
            .windows("合成的私人草稿".len())
            .any(|slice| slice == "合成的私人草稿".as_bytes()));
        let reopened = Queue::load(&root, "owner-a").unwrap();
        assert_eq!(reopened.entries[1].state, State::Running);
        assert_eq!(Queue::load(&root, "owner-b").unwrap().pending_count(), 0);
        assert_eq!(reopened.next_ready(time("2026-11-07 07:00")), None);
    }
}
