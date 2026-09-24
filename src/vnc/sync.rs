//! 手動登入並保留 Session 供預覽重取，匯入／捨棄後登出；帳密以 DPAPI 保存。
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

mod session;
#[cfg(test)]
use session::download;
pub use session::{run_session, SessionCommand, SessionEvent};

impl Manager {
    /// 依名稱跨分類比對；同名多筆必須全部 IP 相同才可標記一致，避免掩蓋衝突。
    pub fn comparison(&self, remote: &RemoteMachine) -> &'static str {
        let matches: Vec<_> = self
            .machines
            .values()
            .flatten()
            .filter(|m| m.name == remote.name)
            .collect();
        if matches.is_empty() {
            "new"
        } else if matches.iter().all(|m| m.ip == remote.ip) {
            "same"
        } else {
            "changed"
        }
    }

    /// 新機台密碼預設 1234；同來源 ID 優先，找不到識別碼時以全域唯一名稱對應。
    /// 再同步保留既有密碼與個人分類，受影響分類依名稱排序；未勾選或手動新增的項目不刪除。
    pub fn import(&mut self, download: &Download, indices: &[usize]) -> AppResult<usize> {
        let chosen: BTreeSet<_> = indices.iter().copied().collect();
        if chosen.is_empty() {
            return Err("請勾選要匯入的分類或機台。".into());
        }
        let mut machines = self.machines.clone();
        let mut affected = BTreeSet::new();
        let mut updated = BTreeSet::new();
        let mut imported = 0;
        for index in &chosen {
            let remote = download
                .machines
                .get(*index)
                .ok_or("匯入預覽已變更，請重新取得清單。")?;
            // UI 的 disabled 不是唯一防線；偽造或過期的選取也不能重匯完全相同機台。
            if self.comparison(remote) == "same" {
                continue;
            }
            let identity_matches: Vec<_> = machines
                .iter()
                .flat_map(|(group, list)| {
                    list.iter().enumerate().filter_map(move |(i, m)| {
                        let same_id = !remote.id.is_empty()
                            && m.extra.get("_lm_sync_id") == Some(&json!(remote.id))
                            && m.extra.get("_lm_sync_source") == Some(&json!(download.source));
                        same_id.then(|| (group.clone(), i))
                    })
                })
                .collect();
            // 沒有穩定識別碼時，以全域唯一名稱更新，對應預覽的名稱比對規則。
            let matches: Vec<_> = if identity_matches.is_empty() {
                // 名稱備援只比對匯入前的清單，不能把這一批剛新增的同名機台互相覆蓋。
                self.machines
                    .iter()
                    .flat_map(|(group, list)| {
                        list.iter()
                            .enumerate()
                            .filter(|(_, m)| m.name == remote.name)
                            .map(|(i, _)| (group.clone(), i))
                    })
                    .collect()
            } else {
                identity_matches
            };
            if matches.len() > 1 {
                return Err("本機有多筆相同名稱或識別碼，請先整理重複機台再匯入。".into());
            }
            let machine = if let Some((group, i)) = matches.first() {
                if !updated.insert((group.clone(), *i)) {
                    return Err("本次有多台機台對應到同一筆設定，請分別確認後匯入。".into());
                }
                affected.insert(group.clone());
                &mut machines.get_mut(group).ok_or("分類已不存在。")?[*i]
            } else {
                affected.insert(remote.group.clone());
                let list = machines.entry(remote.group.clone()).or_default();
                list.push(Machine {
                    name: String::new(),
                    ip: String::new(),
                    password: "1234".into(),
                    extra: Map::new(),
                });
                list.last_mut().ok_or("無法建立機台。")?
            };
            imported += 1;
            machine.name.clone_from(&remote.name);
            machine.ip.clone_from(&remote.ip);
            if !remote.id.is_empty() {
                machine.extra.insert("_lm_sync_id".into(), json!(remote.id));
                machine
                    .extra
                    .insert("_lm_sync_source".into(), json!(download.source));
            }
        }
        // 明確匯入時排序受影響分類；之後的手動上／下移仍照原方式保存。
        for group in affected {
            machines
                .get_mut(&group)
                .ok_or("分類已不存在。")?
                .sort_by(|a, b| natural_cmp(&a.name, &b.name));
        }
        if imported > 0 {
            self.save_machines(machines)?;
        }
        Ok(imported)
    }
}

#[cfg(test)]
mod tests;
