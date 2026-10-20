//! 同一個背景工作持有網站 Session，直到匯入、捨棄、取消或錯誤才登出。
//! UI 只傳送 Refresh／Close，不接觸 Cookie，也不能在預覽期間更換登入網址。
use super::*;
use std::sync::mpsc::Receiver;

pub enum SessionCommand {
    Refresh,
    Close,
}

pub enum SessionEvent {
    Ready(Download),
    Finished(AppResult<()>),
}

struct Session<'a> {
    settings: &'a Settings,
    cookie: Option<String>,
    cancelled: &'a AtomicBool,
}

impl Session<'_> {
    fn check(&self) -> AppResult<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err("已停止取得機台清單。".into())
        } else {
            Ok(())
        }
    }

    fn login(&mut self) -> AppResult<()> {
        self.settings.validate()?;
        if self.settings.username.trim().is_empty() || self.settings.password.is_empty() {
            return Err("請先輸入機台網站的帳號與密碼。".into());
        }
        self.check()?;
        let home = send(
            self.settings,
            &self.settings.home,
            "GET",
            "",
            &mut self.cookie,
        )?;
        if !(200..400).contains(&home.status) || self.cookie.is_none() {
            return Err("首頁未提供 PHPSESSID，請檢查首頁連結。".into());
        }
        self.check()?;
        let before = self.cookie.clone();
        let form = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("uid", self.settings.username.as_str()),
                ("pwd", self.settings.password.as_str()),
                ("type", "1"),
            ])
            .finish();
        let login = send(
            self.settings,
            &self.settings.login,
            "POST",
            &form,
            &mut self.cookie,
        )?;
        let denied = serde_json::from_str::<Value>(&login.body)
            .ok()
            .is_some_and(|v| v["ok"] == false);
        if !(200..400).contains(&login.status)
            || denied
            || self.cookie.is_none()
            || self.cookie == before
        {
            return Err("登入未取得新的 PHPSESSID，請確認帳密及登入連結。".into());
        }
        self.check()
    }

    /// 每輪重新建立完整快照，不能把第二輪 IP 混入第一輪的分類或索引。
    fn fetch(&mut self) -> AppResult<Download> {
        let mut machines: Vec<RemoteMachine> = Vec::new();
        for path in self
            .settings
            .endpoints
            .iter()
            .filter(|p| !p.trim().is_empty())
        {
            self.check()?;
            let response = send(self.settings, path, "GET", "", &mut self.cookie)?;
            if response.status != 200 {
                return Err(format!(
                    "機台 API 回報 HTTP {}，本機清單未變更。",
                    response.status
                ));
            }
            for machine in parse_machines(&response.body)? {
                let old = machines.iter().find(|m| {
                    if !machine.id.is_empty() {
                        m.id == machine.id
                    } else {
                        m.id.is_empty() && m.group == machine.group && m.name == machine.name
                    }
                });
                if let Some(old) = old {
                    if old.group != machine.group
                        || old.name != machine.name
                        || old.ip != machine.ip
                    {
                        return Err("不同 API 的相同機台資料有衝突，請檢查查詢連結。".into());
                    }
                } else {
                    machines.push(machine);
                }
                if machines.len() > 10_000 {
                    return Err("合併的機台清單超過 10,000 筆。".into());
                }
            }
        }
        self.check()?;
        machines.sort_by(|a, b| {
            group_cmp(&a.group, &b.group).then_with(|| natural_cmp(&a.name, &b.name))
        });
        Ok(Download {
            machines,
            source: self.settings.base()?.to_string(),
            warning: String::new(),
        })
    }

    fn logout(&mut self) -> AppResult<()> {
        if self.cookie.is_none() {
            return Ok(());
        }
        let result = send(
            self.settings,
            &self.settings.logout,
            "POST",
            "",
            &mut self.cookie,
        );
        self.cookie = None;
        match result {
            Ok(r) if (200..400).contains(&r.status) => Ok(()),
            _ => Err("網站登出未成功，請至網站確認登入狀態。".into()),
        }
    }
}

/// 50% 包含剛好一半；空清單不觸發重取。一次登入最多自動重取一次。
fn needs_retry(download: &Download) -> bool {
    !download.machines.is_empty()
        && download
            .machines
            .iter()
            .filter(|m| m.group == "未分類")
            .count()
            * 2
            >= download.machines.len()
}

/// 登入、讀取與登出都在同一工作執行緒，Close 與 Refresh 不會並行使用 Cookie。
pub fn run_session(
    settings: &Settings,
    cancelled: &AtomicBool,
    commands: Receiver<SessionCommand>,
    mut publish: impl FnMut(SessionEvent) -> bool,
) {
    let mut session = Session {
        settings,
        cookie: None,
        cancelled,
    };
    let result = (|| {
        session.login()?;
        let mut download = session.fetch()?;
        if needs_retry(&download) {
            download = session.fetch()?;
            download.warning =
                "未分類達 50%，已自動重新取得一次；如需再查詢，請按「再次取得更新清單」。".into();
        }
        if !publish(SessionEvent::Ready(download)) {
            return Ok(());
        }
        // 沒有計時器；只有使用者的按鈕命令才會再次要求機台 API。
        while let Ok(command) = commands.recv() {
            match command {
                SessionCommand::Close => break,
                SessionCommand::Refresh => {
                    let download = session.fetch()?;
                    if !publish(SessionEvent::Ready(download)) {
                        break;
                    }
                }
            }
        }
        Ok(())
    })();
    let logout = session.logout();
    let result = match (result, logout) {
        (Ok(()), result) => result,
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(logout)) => Err(format!("{error} {logout}")),
    };
    publish(SessionEvent::Finished(result));
}

/// 舊有傳輸測試仍可測試單輪讀取；正式 UI 一律使用 run_session 保留登入。
#[cfg(test)]
pub(super) fn download(settings: &Settings, cancelled: &AtomicBool) -> AppResult<Download> {
    let mut session = Session {
        settings,
        cookie: None,
        cancelled,
    };
    let result = session.login().and_then(|_| session.fetch());
    let logout = session.logout();
    match (result, logout) {
        (Ok(data), Ok(())) => Ok(data),
        (Ok(mut data), Err(error)) => {
            data.warning = error;
            Ok(data)
        }
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(logout)) => Err(format!("{error} {logout}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_threshold_includes_half_but_excludes_empty_and_below_half() {
        for (total, unclassified, expected) in
            [(0, 0, false), (3, 1, false), (2, 1, true), (3, 2, true)]
        {
            let download = Download {
                machines: (0..total)
                    .map(|i| RemoteMachine {
                        id: i.to_string(),
                        name: format!("A01-{i}"),
                        ip: String::new(),
                        group: if i < unclassified { "未分類" } else { "A01" }.into(),
                    })
                    .collect(),
                source: String::new(),
                warning: String::new(),
            };
            assert_eq!(needs_retry(&download), expected);
        }
    }
}
