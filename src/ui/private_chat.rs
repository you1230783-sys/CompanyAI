//! 私聊只從已授權的原生入口啟動。背景回應以登入世代及操作序號隔離。
use super::*;
use crate::private_chat::{
    self as chat, Account, Api, Attempts, Contacts, Device, Friend, Invitations, LockPolicy,
    Password, Profile, Vault,
};
use windows_sys::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};

const CONTACTS: &str = "contacts.gcm";
const CONTACTS_LIMIT: u64 = 4 * 1024 * 1024;
const REFRESH_SECONDS: u64 = 20;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum Command {
    Enable { enabled: bool },
    Open,
    Leave,
    Lock,
    Policy { policy: LockPolicy },
    Unlock { password: Password },
    ChangePassword { old: Password, new: Password },
    Reset { confirmed: bool },
    Erase { confirmed: bool },
    Refresh,
    CopyId,
    Regenerate,
    Lookup { code: String },
    Invite,
    Answer { id: String, response: Answer },
    Unblock { employee_no: String },
    RemoveFriend { id: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Answer {
    Accept,
    Reject,
    Block,
}

pub(super) enum Reply {
    Activated,
    Verified,
    Unlocked(Vault),
    Lookup(Profile),
    Refreshed(Profile, Vec<Friend>, Invitations),
    PasswordChanged,
    Updated(&'static str),
}

pub(super) struct Runtime {
    account: Option<Account>,
    device: Option<Device>,
    vault: Option<Vault>,
    contacts: Contacts,
    lookup: Option<Profile>,
    attempts: Attempts,
    operation: u64,
    worker: std::sync::Weak<()>,
    busy: bool,
    requested: bool,
    verified: bool,
    suspended: bool,
    next_poll: u64,
    unlocked_at: u64,
    cached_until: Option<u64>,
    left_at: Option<u64>,
    windows_events: bool,
    status: String,
}

impl Runtime {
    pub(super) fn new(window: HWND) -> Self {
        // SAFETY: window 是目前 UI 執行緒剛建立的 HWND。
        let windows_events =
            unsafe { WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) != 0 };
        Self {
            account: None,
            device: None,
            vault: None,
            contacts: Contacts::default(),
            lookup: None,
            attempts: Attempts::default(),
            operation: 0,
            worker: std::sync::Weak::new(),
            busy: false,
            requested: false,
            verified: false,
            suspended: false,
            next_poll: 0,
            unlocked_at: 0,
            cached_until: None,
            left_at: None,
            windows_events,
            status: String::new(),
        }
    }

    pub(super) fn lock(&mut self) -> AppResult<()> {
        self.operation = self.operation.wrapping_add(1);
        self.busy = false;
        self.requested = false;
        self.vault = None;
        self.contacts = Contacts::default();
        self.lookup = None;
        self.unlocked_at = 0;
        self.verified = true;
        self.left_at = None;
        if let Some(account) = &self.account {
            if let Err(error) = account.lock() {
                // 記憶體已上鎖。檔案移除失敗時由下一次 tick 再嘗試。
                self.cached_until = Some(crate::unix_now());
                return Err(error);
            }
        }
        self.cached_until = None;
        Ok(())
    }

    fn active(&self) -> bool {
        self.device.as_ref().is_some_and(|device| device.active)
    }

    fn unlocked(&self) -> AppResult<&Vault> {
        self.vault.as_ref().ok_or_else(|| "請先解鎖私聊。".into())
    }
}

pub(super) fn unregister_windows_lock(window: HWND) {
    // SAFETY: 只解除本視窗註冊，未註冊或已銷毀時 Windows 會回傳失敗。
    unsafe { WTSUnRegisterSessionNotification(window) };
}

impl App {
    fn private_chat_principal(&self) -> Option<&str> {
        if self.smoke
            || self.demo
            || self.versions.blocked()
            || !self.logged_in()
            || !self.config.uses_transport_v2()
            || !self.config.extra_features_enabled
            || !crate::transport::pins::configured().is_ok_and(|pins| pins.is_some())
        {
            return None;
        }
        let caps = self.work.caps.as_ref()?;
        (caps.chat.enabled && caps.principal_id == self.work.store.principal_id)
            .then_some(caps.principal_id.as_str())
    }

    pub(super) fn private_chat_available(&self) -> bool {
        self.private_chat_principal().is_some() && !self.private_chat.suspended
    }

    pub(super) fn private_chat_capability_updated(&mut self) {
        self.private_chat.suspended = false;
    }

    fn publish_private_chat(&self) {
        let state = &self.private_chat;
        let available = self.private_chat_available();
        let enabled = available && self.config.private_chat_enabled && state.active();
        let open = enabled && state.requested && state.vault.is_some();
        let _ = self.view.post(&json!({
            "type": "private_chat_state",
            "available": available, "enabled": enabled, "busy": state.busy,
            "open": open, "authenticate": enabled && state.requested && state.vault.is_none(),
            "has_password": state.account.as_ref().is_some_and(Account::exists),
            "status": state.status,
            "contacts": open.then_some(&state.contacts), "lookup": open.then_some(&state.lookup),
        }));
    }

    pub(super) fn lock_private_chat(&mut self) {
        if let Err(error) = self.private_chat.lock() {
            self.private_chat.status = error;
        }
        self.publish_private_chat();
    }

    fn bind_private_chat(&mut self) -> AppResult<bool> {
        let Some(principal) = self.private_chat_principal() else {
            if self.private_chat.account.is_some() {
                self.private_chat.lock()?;
                self.private_chat.account = None;
                self.private_chat.device = None;
                self.private_chat.verified = false;
                self.private_chat.attempts = Attempts::default();
                return Ok(true);
            }
            return Ok(false);
        };
        let account = Account::new(&self.root, &self.config.binding()?, principal)?;
        if self
            .private_chat
            .account
            .as_ref()
            .is_some_and(|current| current.directory == account.directory)
        {
            return Ok(false);
        }
        self.private_chat.lock()?;
        self.private_chat.device = Device::load(&account)?;
        self.private_chat.account = Some(account);
        self.private_chat.verified = false;
        self.private_chat.next_poll = 0;
        self.private_chat.attempts = Attempts::default();
        Ok(true)
    }

    fn private_chat_start(
        &mut self,
        worker: impl FnOnce(Api) -> Result<Reply, chat::Error> + Send + 'static,
    ) -> AppResult<()> {
        if self.private_chat.busy || self.private_chat.worker.strong_count() != 0 {
            return Err("請等候目前的私聊操作完成。".into());
        }
        let device = self
            .private_chat
            .device
            .as_ref()
            .ok_or("請先啟用私聊裝置。")?;
        let session = self.session.as_ref().ok_or("請先登入。")?;
        let api = Api::new(&self.config, session, &device.id)?;
        self.private_chat.operation = self.private_chat.operation.wrapping_add(1);
        self.private_chat.busy = true;
        let live = Arc::new(());
        self.private_chat.worker = Arc::downgrade(&live);
        let (tx, generation, operation) = (
            self.tx.clone(),
            self.generation,
            self.private_chat.operation,
        );
        crate::reliability::spawn(move || {
            let result = worker(api);
            drop(live);
            let _ = tx.send(Event::PrivateChat(generation, operation, result));
        });
        Ok(())
    }

    fn private_chat_refresh(&mut self) -> AppResult<()> {
        self.private_chat.next_poll = crate::unix_now().saturating_add(REFRESH_SECONDS);
        if self.private_chat.vault.is_none() {
            return self.private_chat_start(|api| {
                api.profile()?;
                Ok(Reply::Verified)
            });
        }
        let blocked = self.private_chat.contacts.blocked.clone();
        self.private_chat_start(move |api| {
            let (profile, friends, invitations) = api.refresh_contacts(&blocked)?;
            Ok(Reply::Refreshed(profile, friends, invitations))
        })
    }

    pub(super) fn private_chat_tick(&mut self) {
        let result = (|| {
            let mut changed = self.bind_private_chat()?;
            let now = crate::unix_now();
            let state = &mut self.private_chat;
            if state.vault.is_some()
                && self
                    .config
                    .private_chat_lock
                    .deadline(state.unlocked_at, state.left_at)
                    .is_some_and(|deadline| now >= deadline)
            {
                state.lock()?;
                changed = true;
            }
            if state.cached_until.is_some_and(|expiry| now >= expiry) {
                if let Some(account) = &state.account {
                    account.lock()?;
                }
                state.cached_until = None;
            }
            if self.private_chat_available()
                && self.config.private_chat_enabled
                && self.private_chat.active()
                && !self.private_chat.busy
                && now >= self.private_chat.next_poll
            {
                self.private_chat_refresh()?;
                changed = true;
            }
            Ok::<bool, String>(changed)
        })();
        match result {
            Ok(true) => self.publish_private_chat(),
            Ok(false) => (),
            Err(error) => {
                if self.private_chat.status != error {
                    self.private_chat.status = error;
                    self.publish_private_chat();
                }
            }
        }
    }

    fn set_private_chat_enabled(&mut self, enabled: bool) -> AppResult<()> {
        let mut config = self.config.clone();
        config.private_chat_enabled = enabled;
        storage::save_config(&self.root, &config)?;
        self.config = config;
        Ok(())
    }

    fn cache_private_chat(&mut self) -> AppResult<()> {
        let state = &mut self.private_chat;
        if let Some(vault) = &state.vault {
            let expiry = self
                .config
                .private_chat_lock
                .cache_deadline(state.unlocked_at, state.left_at);
            vault.cache(state.unlocked_at, expiry)?;
            state.cached_until = Some(expiry);
        }
        Ok(())
    }

    fn load_private_contacts(&mut self, vault: Vault, unlocked_at: u64) -> AppResult<()> {
        self.private_chat.contacts = vault.read(CONTACTS, CONTACTS_LIMIT)?;
        self.private_chat.vault = Some(vault);
        self.private_chat.unlocked_at = unlocked_at;
        self.private_chat.attempts = Attempts::default();
        self.private_chat.next_poll = 0;
        Ok(())
    }

    pub(super) fn private_chat_event(
        &mut self,
        operation: u64,
        result: Result<Reply, chat::Error>,
    ) {
        if operation != self.private_chat.operation
            || self.bind_private_chat().unwrap_or(true)
            || !self.private_chat_available()
        {
            return;
        }
        self.private_chat.busy = false;
        let result =
            result.and_then(|reply| self.apply_private_chat_reply(reply).map_err(Into::into));
        if let Err(error) = result {
            if error.password_failed {
                self.private_chat.attempts.fail(crate::unix_now());
            }
            if error.unavailable {
                self.lock_private_chat();
                self.private_chat.verified = false;
                if error.replaced {
                    if let (Some(device), Some(account)) =
                        (&mut self.private_chat.device, &self.private_chat.account)
                    {
                        device.active = false;
                        if let Err(error) = device.save(account) {
                            self.private_chat.status = error;
                        }
                    }
                    let _ = self.set_private_chat_enabled(false);
                } else {
                    self.private_chat.suspended = true;
                }
            }
            self.private_chat.status = error.message;
        }
        self.publish_private_chat();
    }

    fn apply_private_chat_reply(&mut self, reply: Reply) -> AppResult<()> {
        match reply {
            Reply::Activated => {
                let device = self
                    .private_chat
                    .device
                    .as_mut()
                    .ok_or("找不到私聊裝置。")?;
                device.active = true;
                device.save(
                    self.private_chat
                        .account
                        .as_ref()
                        .ok_or("找不到私聊帳號。")?,
                )?;
                self.set_private_chat_enabled(true)?;
                self.private_chat.status = "此電腦已啟用私聊。".into();
                self.private_chat.next_poll = 0;
            }
            Reply::Verified => {
                if !self.private_chat.verified {
                    self.private_chat.verified = true;
                    let account = self
                        .private_chat
                        .account
                        .as_ref()
                        .ok_or("找不到私聊帳號。")?;
                    if !self.config.private_chat_lock.windows_lock
                        || self.private_chat.windows_events
                    {
                        if let Some((vault, start, expiry)) = account.recover(crate::unix_now())? {
                            self.load_private_contacts(vault, start)?;
                            // 重開不能延長已開始的離開期限。
                            let seconds = u64::from(self.config.private_chat_lock.minutes) * 60;
                            self.private_chat.left_at =
                                Some(crate::unix_now().min(expiry.saturating_sub(seconds)));
                            self.cache_private_chat()?;
                        }
                    } else {
                        account.lock()?;
                    }
                }
            }
            Reply::Unlocked(vault) => {
                self.load_private_contacts(vault, crate::unix_now())?;
                self.private_chat.verified = true;
                self.cache_private_chat()?;
                self.private_chat.status.clear();
            }
            Reply::Lookup(profile) => {
                self.private_chat.lookup = Some(profile);
                self.private_chat.status = "請核對暱稱與工號，再送出邀請。".into();
            }
            Reply::Refreshed(profile, friends, invitations) => {
                let mut contacts = self.private_chat.contacts.clone();
                contacts.profile = Some(profile);
                contacts.merge_friends(friends);
                contacts.invites = invitations;
                self.save_private_contacts(contacts)?;
            }
            Reply::PasswordChanged => {
                self.private_chat.attempts = Attempts::default();
                self.private_chat.status = "聊天密碼已更新，請用新密碼解鎖。".into();
            }
            Reply::Updated(status) => {
                self.private_chat.lookup = None;
                self.private_chat.next_poll = 0;
                self.private_chat.status = status.into();
            }
        }
        Ok(())
    }

    pub(super) fn private_chat_command(&mut self, command: Command) {
        if let Err(error) = self.apply_private_chat_command(command) {
            self.private_chat.status = error;
        }
        self.publish_private_chat();
    }

    fn apply_private_chat_command(&mut self, command: Command) -> AppResult<()> {
        self.bind_private_chat()?;
        let now = crate::unix_now();
        let state = &self.private_chat;
        if state.vault.is_some()
            && self
                .config
                .private_chat_lock
                .deadline(state.unlocked_at, state.left_at)
                .is_some_and(|deadline| now >= deadline)
        {
            self.private_chat.lock()?;
        }
        if matches!(command, Command::Lock | Command::Enable { enabled: false }) {
            self.private_chat.lock()?;
            if matches!(command, Command::Enable { enabled: false }) {
                self.set_private_chat_enabled(false)?;
            }
            return Ok(());
        }
        if !self.private_chat_available() {
            return Err("私聊目前不可用。".into());
        }
        let account = self
            .private_chat
            .account
            .clone()
            .ok_or("找不到私聊帳號。")?;
        if let Command::Enable { enabled: true } = command {
            if self.private_chat.busy {
                return Err("請等候目前的私聊操作完成。".into());
            }
            self.private_chat.device = Some(Device::load_or_create(&account)?);
            return self.private_chat_start(|api| {
                api.activate()?;
                Ok(Reply::Activated)
            });
        }
        if !self.config.private_chat_enabled || !self.private_chat.active() {
            return Err("請先在設定中啟用私聊。".into());
        }
        match command {
            Command::Open => {
                self.private_chat.requested = true;
                self.private_chat.left_at = None;
                self.private_chat.status.clear();
                self.cache_private_chat()?;
            }
            Command::Leave => {
                if matches!(self.config.private_chat_lock.when, chat::LockWhen::Leave)
                    && self.config.private_chat_lock.minutes == 0
                {
                    return self.private_chat.lock();
                }
                self.private_chat.requested = false;
                self.private_chat.lookup = None;
                self.private_chat.left_at = Some(now);
                self.cache_private_chat()?;
            }
            Command::Policy { policy } => {
                if !policy.valid() || policy.windows_lock && !self.private_chat.windows_events {
                    return Err(
                        "此自動上鎖設定無法使用。請檢查時間選項與 Windows 工作階段。".into(),
                    );
                }
                let mut config = self.config.clone();
                config.private_chat_lock = policy;
                storage::save_config(&self.root, &config)?;
                self.config = config;
                self.cache_private_chat()?;
            }
            Command::Unlock { password } => {
                if !self.private_chat.requested || self.private_chat.vault.is_some() {
                    return Err("請從私聊入口開啟密碼視窗。".into());
                }
                if self.config.private_chat_lock.windows_lock && !self.private_chat.windows_events {
                    return Err("目前無法偵測 Windows 鎖定。請重開程式或調整自動上鎖設定。".into());
                }
                self.private_chat.attempts.check(now)?;
                chat::password_rule(password.expose())?;
                // 只為有效密碼輸入排程 KDF；成功會清除累計。
                self.private_chat_start(move |api| {
                    let secret = api.key_secret(false)?;
                    let vault = if account.exists() {
                        account.unlock(password.expose(), &secret)?
                    } else {
                        account.create(password.expose(), &secret)?
                    };
                    Ok(Reply::Unlocked(vault))
                })?;
            }
            Command::ChangePassword { old, new } => {
                if self.private_chat.busy {
                    return Err("請先等候私聊同步完成。".into());
                }
                self.private_chat.attempts.check(now)?;
                chat::password_rule(old.expose())?;
                chat::password_rule(new.expose())?;
                self.private_chat.lock()?;
                self.private_chat_start(move |api| {
                    let secret = api.key_secret(false)?;
                    account.change_password(old.expose(), new.expose(), &secret)?;
                    Ok(Reply::PasswordChanged)
                })?;
            }
            Command::Reset { confirmed: true } => {
                if self.private_chat.busy {
                    return Err("請先等候私聊同步完成。".into());
                }
                self.private_chat.lock()?;
                self.private_chat_start(move |api| {
                    api.key_secret(true)?;
                    account.erase()?;
                    Ok(Reply::Updated("已重設私聊。再次開啟即可設定新密碼。"))
                })?;
            }
            Command::Erase { confirmed: true } => {
                if self.private_chat.busy {
                    return Err("請先等候私聊同步完成。".into());
                }
                self.private_chat.lock()?;
                account.erase()?;
                self.private_chat.status = "已清除本機聊天資料。".into();
            }
            Command::Reset { .. } | Command::Erase { .. } => {
                return Err("請先確認永久清除本機聊天紀錄。".into());
            }
            _ => self.private_contacts_command(command)?,
        }
        Ok(())
    }

    fn save_private_contacts(&mut self, contacts: Contacts) -> AppResult<()> {
        if contacts != self.private_chat.contacts {
            self.private_chat.unlocked()?.write(CONTACTS, &contacts)?;
            self.private_chat.contacts = contacts;
        }
        Ok(())
    }

    fn private_contacts_command(&mut self, command: Command) -> AppResult<()> {
        if self.private_chat.busy || !self.private_chat.requested {
            return Err("請等候私聊畫面完成同步。".into());
        }
        self.private_chat.unlocked()?;
        match command {
            Command::Refresh => self.private_chat_refresh(),
            Command::CopyId => {
                let profile = self
                    .private_chat
                    .contacts
                    .profile
                    .as_ref()
                    .ok_or("聊天 ID 尚未載入。")?;
                selection::copy_text(self.window, &profile.chat_id)?;
                self.private_chat.status = "已複製聊天 ID。".into();
                Ok(())
            }
            Command::Regenerate => self.private_chat_start(|api| {
                api.regenerate()?;
                Ok(Reply::Updated("已重新產生聊天 ID。"))
            }),
            Command::Lookup { code } => {
                self.private_chat.lookup = None;
                self.private_chat_start(move |api| Ok(Reply::Lookup(api.lookup(&code)?)))
            }
            Command::Invite => {
                let profile = self
                    .private_chat
                    .lookup
                    .take()
                    .ok_or("請先搜尋並確認對方。")?;
                if self.private_chat.contacts.is_blocked(&profile.employee_no) {
                    return Err("請先解除對方的黑名單。".into());
                }
                self.private_chat_start(move |api| {
                    api.invite(&profile.chat_id)?;
                    Ok(Reply::Updated("已送出好友邀請。"))
                })
            }
            Command::Answer { id, response } => {
                let invite = self
                    .private_chat
                    .contacts
                    .invites
                    .incoming
                    .iter()
                    .find(|entry| entry.invite_id == id)
                    .ok_or("找不到這則好友邀請。")?;
                let accept = matches!(response, Answer::Accept);
                if matches!(response, Answer::Block) {
                    let mut contacts = self.private_chat.contacts.clone();
                    contacts.blocked.push(Profile {
                        chat_id: String::new(),
                        nickname: invite.nickname.clone(),
                        employee_no: invite.employee_no.clone(),
                    });
                    contacts
                        .invites
                        .incoming
                        .retain(|entry| entry.invite_id != id);
                    self.save_private_contacts(contacts)?;
                }
                self.private_chat_start(move |api| {
                    api.answer(&id, accept)?;
                    Ok(Reply::Updated("已處理好友邀請。"))
                })
            }
            Command::Unblock { employee_no } => {
                let mut contacts = self.private_chat.contacts.clone();
                contacts
                    .blocked
                    .retain(|entry| entry.employee_no != employee_no);
                self.save_private_contacts(contacts)?;
                self.private_chat.status = "已解除黑名單。".into();
                Ok(())
            }
            Command::RemoveFriend { id } => {
                if !self
                    .private_chat
                    .contacts
                    .friends
                    .iter()
                    .any(|entry| entry.friend_id == id && entry.status != "removed")
                {
                    return Err("找不到這位好友。".into());
                }
                self.private_chat_start(move |api| {
                    api.remove_friend(&id)?;
                    Ok(Reply::Updated("已解除好友。"))
                })
            }
            _ => Err("私聊操作無法執行。".into()),
        }
    }
}
