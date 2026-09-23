//! 使用者手動要求時才登入、取得清單、登出。帳密以目前 Windows 使用者的 DPAPI 保存。
//! 下載結果只暫存在記憶體；使用者勾選後才合併 machines.json，不刪除手動機台。
use super::*;
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};
use url::Url;

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub root: String,
    pub home: String,
    pub login: String,
    pub logout: String,
    pub endpoints: Vec<String>,
    pub username: String,
    pub password: String,
}
impl Default for Settings {
    fn default() -> Self {
        let mut endpoints = vec![String::new(); 10];
        endpoints[0] = "api/machine/info_map.php?floor=2F".into();
        endpoints[1] = "api/machine/info_map.php?floor=4F".into();
        Self {
            root: "http://lmmes.largan.com.tw/transdata".into(),
            home: "?p=machineInfoMap&v=2".into(),
            login: "login.php".into(),
            logout: "logout.php".into(),
            endpoints,
            username: String::new(),
            password: String::new(),
        }
    }
}

impl Settings {
    pub fn base(&self) -> AppResult<Url> {
        let mut base = Url::parse(self.root.trim()).map_err(|_| "根目錄網址格式不正確。")?;
        if !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
        {
            return Err("根目錄須為 HTTP(S) 網址，不可包含帳密、查詢參數或 #。".into());
        }
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }
        Ok(base)
    }
    /// 所有登入／查詢／登出只連至使用者指定的同一網站及根目錄。
    pub fn endpoint(&self, path: &str) -> AppResult<Url> {
        let base = self.base()?;
        let path = path.trim();
        let lower = path.to_ascii_lowercase();
        if path.len() > 2048
            || path.starts_with('/')
            || path.contains(['\\', '#'])
            || lower.contains("%2f")
            || lower.contains("%5c")
            || path.chars().any(char::is_control)
        {
            return Err("連結請填根目錄下的相對路徑，可包含 ? 查詢參數。".into());
        }
        let url = base.join(path).map_err(|_| "相對連結格式不正確。")?;
        if url.origin() != base.origin()
            || !url.path().starts_with(base.path())
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("連結不得離開設定的網站根目錄。".into());
        }
        Ok(url)
    }
    pub fn validate(&self) -> AppResult<()> {
        if self.root.len() > 2048
            || self.username.len() > 256
            || self.password.len() > 1024
            || self.username.chars().any(char::is_control)
            || self.password.contains('\0')
        {
            return Err("同步設定或帳密長度／格式不正確。".into());
        }
        if self.endpoints.len() != 10
            || self.login.trim().is_empty()
            || self.logout.trim().is_empty()
            || self.endpoints.iter().all(|p| p.trim().is_empty())
        {
            return Err("請設定登入、登出及至少一個機台 API；最多提供十個 API 欄位。".into());
        }
        for path in [&self.home, &self.login, &self.logout]
            .into_iter()
            .chain(self.endpoints.iter().filter(|p| !p.trim().is_empty()))
        {
            self.endpoint(path)?;
        }
        Ok(())
    }
    /// 密碼不回傳 UI；留空可沿用已存值。
    pub fn public(&self) -> Value {
        json!({"root":self.root,"home":self.home,"login":self.login,"logout":self.logout,"endpoints":self.endpoints,"username":self.username,"has_password":!self.password.is_empty()})
    }
    pub fn load(root: &Path) -> AppResult<Self> {
        let Some(bytes) = read_optional(&root.join("vnc-sync.dpapi"))? else {
            return Ok(Self::default());
        };
        let settings: Self = serde_json::from_slice(&storage::protect(&bytes, false)?)
            .map_err(|_| "機台網站登入設定無法讀取。")?;
        settings.validate()?;
        Ok(settings)
    }
    pub fn save(&self, root: &Path) -> AppResult<()> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| "同步設定無法保存。")?;
        storage::atomic_write(
            &root.join("vnc-sync.dpapi"),
            &storage::protect(&bytes, true)?,
        )
    }
}

#[derive(Clone, Serialize)]
pub struct RemoteMachine {
    pub id: String,
    pub group: String,
    pub name: String,
    pub ip: String,
}

pub struct Download {
    pub machines: Vec<RemoteMachine>,
    pub source: String,
    pub warning: String,
}

fn field(value: &Value, name: &str) -> AppResult<String> {
    match &value[name] {
        Value::Null => Ok(String::new()),
        Value::String(s) => Ok(s.trim().to_string()),
        Value::Number(n) if name == "machine_id" => Ok(n.to_string()),
        _ => Err(format!("機台資料的 {name} 型別不正確，尚未變更本機清單。")),
    }
}

fn parse_machines(body: &str) -> AppResult<Vec<RemoteMachine>> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| "機台 API 未回傳有效 JSON，可能登入已失效。")?;
    if value["ok"] != true || value["code"] != 0 {
        return Err("機台 API 回報失敗，請確認帳密及查詢權限。".into());
    }
    let list = value["data"]["machines"]
        .as_array()
        .ok_or("機台 API 缺少 data.machines 陣列。")?;
    if list.len() > 10_000 {
        return Err("單次機台清單超過 10,000 筆。".into());
    }
    let mut machines = Vec::new();
    for value in list {
        if !value.is_object() {
            return Err("機台清單含無效項目。".into());
        }
        let name = field(value, "machine_name")?;
        if name.is_empty() {
            continue;
        }
        let mut group = field(value, "eq_type")?;
        if group.is_empty() {
            group = "未分類".into();
        }
        let ip = field(value, "machine_ip")?;
        let id = field(value, "machine_id")?;
        if id.len() > 256 || id.chars().any(char::is_control) {
            return Err("機台識別碼格式不正確。".into());
        }
        validate_machine(&group, &name, &ip, "1234")?;
        machines.push(RemoteMachine {
            id,
            group,
            name,
            ip,
        });
    }
    Ok(machines)
}

