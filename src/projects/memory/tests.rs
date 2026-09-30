use super::*;
fn fixture() -> Project {
    let root = std::env::current_dir()
        .unwrap()
        .join(".build")
        .join("memory-tests")
        .join(jobs::new_id().unwrap());
    std::fs::create_dir_all(&root).unwrap();
    Project {
        id: "test".into(),
        name: "test".into(),
        root,
        imports: BTreeMap::new(),
    }
}
#[test]
fn notes_are_persistent_versioned_scoped_and_restorable() {
    let project = fixture();
    let memory = Memory::open(project.clone(), "chat_a").unwrap();
    let created = memory
        .create_note("conversation", "條件", "保留數值 42")
        .unwrap();
    let id = created["id"].as_str().unwrap();
    let reopened = Memory::open(project.clone(), "chat_a").unwrap();
    assert_eq!(reopened.read_note(id).unwrap()["body"], "保留數值 42");
    assert!(reopened
        .change_note(id, "wrong", Some(("條件", "錯誤覆寫")), false)
        .is_err());
    assert_eq!(
        reopened
            .change_note(id, "1", Some(("條件", "保留數值 43")), false)
            .unwrap()["revision"],
        "2"
    );
    assert_eq!(
        reopened.change_note(id, "2", None, false).unwrap()["deleted"],
        true
    );
    assert_eq!(
        reopened.change_note(id, "3", None, true).unwrap()["deleted"],
        false
    );
    assert_eq!(reopened.read_note(id).unwrap()["body"], "保留數值 43");
    let other = Memory::open(project.clone(), "chat_b").unwrap();
    assert!(other.read_note(id).is_err());
    let shared = memory
        .create_note("project", "專案事實", "使用者確認版本 A")
        .unwrap();
    assert!(other.read_note(shared["id"].as_str().unwrap()).is_ok());
    let bytes = std::fs::read(project.root.join(".lmai/project/index.dpapi")).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("保留數值"));
    assert!(crate::storage::protect(&bytes, false).is_ok());
}
#[test]
fn document_sections_require_read_evidence_and_reject_stale_sources() {
    let project = fixture();
    let source = "α😀內容\n\n".repeat(2000);
    std::fs::write(project.root.join("source.txt"), &source).unwrap();
    let mut memory = Memory::open(project.clone(), "chat").unwrap();
    let stamp = memory.source_stamp("source.txt").unwrap();
    memory
        .register_document("source.txt", &source, &stamp)
        .unwrap();
    let revision = text::revision(&source);
    assert!(memory
        .update_document_note("source.txt", &revision, "1", None, "假裝完整摘要")
        .is_err());
    assert!(memory.document_info("source.txt", usize::MAX).is_err());
    let info = memory.document_info("source.txt", 0).unwrap();
    let id = info["sections"][0]["section_id"].as_str().unwrap();
    assert!(memory
        .update_document_note("source.txt", &revision, "1", Some(id), "未讀部分")
        .is_err());
    let read = memory.read_section("source.txt", &revision, id).unwrap();
    assert!(read["text"].as_str().unwrap().chars().count() <= 4000);
    let changed = memory
        .update_document_note(
            "source.txt",
            &revision,
            "1",
            Some(id),
            "本段包含測試文字與 emoji",
        )
        .unwrap();
    assert_eq!(changed["note_revision"], "2");
    assert!(memory
        .update_document_note("source.txt", &revision, "1", Some(id), "舊版本覆寫")
        .is_err());
    memory.record_read("source.txt", 0, source.chars().count());
    memory
        .update_document_note("source.txt", &revision, "2", None, "完整文件摘要")
        .unwrap();
    let reopened = Memory::open(project.clone(), "chat").unwrap();
    assert_eq!(
        reopened.document_info("source.txt", 0).unwrap()["summary"],
        "完整文件摘要"
    );
    std::fs::write(project.root.join("source.txt"), "新版本").unwrap();
    assert!(reopened.document_info("source.txt", 0).is_err());
    let context = reopened.context(&[Message::user("source.txt")]).unwrap();
    assert!(!context[0].content.contains("完整文件摘要"));
    assert!(memory
        .register_document("source.txt", "新版本", &stamp)
        .is_err());
}
#[test]
fn context_keeps_original_results_on_disk_and_only_sends_selected_memory() {
    let project = fixture();
    let memory = Memory::open(project.clone(), "chat").unwrap();
    let long = "UNIQUE_FULL_ANSWER_".repeat(2000);
    memory
        .save_run(
            "previous",
            "原始要求",
            &long,
            "completed",
            Some("上一輪已確認數值 42"),
            &["_AI_Output/report.txt".into()],
        )
        .unwrap();
    let mut user = Message::user("原始要求");
    user.request_id = Some("previous".into());
    let messages = vec![
        user,
        Message::assistant(long.clone()),
        Message::user("請修改第三點"),
    ];
    memory
        .save_run(
            "future",
            "後來要求",
            "後來答案",
            "failed",
            Some("不應附帶的後來結果"),
            &[],
        )
        .unwrap();
    let context = memory.context(&messages).unwrap();
    assert!(!context[0].content.contains("不應附帶的後來結果"));
    assert!(!memory.context(&[Message::user("原始要求")]).unwrap()[0]
        .content
        .contains("上一輪已確認"));
    assert!(!context
        .iter()
        .any(|m| m.content.contains("UNIQUE_FULL_ANSWER_")));
    assert!(context[0].content.contains("上一輪已確認數值 42"));
    assert!(context[0].content.contains("previous"));
    assert_eq!(context.last().unwrap().content, "請修改第三點");
    let old = memory.read_task_result("previous", "result", 6000).unwrap();
    assert_eq!(old["offset"], 6000);
    assert_eq!(old["text"].as_str().unwrap().chars().count(), 6000);
    let other = Memory::open(project, "other").unwrap();
    assert!(other.read_task_result("previous", "result", 0).is_err());
    for i in 0..12 {
        memory
            .create_note("conversation", &format!("note {i}"), &"甲".repeat(2000))
            .unwrap();
    }
    assert!(memory.context(&messages).unwrap()[0].content.len() < 42_000);
}
#[test]
fn reserved_folder_and_linked_private_files_are_rejected() {
    let project = fixture();
    let memory = Memory::open(project.clone(), "chat").unwrap();
    for path in [
        ".lmai",
        ".LMAI/a.txt",
        "sub/.lmai/source.txt",
        ".lmai/../a.txt",
    ] {
        assert!(files::relative(path).is_err());
    }
    assert!(files::validate_root(&project.root.join(".lmai")).is_err());
    std::fs::write(project.root.join("original.txt"), "original").unwrap();
    std::fs::create_dir(project.root.join(".lmai/project")).unwrap_or(());
    let target = project.root.join(".lmai/project/index.dpapi");
    std::fs::hard_link(project.root.join("original.txt"), target).unwrap();
    assert!(memory
        .create_note("project", "should fail", "do not touch original")
        .is_err());
    assert_eq!(
        std::fs::read_to_string(project.root.join("original.txt")).unwrap(),
        "original"
    );
}
#[test]
fn pdf_cache_survives_reopen_checks_hash_and_has_a_bounded_index() {
    let project = fixture();
    pdf_write(&project.root, "source", "profile", "# 原文").unwrap();
    assert_eq!(
        pdf_read(&project.root, "source", "profile")
            .unwrap()
            .as_deref(),
        Some("# 原文")
    );
    assert!(pdf_read(&project.root, "new-source", "profile")
        .unwrap()
        .is_none());
    assert!(pdf_read(&project.root, "source", "changed-profile")
        .unwrap()
        .is_none());
    let vault = Vault::new(&project.root).unwrap();
    vault
        .transaction()
        .unwrap()
        .write(
            "cache",
            &text::revision("source:profile"),
            &PdfCache {
                source_hash: "source".into(),
                profile: "profile".into(),
                markdown: "altered".into(),
                markdown_hash: "incorrect".into(),
            },
        )
        .unwrap();
    assert!(pdf_read(&project.root, "source", "profile")
        .unwrap()
        .is_none());
    for i in 0..25 {
        pdf_write(&project.root, &format!("s{i}"), "profile", "text").unwrap();
    }
    assert_eq!(
        std::fs::read_dir(project.root.join(".lmai/cache"))
            .unwrap()
            .count(),
        21
    );
}
