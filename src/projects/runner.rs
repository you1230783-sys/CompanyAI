//! 專案代理往返。每輪模型請求有獨立 ID；工具只在本次活躍任務內執行。
//! 中斷紀錄保留供檢查，不在重新登入或重啟後自動重播寫入。

mod execution;
use execution::*;
mod recovery;
mod task_plan;
use recovery::*;
pub use recovery::{
    recover, recover_activity, recover_analysis, recover_charts, recover_diagrams,
    recover_local_time, recover_overview, recover_review,
};
pub use task_plan::{pending_plan, review_plan};
#[cfg(test)]
mod pause_tests;
use super::{files::Broker, sandbox::Worker, Decision, Project};
use crate::calendar::Activity;
use crate::{
    config::Config,
    jobs::{self, Task},
    protocol::Message,
    storage::{self, Session},
    AppResult,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub struct Run {
    pub plan_mode: bool,
    pub id: String,
    pub resume: bool,
    pub project: Project,
    pub conversation: String,
    pub messages: Vec<Message>,
    pub config: Config,
    pub session: Session,
    pub root: PathBuf,
    pub cancel: Arc<AtomicBool>,
    pub instructions: Option<super::steering::Inbox>,
    pub outlook_consent: Option<super::mail::Consent>,
    pub cad_waiter: Option<crate::cad::Waiter>,
    pub cad_progress: Option<crate::cad::Progress>,
    pub file_waiter: Option<super::interaction::FileWaiter>,
}

impl Run {
    pub(super) fn retry_control(&self) -> AppResult<super::steering::Inbox> {
        self.instructions.as_ref().map_or_else(
            || super::steering::Inbox::open(&self.root, &self.id),
            |inbox| Ok(inbox.clone()),
        )
    }
}

const MAX_TOOLS: usize = 60;
const MAX_REPLIES: usize = 80;
fn quota_reached(replies: usize, tools: usize) -> bool {
    replies >= MAX_REPLIES || tools >= MAX_TOOLS
}

/// 尚未產生DWG時，已保存的方案與建圖前錯誤仍是下一輪必需的資料。
/// 不只檢查成果清單，否則筆記被新進度取代後，模型看不到本機方案。
fn has_cad_context(state: &Value) -> bool {
    state["artifacts"]
        .as_array()
        .is_some_and(|items| !items.is_empty())
        || state["blocked_after_error"] == true
        || !state["last_execution"].is_null()
        || !state["design_planning"].is_null()
}

const SEGMENT_BUDGET: Duration = Duration::from_secs(24 * 60 * 60);
pub(super) const TIME_LIMIT_REASON: &str = "本段專案執行已達 24 小時";
#[derive(serde::Serialize, serde::Deserialize)]
struct PausedRun {
    version: u32,
    available: bool,
    project: String,
    project_root: PathBuf,
    conversation: String,
    principal: String,
    request_text: String,
    broker: super::files::SavedBroker,
    progress: super::progress::Progress,
    /// 0.8.26 暫停檔沒有此欄位；舊版只會在工具全部完成後暫停。
    #[serde(default)]
    pending_model: Option<Task>,
}
fn pause_path(root: &Path, id: &str) -> AppResult<PathBuf> {
    jobs::validate_id(id)?;
    Ok(root.join("project-runs").join(format!("{id}.resume.dpapi")))
}
fn load_pause(root: &Path, id: &str) -> AppResult<PausedRun> {
    let path = pause_path(root, id)?;
    if std::fs::metadata(&path)
        .map_err(|_| "找不到暫停紀錄。")?
        .len()
        > 64_000_000
    {
        return Err("暫停紀錄過大。".into());
    }
    let bytes = storage::protect(&std::fs::read(path).map_err(|e| e.to_string())?, false)?;
    serde_json::from_slice(&bytes).map_err(|_| "暫停紀錄無法解析。".into())
}
/// 按鈕依程式建立的暫存紀錄顯示，不相信模型自行聲稱已暫停。
pub fn paused_available(root: &Path, id: &str) -> bool {
    load_pause(root, id).is_ok_and(|s| s.version == 1 && s.available)
}

/// 只有原帳號與對話能更改暫停任務的重試偏好。
pub fn paused_for(root: &Path, id: &str, conversation: &str, principal: &str) -> bool {
    load_pause(root, id).is_ok_and(|s| {
        s.version == 1 && s.available && s.conversation == conversation && s.principal == principal
    })
}
fn save_pause(
    run: &Run,
    principal: &str,
    request: &str,
    broker: &Broker,
    progress: &super::progress::Progress,
    pending_model: Option<&Task>,
    available: bool,
) -> AppResult<()> {
    let state = json!({"version":1,"available":available,"project":run.project.id,"project_root":run.project.root,
        "conversation":run.conversation,"principal":principal,"request_text":request,
        "broker":broker.saved()?,"progress":progress,"pending_model":pending_model});
    checkpoint(&pause_path(&run.root, &run.id)?, &state)
}

fn checkpoint(path: &Path, value: &Value) -> AppResult<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > 60_000_000 {
        return Err("專案暫存紀錄超過 60 MB，未宣稱已暫存。".into());
    }
    storage::atomic_write(path, &storage::protect(&bytes, true)?)
}

