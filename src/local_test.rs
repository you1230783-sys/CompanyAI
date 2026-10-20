//! 明確命令列啟用的 localhost 驗證入口；不從正式偏好檔覆寫公司連線。
use crate::{config::Config, protocol::TokenResponse, storage::Session, AppResult};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Connection {
    origin: String,
    token: String,
}

pub struct LocalTest {
    pub config: Config,
    pub session: Session,
    pub root: PathBuf,
}

impl LocalTest {
    /// 連線檔由本機服務產生；只接受明確 IPv4 loopback 與獨立測試資料夾。
    pub fn load(connection: &Path, root: &Path) -> AppResult<Self> {
        let bytes = std::fs::read(connection).map_err(|e| e.to_string())?;
        if bytes.len() > 4096 {
            return Err("本機測試連線檔過大".into());
        }
        let connection: Connection = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let url = url::Url::parse(&connection.origin).map_err(|e| e.to_string())?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.port().is_none()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || connection.token.len() < 20
        {
            return Err("本機測試僅接受 http://127.0.0.1:<port> 與測試授權碼".into());
        }
        if !root.is_absolute() || root.exists() {
            return Err("請指定尚未存在的絕對路徑作為隔離測試資料夾".into());
        }
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let config = Config {
            server_url: connection.origin,
            model: "quality".into(),
            debug_mode: true,
            hotkey_enabled: false,
            selection_icon: false,
            ..Config::default()
        };
        let session = Session::from_token(
            TokenResponse {
                access_token: connection.token,
                token_type: "Bearer".into(),
                expires_in: 86400,
            },
            &config,
        )?;
        // 準備獨立專案；GUI 可直接選取並送出合成測試要求。
        let workspace = root.join("workspace");
        std::fs::create_dir(&workspace).map_err(|e| e.to_string())?;
        let mut store = crate::projects::Store::default();
        store.add("Luna 本機 CAD 測試", workspace)?;
        store.save(root)?;
        Ok(Self {
            config,
            session,
            root: root.to_path_buf(),
        })
    }

    /// 無人值守測試同樣走正式 runner／Broker／CAD worker，不用合成工具回覆。
    pub fn probe(self, request_file: &Path) -> AppResult<()> {
        use crate::projects::runner::{self, Run};
        use std::sync::{atomic::AtomicBool, Arc};
        let request = std::fs::read_to_string(request_file).map_err(|e| e.to_string())?;
        let id = crate::jobs::new_id()?;
        let project = crate::projects::Store::load(&self.root)?.projects.remove(0);
        let mut events = Vec::new();
        let result = runner::run(
            Run {
                id: id.clone(),
                resume: false,
                project,
                conversation: crate::jobs::new_id()?,
                messages: vec![crate::protocol::Message::user(&request)],
                config: self.config,
                session: self.session,
                root: self.root.clone(),
                cancel: Arc::new(AtomicBool::new(false)),
                instructions: None,
                outlook_consent: None,
                // 無人值守只讀測試不自動同意 CAD 修改或重新連線。
                cad_waiter: None,
                cad_progress: None,
                file_waiter: None,
            },
            |event| {
                events.push(event);
            },
        );
        crate::storage::atomic_write(
            &self.root.join("result.json"),
            &serde_json::to_vec_pretty(&serde_json::json!({"run_id":id,"events":events,
                "result":result,"native_runner":true}))
            .map_err(|e| e.to_string())?,
        )?;
        result.map(|_| ())
    }
}
