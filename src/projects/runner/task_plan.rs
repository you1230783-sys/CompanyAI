//! 使用者的核准只更新已安全暫停的任務；不消耗 checkpoint，也不重播工具。
use super::*;

/// 取得可恢復任務的計畫狀態；不消耗暫停點，也不執行任何模型或工具。
pub fn pending_plan(root: &Path, id: &str) -> Option<crate::projects::task_plan::State> {
    let saved = load_pause(root, id).ok()?;
    (saved.available && saved.broker.task_plan.enabled).then_some(saved.broker.task_plan)
}

#[allow(clippy::too_many_arguments)]
/// 核對原生身分與版本後原子保存使用者決策，續跑仍由既有任務入口處理。
pub fn review_plan(
    root: &Path,
    id: &str,
    project: &Project,
    conversation: &str,
    principal: &str,
    revision: u32,
    edited: Option<crate::projects::task_plan::Plan>,
    feedback: &str,
) -> AppResult<crate::projects::task_plan::State> {
    let mut saved = load_pause(root, id)?;
    if saved.version != 1
        || !saved.available
        || saved.pending_model.is_some()
        || saved.conversation != conversation
        || saved.project != project.id
        || saved.project_root != project.root
        || saved.principal != principal
    {
        return Err("計畫不屬於此帳號／專案，或任務尚未安全暫停，未核准。".into());
    }
    let state = saved.broker.review_task_plan(revision, edited, feedback)?;
    checkpoint(
        &pause_path(root, id)?,
        &serde_json::to_value(saved).map_err(|e| e.to_string())?,
    )?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_pending_review_preserves_identity_and_exact_delivery() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".build")
            .join(format!("plan-test-{}", jobs::new_id().unwrap()));
        std::fs::create_dir_all(&root).unwrap();
        let project = Project {
            id: "plan_project".into(),
            name: "計畫測試".into(),
            root: root.clone(),
            imports: Default::default(),
        };
        let mut broker = Broker::new(project.clone(), "plan_run".into()).unwrap();
        broker.task_plan = crate::projects::task_plan::State::new(true);
        let mut proposed = crate::projects::task_plan::tests::plan();
        broker.task_plan.submit(&proposed, false, "").unwrap();
        let mut saved = broker.saved().unwrap();
        // 舊格式缺欄位仍可讀，維持關閉計畫模式。
        let mut legacy = serde_json::to_value(&saved).unwrap();
        legacy.as_object_mut().unwrap().remove("task_plan");
        let old: crate::projects::files::SavedBroker = serde_json::from_value(legacy).unwrap();
        assert!(!old.task_plan.enabled);
        let pending = PausedRun {
            version: 1,
            available: true,
            project: project.id.clone(),
            project_root: root.clone(),
            conversation: "chat".into(),
            principal: "owner".into(),
            request_text: "request".into(),
            broker: saved,
            progress: crate::projects::progress::Progress::new(vec![]),
            pending_model: None,
        };
        let path = pause_path(&root, "plan_run").unwrap();
        checkpoint(&path, &serde_json::to_value(&pending).unwrap()).unwrap();
        assert!(
            !String::from_utf8_lossy(&std::fs::read(&path).unwrap()).contains("分析來源並交付報告")
        );
        assert!(pending_plan(&root, "plan_run").unwrap().pending.is_some());
        assert!(review_plan(
            &root,
            "plan_run",
            &project,
            "other",
            "owner",
            1,
            Some(proposed.clone()),
            ""
        )
        .is_err());
        assert!(review_plan(
            &root,
            "plan_run",
            &project,
            "chat",
            "other",
            1,
            Some(proposed.clone()),
            ""
        )
        .is_err());
        proposed.goal = "使用者核准後的新文字".into();
        review_plan(
            &root,
            "plan_run",
            &project,
            "chat",
            "owner",
            1,
            Some(proposed.clone()),
            "",
        )
        .unwrap();
        let restored = load_pause(&root, "plan_run").unwrap();
        assert!(restored.available);
        assert!(!restored.broker.task_plan.read_only);
        assert_eq!(restored.broker.task_plan.approved, Some(proposed));
        let data = serde_json::to_value(&restored.broker).unwrap();
        assert_eq!(data["delivery_plan"]["items"][0]["id"], "report");
        assert_eq!(data["delivery_plan"]["items"][0]["description"], "分析報告");
        assert!(data["delivery_plan"]["items"][0]["target"].is_null());
        assert!(
            review_plan(&root, "plan_run", &project, "chat", "owner", 1, None, "重送").is_err()
        );
        saved = restored.broker;
        // 一次核准不產生工具操作；已存在的成功操作仍由原索引去重。
        assert_eq!(serde_json::to_value(&saved).unwrap()["results"], json!({}));
    }
}