/// 每次送出前等待 300ms；只執行一次指定流程，不排程自動同步或自動重試登入。
fn send(
    settings: &Settings,
    path: &str,
    method: &str,
    body: &str,
    cookie: &mut Option<String>,
) -> AppResult<crate::transport::vnc::Response> {
    thread::sleep(Duration::from_millis(300));
    crate::transport::vnc::request(&settings.endpoint(path)?, method, body, cookie)
}

pub fn download(settings: &Settings, cancelled: &AtomicBool) -> AppResult<Download> {
    settings.validate()?;
    if settings.username.trim().is_empty() || settings.password.is_empty() {
        return Err("請先輸入機台網站的帳號與密碼。".into());
    }
    let mut cookie = None;
    let received = (|| {
        let check = || {
            if cancelled.load(Ordering::Relaxed) {
                Err("已停止取得機台清單。".to_string())
            } else {
                Ok(())
            }
        };
        check()?;
        let home = send(settings, &settings.home, "GET", "", &mut cookie)?;
        if !(200..400).contains(&home.status) || cookie.is_none() {
            return Err("首頁未提供 PHPSESSID，請檢查首頁連結。".into());
        }
        check()?;
        let before = cookie.clone();
        let form = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("uid", settings.username.as_str()),
                ("pwd", settings.password.as_str()),
                ("type", "1"),
            ])
            .finish();
        let login = send(settings, &settings.login, "POST", &form, &mut cookie)?;
        let denied = serde_json::from_str::<Value>(&login.body)
            .ok()
            .is_some_and(|v| v["ok"] == false);
        if !(200..400).contains(&login.status) || denied || cookie.is_none() || cookie == before {
            return Err("登入未取得新的 PHPSESSID，請確認帳密及登入連結。".into());
        }
        let mut machines: Vec<RemoteMachine> = Vec::new();
        for path in settings.endpoints.iter().filter(|p| !p.trim().is_empty()) {
            check()?;
            let response = send(settings, path, "GET", "", &mut cookie)?;
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
        check()?;
        Ok(machines)
    })();
    // 已取得 session 後，任何成功／失敗／取消路徑都嘗試登出；不持久保存 Cookie。
    let logout = if cookie.is_some() {
        send(settings, &settings.logout, "POST", "", &mut cookie).and_then(|r| {
            if (200..400).contains(&r.status) {
                Ok(())
            } else {
                Err("登出未成功。".into())
            }
        })
    } else {
        Ok(())
    };
    let warning = if logout.is_err() {
        "網站登出未成功，請至網站確認登入狀態。".to_string()
    } else {
        String::new()
    };
    match received {
        Ok(_) if cancelled.load(Ordering::Relaxed) => {
            Err(format!("已停止取得機台清單。 {warning}"))
        }
        Ok(machines) => Ok(Download {
            machines,
            source: settings.base()?.to_string(),
            warning,
        }),
        Err(error) => Err(if warning.is_empty() {
            error
        } else {
            format!("{error} {warning}")
        }),
    }
}

impl Manager {
    /// 新機台密碼預設 1234；同來源 ID 優先，舊檔則以分類＋唯一名稱對應。
    /// 再同步保留既有密碼、個人分類及順序；未勾選或手動新增的項目不刪除。
    pub fn import(&mut self, download: &Download, indices: &[usize]) -> AppResult<usize> {
        let chosen: BTreeSet<_> = indices.iter().copied().collect();
        if chosen.is_empty() {
            return Err("請勾選要匯入的分類或機台。".into());
        }
        let mut machines = self.machines.clone();
        for index in &chosen {
            let remote = download
                .machines
                .get(*index)
                .ok_or("匯入預覽已變更，請重新取得清單。")?;
            let matches: Vec<_> = machines
                .iter()
                .flat_map(|(group, list)| {
                    list.iter().enumerate().filter_map(move |(i, m)| {
                        let same_id = !remote.id.is_empty()
                            && m.extra.get("_lm_sync_id") == Some(&json!(remote.id))
                            && m.extra.get("_lm_sync_source") == Some(&json!(download.source));
                        let legacy = m.extra.get("_lm_sync_id").is_none()
                            && group == &remote.group
                            && m.name == remote.name;
                        (same_id || legacy).then(|| (group.clone(), i))
                    })
                })
                .collect();
            if matches.len() > 1 {
                return Err("本機有多筆相同名稱或識別碼，請先整理重複機台再匯入。".into());
            }
            let machine = if let Some((group, i)) = matches.first() {
                &mut machines.get_mut(group).ok_or("分類已不存在。")?[*i]
            } else {
                let list = machines.entry(remote.group.clone()).or_default();
                list.push(Machine {
                    name: String::new(),
                    ip: String::new(),
                    password: "1234".into(),
                    extra: Map::new(),
                });
                list.last_mut().ok_or("無法建立機台。")?
            };
            machine.name.clone_from(&remote.name);
            machine.ip.clone_from(&remote.ip);
            if !remote.id.is_empty() {
                machine.extra.insert("_lm_sync_id".into(), json!(remote.id));
                machine
                    .extra
                    .insert("_lm_sync_source".into(), json!(download.source));
            }
        }
        self.save_machines(machines)?;
        Ok(chosen.len())
    }
}

#[cfg(test)]
mod tests;
