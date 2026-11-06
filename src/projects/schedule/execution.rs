//! 固定流程執行器：每一步先落盤 InFlight，成功快照落盤後才前進。
use super::{Grant, Journal, Phase};
use crate::{
    projects::{
        files::{self, Broker},
        sandbox::Worker,
        skills, Project, Tool,
    },
    AppResult,
};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

/// 只接受保存時核准的帳號／專案／根目錄與範本；每次啟動及續接重新核對。
pub fn check_target(project: &Project, grant: &Grant, principal: &str) -> AppResult<()> {
    grant.validate()?;
    if project.id != grant.target.project
        || principal != grant.target.principal
        || files::directory_identity(&project.root)? != grant.target.root_identity
    {
        return Err("排程帳號、專案或根目錄已改變，請重新預覽授權。".into());
    }
    let current =
        crate::projects::memory::Memory::open(project.clone(), &grant.target.conversation)?
            .load_workflow(&grant.target.template)?;
    if crate::projects::text::revision(&serde_json::to_string(&current).map_err(|e| e.to_string())?)
        != grant.target.revision
    {
        return Err("流程範本版本已改變，原排程已暫停。".into());
    }
    Ok(())
}

/// 固定的內建技能只啟用已核准步驟所需能力，不加入模型或新的工具操作。
fn step_skill(tool: &str) -> Option<&'static str> {
    match tool {
        "run_python" => Some("python-analysis"),
        "inspect_excel" | "read_excel_range" => Some("excel-read"),
        "inspect_dataset" | "chart_dataset" => Some("dataset-charts"),
        "create_working_copy"
        | "office_action"
        | "office_batch"
        | "edit_office"
        | "save_copy"
        | "register_report_values"
        | "apply_report_values"
        | "export_report_values" => Some("office-edit"),
        "create_mermaid" | "export_mermaid" => Some("mermaid"),
        _ => None,
    }
}

pub fn run(
    root: &Path,
    exe: &Path,
    project: Project,
    grant: &Grant,
    execution: &str,
    cancel: &AtomicBool,
    resume: bool,
) -> AppResult<()> {
    run_observed(
        root,
        exe,
        project,
        grant,
        execution,
        cancel,
        resume,
        |_, _| Ok(()),
    )
}

