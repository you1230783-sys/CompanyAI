//! 私聊訊息輪詢與畫面命令。只有解鎖且資料已落盤的收件才會送 ack。
use super::*;
use crate::{
    notification_preferences::{Category, Delivery},
    private_chat::{
        messages::{self, History, Kind, Payload, Period, State},
        IncomingMessage, Sent, Status,
    },
    ui::popup::Target,
};

const PAGE_SIZE: usize = 30;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(in crate::ui) enum Command {
    Select {
        peer: String,
    },
    Page {
        older: bool,
    },
    Space,
    SendText {
        peer: String,
        text: String,
    },
    ChooseImage {
        peer: String,
    },
    PasteImage {
        peer: String,
    },
    DiscardImage,
    SendImage {
        peer: String,
    },
    Retry {
        id: String,
    },
    ShowImage {
        id: String,
    },
    Remove {
        id: String,
        confirmed: bool,
    },
    Cleanup {
        period: Period,
        only_images: bool,
        confirmed: bool,
    },
}

pub(in crate::ui) enum Reply {
    Status(Status),
    Incoming(Option<IncomingMessage>),
    Acknowledged,
    Sent(String, Result<Sent, chat::Error>),
    ImageDraft(String, Payload),
}

#[derive(Default)]
pub(super) struct Runtime {
    history: History,
    peer: Option<String>,
    page: usize,
    draft_image: Option<Payload>,
    need_fetch: bool,
    last_message_count: u32,
    last_invite_count: u32,
    pub(super) next_contacts: u64,
}

#[derive(Debug, Eq, PartialEq)]
enum Poll {
    Status,
    Contacts,
    Receive,
}

impl Runtime {
    fn due(&self, now: u64, heartbeat: u64, unlocked: bool) -> Option<Poll> {
        if now >= heartbeat {
            Some(Poll::Status)
        } else if !unlocked {
            None
        } else if now >= self.next_contacts {
            Some(Poll::Contacts)
        } else if self.need_fetch {
            Some(Poll::Receive)
        } else {
            None
        }
    }

    pub(super) fn lock(&mut self) {
        self.history = History::default();
        self.leave();
        self.need_fetch = false;
    }

    pub(super) fn leave(&mut self) {
        self.peer = None;
        self.page = 0;
        self.draft_image = None;
    }

    pub(super) fn load(&mut self, vault: &Vault) -> AppResult<()> {
        let mut history = History::load(vault)?;
        let staged = vault.account.read_receipts()?;
        if !staged.is_empty() {
            history.apply_receipts(vault, &staged)?;
            vault.account.clear_receipts()?;
        }
        self.history = history;
        self.next_contacts = 0;
        self.need_fetch = true;
        Ok(())
    }
}

