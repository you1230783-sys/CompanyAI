//! Office 關閉後，ROT 可能短暫仍列出已斷線的物件；只重新查詢占用狀態，不重播文件操作。
use super::*;
use windows::core::Interface;

/// 每次都重新取得 ROT 物件。只有確認沒有實例或文件數為零，才允許建立自己的實例。
pub(super) fn ensure_idle(class: &GUID, collection: &str, program: &str) -> AppResult<()> {
    check_with_retry(
        program,
        10,
        || {
            let mut active = None;
            match unsafe { windows::Win32::System::Ole::GetActiveObject(class, None, &mut active) }
            {
                Ok(()) => {
                    let app: IDispatch = active
                        .ok_or("Office 實例查詢回傳空值。")?
                        .cast()
                        .map_err(|e| e.to_string())?;
                    integer(&child(&app, collection)?, "Count").map(Some)
                }
                // MK_E_UNAVAILABLE 表示 ROT 中沒有登錄實例，並非略過查詢失敗。
                Err(error) if error.code().0 == 0x800401E3u32 as i32 => Ok(None),
                Err(error) => Err(error.to_string()),
            }
        },
        || std::thread::sleep(std::time::Duration::from_millis(100)),
    )
}

/// 有限重查只涵蓋唯讀的實例／文件數查詢；持續失敗仍拒絕，已確認占用則立即回傳。
fn check_with_retry(
    program: &str,
    attempts: usize,
    mut query: impl FnMut() -> AppResult<Option<i32>>,
    mut wait: impl FnMut(),
) -> AppResult<()> {
    for attempt in 0..attempts {
        let error = match query() {
            Ok(None | Some(0)) => return Ok(()),
            Ok(Some(count)) if count > 0 => {
                return Err(format!(
                    "{} {program} Office 正在使用中，請先儲存並關閉相關視窗後重試。",
                    super::super::interaction::BUSY
                ));
            }
            Ok(Some(_)) => "Office 文件數無效。".to_owned(),
            Err(error) => error,
        };
        if attempt + 1 == attempts {
            return Err(format!("無法確認 {program} 是否占用：{error}"));
        }
        wait();
    }
    Err("Office 占用查詢次數不可為零。".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn disconnected_rot_is_requeried_until_confirmed_idle() {
        for idle in [None, Some(0)] {
            let mut replies = VecDeque::from([Err("0x80010108".into()), Ok(idle)]);
            let mut waits = 0;
            check_with_retry("Excel", 10, || replies.pop_front().unwrap(), || waits += 1).unwrap();
            assert!(replies.is_empty());
            assert_eq!(waits, 1);
        }
    }

    #[test]
    fn busy_or_persistent_query_failure_never_becomes_idle() {
        let mut waits = 0;
        let mut replies = VecDeque::from([Err("已斷線".into()), Ok(Some(1))]);
        let busy = check_with_retry("Word", 10, || replies.pop_front().unwrap(), || waits += 1)
            .unwrap_err();
        assert!(busy.contains(super::super::super::interaction::BUSY));
        assert_eq!(waits, 1);
        let mut calls = 0;
        let failure = check_with_retry(
            "Excel",
            3,
            || {
                calls += 1;
                Err("拒絕存取".into())
            },
            || {},
        )
        .unwrap_err();
        assert!(failure.contains("拒絕存取"));
        assert_eq!(calls, 3);
    }
}