pub(super) fn diagnostic_excerpt(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let mut result: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        result.push_str("\n[內容過長，已截斷顯示]");
    }
    result
}

pub fn run(run: Run, progress: impl FnMut(Activity)) -> AppResult<String> {
    run_for(
        run,
        progress,
        SEGMENT_BUDGET,
        (|_| {}, None, None),
        false,
        None,
        None,
    )
}

/// 本機整合測試專用：使用同一恢復流程但縮短等待，release 不提供此入口。
#[cfg(debug_assertions)]
pub fn with_retry_test_clock<T>(work: impl FnOnce() -> T) -> T {
    super::model::retry::with_fast_clock(work)
}

/// 圖表為結構化 UI 事件，不混入模型文字或一般進度字串。
pub fn run_with_charts(
    run: Run,
    progress: impl FnMut(Activity),
    charts: impl FnMut(Vec<super::charts::Chart>),
) -> AppResult<String> {
    run_for(
        run,
        progress,
        SEGMENT_BUDGET,
        (charts, None, None),
        false,
        None,
        None,
    )
}

/// 正式桌面提供固定 PNG 繪製服務；非 UI 測試入口不虛構成功匯出。
pub fn run_with_chart_export(
    run: Run,
    progress: impl FnMut(Activity),
    charts: impl FnMut(Vec<super::charts::Chart>),
    analysis: impl FnMut(super::analysis::State) + 'static,
    renderer: super::charts::png::Renderer,
    chooser: super::charts::quality::Chooser,
) -> AppResult<String> {
    run_for(
        run,
        progress,
        SEGMENT_BUDGET,
        (charts, Some(Box::new(analysis)), None),
        false,
        Some(renderer),
        Some(chooser),
    )
}

/// 僅供 debug 整合測試推進期限，不改正式 EXE 的 24 小時政策。
#[cfg(debug_assertions)]
pub fn run_with_test_budget(
    run: Run,
    progress: impl FnMut(Activity),
    budget: Duration,
) -> AppResult<String> {
    run_for(
        run,
        progress,
        budget,
        (|_| {}, None, None),
        false,
        None,
        None,
    )
}

/// 舊文字協定的回歸測試入口；正式 EXE 不編入，不能用它降級新任務。
#[cfg(debug_assertions)]
pub fn run_legacy_test(run: Run, progress: impl FnMut(Activity)) -> AppResult<String> {
    run_for(
        run,
        progress,
        SEGMENT_BUDGET,
        (|_| {}, None, None),
        true,
        None,
        None,
    )
}
#[cfg(debug_assertions)]
pub fn run_legacy_with_test_budget(
    run: Run,
    progress: impl FnMut(Activity),
    budget: Duration,
) -> AppResult<String> {
    run_for(
        run,
        progress,
        budget,
        (|_| {}, None, None),
        true,
        None,
        None,
    )
}
#[cfg(debug_assertions)]
pub fn run_legacy_with_charts(
    run: Run,
    progress: impl FnMut(Activity),
    charts: impl FnMut(Vec<super::charts::Chart>),
) -> AppResult<String> {
    run_for(
        run,
        progress,
        SEGMENT_BUDGET,
        (charts, None, None),
        true,
        None,
        None,
    )
}

/// 桌面專案的圖表與 Mermaid 共用固定呈現服務，模型不能提供程式。
pub fn run_with_project_renderers(
    run: Run,
    progress: impl FnMut(Activity),
    charts: impl FnMut(Vec<super::charts::Chart>),
    analysis: impl FnMut(super::analysis::State) + 'static,
    renderers: (super::charts::png::Renderer, super::diagrams::Renderer),
    chooser: super::charts::quality::Chooser,
) -> AppResult<String> {
    run_for(
        run,
        progress,
        SEGMENT_BUDGET,
        (charts, Some(Box::new(analysis)), Some(renderers.1)),
        false,
        Some(renderers.0),
        Some(chooser),
    )
}

type AnalysisCallback = Box<dyn FnMut(super::analysis::State)>;

pub(super) fn check_cancel(cancel: &AtomicBool) -> AppResult<()> {
    if crate::reliability::faulted() {
        return Err(
            "Error Code: 13\n程式已進入異常保護狀態；核對既有成果後重新啟動，不重播寫入。".into(),
        );
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("專案任務已停止，不會繼續執行本機工具。".into());
    }
    Ok(())
}