impl App {
    pub(super) fn poll_private_messages(&mut self, now: u64) -> AppResult<bool> {
        let state = &self.private_chat;
        if !self.private_chat_effective() || state.busy || state.worker.strong_count() != 0 {
            return Ok(false);
        }
        match state
            .messaging
            .due(now, state.next_poll, state.vault.is_some())
        {
            Some(Poll::Status) => {
                let account = state.account.clone().ok_or("找不到私聊帳號。")?;
                self.private_chat.next_poll = now.saturating_add(REFRESH_SECONDS);
                self.private_chat_start(move |api| {
                    let status = api.status()?;
                    // 即使 UI 同時上鎖，已取得的回條也應可在下次解鎖後合併。
                    account.stage_receipts(&status.receipts)?;
                    Ok(super::Reply::Message(Reply::Status(status)))
                })?;
            }
            Some(Poll::Contacts) => self.private_chat_refresh()?,
            Some(Poll::Receive) => {
                self.private_chat.messaging.need_fetch = false;
                self.private_chat_start(|api| {
                    Ok(super::Reply::Message(Reply::Incoming(
                        api.incoming_message()?,
                    )))
                })?;
            }
            None => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn private_friend_invite_notice(&mut self, new_invite: bool) {
        if new_invite && !self.private_chat.requested {
            self.popup(
                Category::ChatInvite,
                "收到私聊好友邀請。",
                Delivery::Windows,
                Target::Window,
            );
        }
    }

    pub(super) fn apply_private_message_reply(&mut self, reply: Reply) -> AppResult<()> {
        match reply {
            Reply::Status(status) => self.apply_private_status(status)?,
            Reply::Incoming(Some(message)) => self.accept_private_message(message)?,
            Reply::Incoming(None) => self.private_chat.messaging.need_fetch = false,
            Reply::Acknowledged => {
                let messaging = &mut self.private_chat.messaging;
                messaging.last_message_count = messaging.last_message_count.saturating_sub(1);
                messaging.need_fetch = true;
            }
            Reply::Sent(id, result) => {
                let state = &mut self.private_chat;
                let vault = state.vault.as_ref().ok_or("私聊已上鎖。")?;
                match result {
                    Ok(sent) => state
                        .messaging
                        .history
                        .mark_sent(vault, &id, &sent.server_time)?,
                    Err(error) => state.messaging.history.reject(vault, &id, error.message)?,
                }
                self.post_private_timeline()?;
            }
            Reply::ImageDraft(peer, payload) => {
                if self.private_chat.requested
                    && self.private_chat.messaging.peer.as_deref() == Some(&peer)
                {
                    if let Payload::Image(bytes) = &payload {
                        let size = bytes.len();
                        self.private_chat.messaging.draft_image = Some(payload);
                        self.view.post(
                            &json!({"type":"private_chat_image_draft","peer":peer,"bytes":size}),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn apply_private_status(&mut self, status: Status) -> AppResult<()> {
        if !self.private_chat.verified {
            self.apply_private_chat_reply(super::Reply::Verified)?;
        }
        let state = &mut self.private_chat;
        let messaging = &mut state.messaging;
        let safe_locked_notice = state.vault.is_none()
            && state
                .account
                .as_ref()
                .is_some_and(Account::locked_notices_safe);
        let notify_messages =
            safe_locked_notice && status.pending_messages > messaging.last_message_count;
        let notify_invites =
            safe_locked_notice && status.pending_invites > messaging.last_invite_count;
        messaging.last_message_count = status.pending_messages;
        messaging.last_invite_count = status.pending_invites;
        if let Some(vault) = &state.vault {
            let staged = vault.account.read_receipts()?;
            if !staged.is_empty() {
                messaging.history.apply_receipts(vault, &staged)?;
                vault.account.clear_receipts()?;
                self.post_private_timeline()?;
            }
            self.private_chat.messaging.need_fetch = true;
        }
        if notify_messages {
            self.popup(
                Category::ChatMessage,
                "收到新訊息。",
                Delivery::Windows,
                Target::Window,
            );
        }
        if notify_invites {
            self.private_friend_invite_notice(true);
        }
        Ok(())
    }

    fn accept_private_message(&mut self, message: IncomingMessage) -> AppResult<()> {
        let state = &mut self.private_chat;
        let vault = state.vault.as_ref().ok_or("私聊已上鎖。")?;
        let blocked = state
            .contacts
            .friends
            .iter()
            .find(|friend| friend.friend_id == message.from_friend_id)
            .is_some_and(|friend| state.contacts.is_blocked(&friend.employee_no));
        let current_thread =
            state.requested && state.messaging.peer.as_deref() == Some(&message.from_friend_id);
        let (ack, payload, added) =
            message.persist(&mut state.messaging.history, vault, blocked)?;
        if self
            .config
            .private_chat_lock
            .deadline(state.unlocked_at, state.left_at)
            .is_some_and(|deadline| crate::unix_now() >= deadline)
        {
            // 轉檔及寫入可能跨過上鎖期限。下次解鎖會重抓並去重後再確認。
            self.lock_private_chat();
            return Ok(());
        }
        self.private_chat_start(move |api| {
            api.ack_message(ack)?;
            Ok(super::Reply::Message(Reply::Acknowledged))
        })?;
        if added {
            self.post_private_timeline()?;
            if !current_thread || !self.is_foreground() {
                let mut detail: String = if self.config.notification_preferences.chat_preview {
                    match &payload {
                        Some(Payload::Text(text)) => text.chars().take(140).collect(),
                        Some(Payload::Image(_)) => "收到新圖片。".into(),
                        None => "收到已過期的訊息紀錄。".into(),
                    }
                } else {
                    "收到新訊息。".into()
                };
                self.popup(
                    Category::ChatMessage,
                    &detail,
                    Delivery::Windows,
                    Target::Window,
                );
                chat::clear_text(&mut detail);
            }
        }
        Ok(())
    }

    pub(super) fn private_message_command(&mut self, command: Command) -> AppResult<()> {
        if !self.private_chat.requested || self.private_chat.vault.is_none() {
            return Err("請先開啟並解鎖私聊。".into());
        }
        match command {
            Command::Select { peer } => {
                if !self
                    .private_chat
                    .contacts
                    .friends
                    .iter()
                    .any(|friend| friend.friend_id == peer)
                {
                    return Err("找不到此私聊好友。".into());
                }
                let state = &mut self.private_chat.messaging;
                state.leave();
                state.peer = Some(peer);
                self.post_private_timeline()
            }
            Command::Page { older } => {
                let state = &mut self.private_chat.messaging;
                if older {
                    state.page = state.page.saturating_add(1);
                } else {
                    state.page = state.page.saturating_sub(1);
                }
                self.post_private_timeline()
            }
            Command::Space => self.post_private_timeline(),
            Command::SendText { peer, text } => {
                self.check_send_peer(&peer)?;
                let body = messages::text_payload(text)?;
                self.queue_private_message(peer, body)
            }
            Command::ChooseImage { peer } => {
                self.check_send_peer(&peer)?;
                let Some(path) = choose_image(self.window)? else {
                    return Ok(());
                };
                self.private_chat_start(move |_| {
                    let extension = path
                        .extension()
                        .and_then(|value| value.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if !matches!(extension.as_str(), "jpg" | "jpeg" | "png") {
                        return Err("請選擇 JPG 或 PNG 圖片。".to_string().into());
                    }
                    let picture = chat::convert_private_image(&path, &extension)?;
                    Ok(super::Reply::Message(Reply::ImageDraft(
                        peer,
                        Payload::Image(picture),
                    )))
                })
            }
            Command::PasteImage { peer } => {
                self.check_send_peer(&peer)?;
                self.private_chat_start(move |_| {
                    let bytes = crate::screenshot::private_clipboard_image()?;
                    Ok(super::Reply::Message(Reply::ImageDraft(
                        peer,
                        Payload::Image(bytes),
                    )))
                })
            }
            Command::DiscardImage => {
                self.private_chat.messaging.draft_image = None;
                Ok(())
            }
            Command::SendImage { peer } => {
                self.check_send_peer(&peer)?;
                let body = self
                    .private_chat
                    .messaging
                    .draft_image
                    .take()
                    .ok_or("請先選擇圖片。")?;
                self.queue_private_message(peer, body)
            }
            Command::Retry { id } => {
                let record = self.selected_private_record(&id)?;
                if !record.outgoing || !matches!(record.state, State::Pending | State::Rejected) {
                    return Err("這則訊息已送出。".into());
                }
                self.check_send_peer(&record.peer)?;
                let body = record.payload(self.private_chat.unlocked()?)?;
                self.send_private_message(record.peer.clone(), id, body)
            }
            Command::ShowImage { id } => {
                let record = self.selected_private_record(&id)?;
                let Payload::Image(bytes) = &record.payload(self.private_chat.unlocked()?)? else {
                    return Err("這則訊息沒有可顯示的圖片。".into());
                };
                let encoded = chat::encode_image(bytes)?;
                self.view
                    .post(&json!({"type":"private_chat_image","peer":record.peer,
                    "url":format!("data:image/jpeg;base64,{encoded}")}))
            }
            Command::Remove {
                id,
                confirmed: true,
            } => {
                self.selected_private_record(&id)?;
                let state = &mut self.private_chat;
                let vault = state.vault.as_ref().ok_or("私聊已上鎖。")?;
                state.messaging.history.remove(vault, &id)?;
                self.post_private_timeline()
            }
            Command::Cleanup {
                period,
                only_images,
                confirmed: true,
            } => {
                if matches!(period, Period::All) && !only_images {
                    return Err("清理所有期間只適用於圖片。".into());
                }
                let state = &mut self.private_chat;
                let vault = state.vault.as_ref().ok_or("私聊已上鎖。")?;
                let cutoff = period.cutoff(crate::unix_now())?;
                let removed = state
                    .messaging
                    .history
                    .cleanup(vault, cutoff, only_images)?;
                state.status = format!("已清理 {removed} 則本機訊息。");
                self.post_private_timeline()
            }
            Command::Remove { .. } | Command::Cleanup { .. } => {
                Err("請先確認清除本機訊息。".into())
            }
        }
    }

    fn check_send_peer(&self, peer: &str) -> AppResult<()> {
        let state = &self.private_chat;
        if state.busy || state.worker.strong_count() != 0 {
            return Err("私聊正在同步，請稍後送出。".into());
        }
        if state.messaging.peer.as_deref() != Some(peer)
            || !state.contacts.friends.iter().any(|friend| {
                friend.friend_id == peer
                    && friend.status == "active"
                    && !state.contacts.is_blocked(&friend.employee_no)
            })
        {
            return Err("目前無法傳送給此好友。".into());
        }
        Ok(())
    }

    fn selected_private_record(&self, id: &str) -> AppResult<&messages::Record> {
        let state = &self.private_chat.messaging;
        state
            .history
            .record(id)
            .filter(|record| Some(record.peer.as_str()) == state.peer.as_deref())
            .ok_or_else(|| "找不到目前私聊中的這則訊息。".into())
    }

    fn queue_private_message(&mut self, peer: String, body: Payload) -> AppResult<()> {
        let state = &mut self.private_chat;
        let vault = state.vault.as_ref().ok_or("私聊已上鎖。")?;
        let kind = body.kind();
        let id = match state.messaging.history.append_outgoing(vault, &peer, &body) {
            Ok(id) => id,
            Err(error) => {
                if kind == Kind::Image {
                    state.messaging.draft_image = Some(body);
                }
                return Err(error);
            }
        };
        let started = self.send_private_message(peer.clone(), id, body);
        let composer = self
            .view
            .post(&json!({"type":"private_chat_composer_sent","peer":peer,"kind":kind}));
        let timeline = self.post_private_timeline();
        started.and(composer).and(timeline)
    }

    fn send_private_message(&mut self, peer: String, id: String, body: Payload) -> AppResult<()> {
        self.private_chat_start(move |api| match api.send_message(&peer, &id, &body) {
            Err(error) if error.unavailable => Err(error),
            result => Ok(super::Reply::Message(Reply::Sent(id, result))),
        })
    }

    pub(super) fn post_private_timeline(&mut self) -> AppResult<()> {
        let state = &mut self.private_chat;
        if !state.requested {
            return Ok(());
        }
        let vault = state.vault.as_ref().ok_or("私聊已上鎖。")?;
        let timeline = &mut state.messaging;
        let mut rows = Vec::new();
        let mut older = false;
        if let Some(peer) = &timeline.peer {
            let entries = timeline.history.for_peer(peer);
            timeline.page = timeline
                .page
                .min(entries.len().saturating_sub(1) / PAGE_SIZE);
            let end = entries.len().saturating_sub(timeline.page * PAGE_SIZE);
            let start = end.saturating_sub(PAGE_SIZE);
            older = start > 0;
            for entry in &entries[start..end] {
                let content = if entry.kind == Kind::Text {
                    entry.payload(vault).ok()
                } else {
                    None
                };
                let text = match &content {
                    Some(Payload::Text(text)) => Some(text),
                    _ => None,
                };
                rows.push(
                    json!({"id":entry.id,"time":entry.time,"kind":entry.kind,"state":entry.state,
                    "reason":entry.reason,"outgoing":entry.outgoing,"text":text}),
                );
            }
        }
        self.view.post(
            &json!({"type":"private_chat_timeline","peer":timeline.peer,"messages":rows,
            "older":older,"newer":timeline.page > 0,"bytes":timeline.history.occupied(vault)}),
        )
    }
}

/// 路徑只由原生檔案選擇器回傳。WebView 無法指定任意本機檔案。
fn choose_image(owner: HWND) -> AppResult<Option<PathBuf>> {
    use windows::{
        core::{w, HRESULT},
        Win32::{
            Foundation::{ERROR_CANCELLED, HWND as NativeWindow},
            System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER},
            UI::Shell::{
                Common::COMDLG_FILTERSPEC, FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST,
                FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
            },
        },
    };
    let result = || -> windows::core::Result<Option<PathBuf>> {
        // SAFETY: 本函式在 UI 的 COM STA 執行，選擇後複製字串並以 CoTaskMemFree 釋放。
        unsafe {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetTitle(w!("選擇私聊圖片"))?;
            dialog.SetOptions(FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_FILEMUSTEXIST)?;
            dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: w!("JPEG 或 PNG"),
                pszSpec: w!("*.jpg;*.jpeg;*.png"),
            }])?;
            if let Err(error) = dialog.Show(Some(NativeWindow(owner))) {
                if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                    return Ok(None);
                }
                return Err(error);
            }
            let wide = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = wide.to_string();
            CoTaskMemFree(Some(wide.0.cast()));
            Ok(Some(PathBuf::from(path?)))
        }
    };
    result().map_err(|error| format!("無法選擇私聊圖片：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_scheduler_can_only_poll_status_even_with_pending_fetch() {
        let mut state = Runtime {
            need_fetch: true,
            next_contacts: 150,
            ..Runtime::default()
        };
        assert_eq!(state.due(100, 100, false), Some(Poll::Status));
        assert_eq!(state.due(101, 120, false), None);
        assert_eq!(state.due(101, 120, true), Some(Poll::Receive));
        assert_eq!(state.due(150, 160, true), Some(Poll::Contacts));
        state.lock();
        assert_eq!(state.due(101, 120, true), None);
    }
}