/// 持久邊界觀察點供原生故障注入驗證；正式入口使用無副作用的觀察器。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    Ready,
    BeforeStep,
    AfterStep,
}
#[allow(clippy::too_many_arguments)]
pub fn run_observed(
    root: &Path,
    exe: &Path,
    project: Project,
    grant: &Grant,
    execution: &str,
    cancel: &AtomicBool,
    resume: bool,
    mut observe: impl FnMut(Boundary, usize) -> AppResult<()>,
) -> AppResult<()> {
    let mut journal = if resume {
        Journal::load(root, execution)?
    } else {
        Journal::new(execution.into(), grant)?
    };
    if resume {
        journal.can_resume(grant)?;
    } else {
        journal.create(root)?;
    }
    check_target(&project, grant, &grant.target.principal)?;
    // D03 執行已核准的本機步驟，沒有模型推論可重試。防睡眠沿用同一個守衛。
    let _awake = grant
        .continuous_retry
        .then(|| crate::keep_awake::KeepAwake::watch(|| true))
        .transpose()?;
    if !resume && needs_office(grant) {
        if let Some(reason) = crate::projects::office::busy_before_dispatch()? {
            journal.phase = Phase::Skipped;
            journal.detail = reason;
            journal.save(root)?;
            return Ok(());
        }
    }
    let mut broker = Broker::new(project.clone(), execution.into())?;
    broker.enable_memory(&grant.target.conversation)?;
    if resume {
        broker.restore(journal.broker.take().ok_or("缺少續接快照。")?, cancel)?;
    } else {
        broker.prepare_workflow(
            &grant.target.template,
            &serde_json::to_string(&grant.parameters).map_err(|e| e.to_string())?,
        )?;
    }
    let mut worker = Worker::start(exe, cancel)?;
    loop {
        let active = broker.workflow.as_ref().ok_or("流程狀態不存在。")?;
        if active.template.id != grant.target.template || active.parameters != grant.parameters {
            return Err("執行中的流程與核准範本不同。".into());
        }
        journal.next = active.next;
        journal.outputs = broker.published().to_vec();
        journal.broker = Some(broker.saved()?);
        journal.phase = if active.next == active.template.steps.len() {
            Phase::Completed
        } else {
            Phase::Ready
        };
        journal.detail = if journal.phase == Phase::Completed {
            "所有核准步驟已完成，成果仍需使用者驗收。".into()
        } else {
            "下一步尚未執行。".into()
        };
        journal.save(root)?;
        observe(Boundary::Ready, journal.next)?;
        if journal.phase == Phase::Completed || cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        check_target(&project, grant, &grant.target.principal)?;
        let request = active.request()?;
        let value = serde_json::to_value(&request).map_err(|e| e.to_string())?;
        let tool = value["tool"].as_str().ok_or("工具名稱缺漏。")?;
        if let Some(skill) = step_skill(tool) {
            // 啟用既有技能本身不寫來源；保存於同一 broker，續接不擴大操作清單。
            let mut ids = vec![];
            skills::activate(&mut ids, skill)?;
            if !skills::enabled(tool, &ids) {
                return Err("排程工具缺少相應內建技能。".into());
            }
            let outcome = broker.execute(
                &format!("schedule_skill_{}", journal.next),
                &Tool::LoadSkill { id: skill.into() },
                &mut worker,
                cancel,
            )?;
            if outcome["ok"] != true {
                return Err("無法啟用排程步驟所需技能。".into());
            }
        }
        let flow_id = broker
            .workflow
            .as_ref()
            .ok_or("流程狀態遺失。")?
            .execution_id
            .clone();
        journal.phase = Phase::InFlight;
        journal.detail = format!(
            "步驟 {} 已準備提交；若此處中斷，結果視為未知。",
            journal.next + 1
        );
        journal.save(root)?;
        observe(Boundary::BeforeStep, journal.next)?;
        let result = broker.execute(
            &format!("schedule_step_{}", journal.next),
            &Tool::ExecuteWorkflowStep {
                execution_id: flow_id,
                step_index: journal.next,
            },
            &mut worker,
            cancel,
        );
        observe(Boundary::AfterStep, journal.next)?;
        match result {
            Ok(result)
                if result["ok"] == true
                    && result["result"]["outcome"]["ok"] == true
                    && result["result"]["outcome"]["result"]["status"] != "FAILED"
                    && result["result"]["outcome"]["result"]["executed"] != false => {}
            result => {
                journal.phase = Phase::Blocked;
                journal.outputs = broker.published().to_vec();
                journal.detail = match result {
                    Ok(value) => value.to_string(),
                    Err(error) => error,
                };
                journal.detail = journal.detail.chars().take(4000).collect();
                journal.broker = Some(broker.saved()?);
                journal.save(root)?;
                return Err("排程步驟未確認成功，已暫停；不自動重播。請查看審計與成果。".into());
            }
        }
        // 成功後進入下一輪先持久保存新版本；保存失敗保留上次 InFlight，禁止重播。
    }
}

/// 檢查固定範本及核准參數中明示的Office路徑／操作；不執行或猜測模型程式。
fn needs_office(grant: &Grant) -> bool {
    fn office_path(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::String(s) => crate::projects::office::supported(Path::new(s)),
            serde_json::Value::Array(values) => values.iter().any(office_path),
            serde_json::Value::Object(values) => values.values().any(office_path),
            _ => false,
        }
    }
    grant.template.steps.iter().any(|step| {
        let tool = step.request["tool"].as_str().unwrap_or("");
        tool.contains("office") || tool.contains("excel") || office_path(&step.request)
    }) || grant.parameters.values().any(office_path)
}
