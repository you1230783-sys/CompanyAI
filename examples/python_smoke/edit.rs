//! 實跑PY讀取、修正、語法檢查與發布；不能以AST成功冒充compile或功能測試成功。
use super::*;

pub(super) fn run(
    broker: &mut Broker,
    worker: &mut Worker,
    project: &Project,
) -> AppResult<String> {
    let original="def greeting(name):\n    return f'你好 {name}'\n\nraise RuntimeError('syntax checker must never execute this')\n";
    std::fs::write(project.root.join("source.py"), original).map_err(|e| e.to_string())?;
    call(
        broker,
        worker,
        "py-edit-skill",
        json!({"tool":"load_skill","id":"python-edit"}),
    )?;
    let read = call(
        broker,
        worker,
        "py-read",
        json!({"tool":"read_file","path":"source.py","offset":0}),
    )?;
    assert_eq!(read["text"], original);
    let copy = call(
        broker,
        worker,
        "py-copy",
        json!({"tool":"create_working_copy","source":"source.py","name":"modified.py"}),
    )?;
    let id = copy["copy_id"].as_str().ok_or("缺少副本ID")?;
    // 未檢查的版本不可被發布為已完成；失敗不建立輸出檔。
    let save: Tool = serde_json::from_value(
        json!({"tool":"save_copy","copy_id":id,"revision":copy["revision"]}),
    )
    .map_err(|e| e.to_string())?;
    assert_eq!(
        broker.execute("py-unchecked", &save, worker, &AtomicBool::new(false))?["ok"],
        false
    );
    let checked = call(
        broker,
        worker,
        "py-check-original",
        json!({"tool":"check_python","path":id,"revision":copy["revision"]}),
    )?;
    assert_eq!(checked["syntax_valid"], true);
    assert_eq!(checked["source_executed"], false);
    assert_eq!(checked["functional_tests_run"], false);
    assert_eq!(checked["outline"][0]["name"], "greeting");
    let invalid = "return 42\n";
    let changed = call(
        broker,
        worker,
        "py-break",
        json!({"tool":"edit_text","copy_id":id,"revision":copy["revision"],"start":0,"expected":original,"replacement":invalid}),
    )?;
    let draft = changed["draft_path"]
        .as_str()
        .ok_or("修改後未保存草稿")?
        .to_owned();
    assert_eq!(
        std::fs::read_to_string(project.root.join(&draft)).map_err(|e| e.to_string())?,
        invalid
    );
    let check = call(
        broker,
        worker,
        "py-check-invalid",
        json!({"tool":"check_python","path":id,"revision":changed["revision"]}),
    )?;
    assert_eq!(check["syntax_valid"], false);
    assert_eq!(check["errors"][0]["line"], 1);
    let invalid_save: Tool = serde_json::from_value(
        json!({"tool":"save_copy","copy_id":id,"revision":changed["revision"]}),
    )
    .map_err(|e| e.to_string())?;
    assert_eq!(
        broker.execute(
            "py-invalid-save",
            &invalid_save,
            worker,
            &AtomicBool::new(false)
        )?["ok"],
        false
    );
    let fixed="def greeting(name):\n    return f'歡迎 {name}'\n\nimport module_that_is_not_installed\nraise RuntimeError('not executed')\n";
    let section = call(
        broker,
        worker,
        "py-section",
        json!({"tool":"read_code_section","path":id,"first_line":1,"last_line":1}),
    )?;
    let changed = call(
        broker,
        worker,
        "py-fix",
        json!({"tool":"edit_code_section","copy_id":id,"revision":section["revision"],"first_line":1,"last_line":1,"section_hash":section["section_hash"],"replacement":fixed}),
    )?;
    assert_eq!(changed["draft_path"], draft);
    assert_eq!(
        std::fs::read_to_string(project.root.join(&draft)).map_err(|e| e.to_string())?,
        fixed
    );
    assert_eq!(
        call(
            broker,
            worker,
            "py-check-fixed",
            json!({"tool":"check_python","path":id,"revision":changed["revision"]})
        )?["syntax_valid"],
        true
    );
    let saved = call(
        broker,
        worker,
        "py-save",
        json!({"tool":"save_copy","copy_id":id,"revision":changed["revision"]}),
    )?;
    assert_eq!(saved["path"], draft);
    assert_eq!(
        call(
            broker,
            worker,
            "py-readback",
            json!({"tool":"read_file","path":saved["path"]})
        )?["text"],
        fixed
    );
    assert_eq!(
        std::fs::read_to_string(project.root.join("source.py")).map_err(|e| e.to_string())?,
        original
    );
    // 編碼宣告必須與實際bytes相符；Big5與UTF-8 BOM各自按Python規則驗證。
    for (name, source, encoding, valid) in [
        (
            "big5.py",
            "# coding: big5\n文字 = '測試'\n",
            company_ai::projects::text::Encoding::CodePage(950),
            true,
        ),
        (
            "bom.py",
            "文字 = '測試'\n",
            company_ai::projects::text::Encoding::Utf8(true),
            true,
        ),
        (
            "cookie.py",
            "# coding: latin-1\n文字 = '測試'\n",
            company_ai::projects::text::Encoding::Utf8(false),
            false,
        ),
        (
            "indent.py",
            "def f():\nreturn 1\n",
            company_ai::projects::text::Encoding::Utf8(false),
            false,
        ),
    ] {
        std::fs::write(
            project.root.join(name),
            company_ai::projects::text::encode(source, encoding)?,
        )
        .map_err(|e| e.to_string())?;
        let revision = company_ai::projects::text::revision(source);
        let check = call(
            broker,
            worker,
            &format!("check-{}", name.replace('.', "-")),
            json!({"tool":"check_python","path":name,"revision":revision}),
        )?;
        assert_eq!(check["syntax_valid"], valid, "{name}");
    }
    println!("PASS Python editing: original preserved, versioned checks, AST plus compile, source never executed, UTF-8/BOM/Big5, coding mismatch and readback.");
    // 回傳已發布的副本ID，讓總驗收一併核對交付清單，不遺漏新增的成果。
    Ok(id.to_owned())
}
