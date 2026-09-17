//! 全站鈴鐺同步控制器。網站與 AI 保有獨立游標、錯誤與操作，只在 UI 合併。
use super::*;
use crate::site_notifications::{self as site, Action, Cache, Failure};
pub(super) enum SiteEvent {
    Synced(u64, Result<Cache, Failure>),
    Acted(Action, Result<(), Failure>),
}
pub(super) struct SiteRuntime {
    pub cache: Cache,
    pub status: String,
    pub loading: bool,
    pub mutating: bool,
    pub blocked: bool,
    pub revision: u64,
    pub last: Instant,
    pub retry_seconds: u64,
    pub pending: bool,
    pub not_before: Option<Instant>,
    pub foreground: bool,
    pub read_all_pending: bool,
}
impl Default for SiteRuntime {
    fn default() -> Self {
        Self {
            cache: Cache::default(),
            status: "登入後同步全站鈴鐺".into(),
            loading: false,
            mutating: false,
            blocked: false,
            revision: 0,
            last: Instant::now(),
            retry_seconds: 60,
            pending: false,
            not_before: None,
            foreground: false,
            read_all_pending: false,
        }
    }
}
impl App {
    pub(super) fn start_site(&mut self) {
        self.site = SiteRuntime::default();
        if let Some(session) = &self.session {
            match site::load(&self.root, &self.config, session) {
                Ok(cache) => self.site.cache = cache,
                Err(e) => {
                    self.site.status = e;
                    self.site.cache =
                        site::Cache::for_session(&self.config, session).unwrap_or_default();
                }
            }
            self.fetch_site(true);
        }
    }
    pub(super) fn fetch_site(&mut self, force: bool) {
        if self.smoke
            || !self.logged_in()
            || self.site.blocked
            || self
                .site
                .not_before
                .is_some_and(|time| Instant::now() < time)
        {
            return;
        }
        if self.site.loading || self.site.mutating {
            if force {
                self.site.pending = true;
            }
            return;
        }
        if !force && self.site.last.elapsed() < Duration::from_secs(self.site.retry_seconds) {
            return;
        }
        self.site.loading = true;
        self.site.last = Instant::now();
        let (config, session, cache, tx, generation, revision) = (
            self.config.clone(),
            self.session.clone(),
            self.site.cache.clone(),
            self.tx.clone(),
            self.generation,
            self.site.revision,
        );
        if let Some(session) = session {
            thread::spawn(move || {
                let result = site::sync(&config, &session, &cache);
                let _ = tx.send(Event::Site(generation, SiteEvent::Synced(revision, result)));
            });
        }
    }
    fn site_failure(&mut self, error: Failure) {
        self.site.status = error.message;
        self.site.blocked = matches!(error.status, 401 | 403);
        self.site.retry_seconds = if error.status == 429 {
            120
        } else {
            (self.site.retry_seconds * 2).clamp(60, 600)
        };
        self.site.last = Instant::now();
        self.site.not_before = Some(Instant::now() + Duration::from_secs(self.site.retry_seconds));
    }
    pub(super) fn site_action(&mut self, action: Action) -> AppResult<()> {
        if self.site.mutating || self.site.blocked {
            return Err("全站通知操作不可用；請確認登入／權限或等候目前操作。".into());
        }
        if let Action::Read { id, .. } = &action {
            if !self.site.cache.items.iter().any(|n| &n.id == id) {
                return Err("找不到這則全站通知，請重新整理。".into());
            }
        }
        let session = self
            .session
            .clone()
            .filter(|s| s.valid_for(&self.config))
            .ok_or("請先登入。")?;
        self.site.mutating = true;
        self.site.revision += 1; // 操作前發出的 REST 副本不再允許覆蓋新狀態。
        let (config, tx, generation) = (self.config.clone(), self.tx.clone(), self.generation);
        thread::spawn(move || {
            let result = site::action(&config, &session, &action);
            let _ = tx.send(Event::Site(generation, SiteEvent::Acted(action, result)));
        });
        Ok(())
    }
    pub(super) fn site_event(&mut self, event: SiteEvent) -> AppResult<()> {
        match event {
            SiteEvent::Synced(revision, result) => {
                self.site.loading = false;
                if revision != self.site.revision {
                    self.fetch_site(true);
                    return Ok(());
                }
                match result {
                    Ok(cache) => {
                        let added = cache.new_unread_count(&self.site.cache);
                        site::save(&self.root, &cache)?;
                        self.site.cache = cache;
                        self.site.retry_seconds = 60;
                        self.site.not_before = None;
                        self.site.status = "全站鈴鐺已同步，已讀與刪除以網站為準".into();
                        if added > 0 && self.config.notification_popups && !self.is_foreground() {
                            self.work.task_balloon = false;
                            tray(
                                self.window,
                                NIM_MODIFY,
                                Some(&format!("收到 {added} 則全站新通知，點一下查看。")),
                            );
                        }
                    }
                    Err(e) => self.site_failure(e),
                }
                if self.site.pending {
                    self.site.pending = false;
                    self.fetch_site(true);
                }
            }
            SiteEvent::Acted(action, result) => {
                self.site.mutating = false;
                match result {
                    Ok(()) => {
                        let mut cache = self.site.cache.clone();
                        let mut open = None;
                        match action {
                            Action::Read {
                                id,
                                open: should_open,
                            } => {
                                if let Some(notice) = cache.items.iter_mut().find(|n| n.id == id) {
                                    if !notice.is_read {
                                        cache.unread_count = cache.unread_count.saturating_sub(1);
                                    }
                                    notice.is_read = true;
                                    notice.read_at = Some(notifications::now_text());
                                    if should_open {
                                        open = notice.url.clone();
                                    }
                                }
                            }
                            Action::ReadAll => {
                                for notice in &mut cache.items {
                                    notice.is_read = true;
                                    notice.read_at = Some(notifications::now_text());
                                }
                                cache.unread_count = 0;
                            }
                            Action::DeleteAll => {
                                cache.items.clear();
                                cache.unread_count = 0;
                            }
                        }
                        site::save(&self.root, &cache)?;
                        self.site.cache = cache;
                        self.site.status = "網站操作已成功，正在同步最新狀態".into();
                        self.fetch_site(true);
                        if let Some(url) = open {
                            open_browser(site::open_url(&self.config, &url)?.as_str())?;
                        }
                    }
                    Err(e) => {
                        self.site_failure(e);
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn site_tick(&mut self) -> bool {
        let foreground = self.is_foreground();
        let resumed = foreground && !self.site.foreground;
        self.site.foreground = foreground;
        if self.site.read_all_pending
            && self.logged_in()
            && !self.site.loading
            && !self.site.mutating
            && !self.site.blocked
            && self
                .site
                .not_before
                .is_none_or(|time| Instant::now() >= time)
        {
            self.site.read_all_pending = false;
            if let Err(error) = self.site_action(Action::ReadAll) {
                self.site.status = error;
            }
            return true;
        }
        let before = self.site.loading;
        self.fetch_site(resumed);
        before != self.site.loading
    }
}
