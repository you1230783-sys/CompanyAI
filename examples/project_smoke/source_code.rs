//! 基礎驗證入口：真實唯讀檔案與原生broker，無模型服務或使用者專案。
use super::*;
use serde_json::{json, Value};

pub fn verify(exe: &std::path::Path, root: &std::path::Path) -> AppResult<()> {
    let cancel = AtomicBool::new(false);
    let mut worker = Worker::start(exe, &cancel)?;
    let project = Project {
        id: "code-read".into(),
        name: "程式唯讀".into(),
        root: root.into(),
        imports: Default::default(),
    };
    let mut broker = Broker::new(project, "code-read".into())?;
    // 正式App會啟用原生對話記憶，批次成果的序列提交也需要此工作日誌。
    broker.enable_memory("code-read-conversation")?;
    for name in [
        "main.cpp",
        "api.h",
        "index.html",
        "app.js",
        "style.css",
        "Cargo.toml",
        "Makefile",
        "CMakeLists.txt",
        "config.ini",
        "script.cmd",
        "script.ps1",
        "unknown.custom_text",
    ] {
        std::fs::write(root.join(name), "// 中文\r\nconst value = 42;\r\n")
            .map_err(|e| e.to_string())?;
    }
    let large = "// 中文大檔\n".repeat(30_000);
    std::fs::write(root.join("large.cpp"), &large).map_err(|e| e.to_string())?;
    std::fs::write(root.join("bad.cpp"), b"bad\0binary").map_err(|e| e.to_string())?;
    std::fs::write(root.join(".env.json"), "secret").map_err(|e| e.to_string())?;
    for folder in ["folder_a", "folder_b"] {
        std::fs::create_dir(root.join(folder)).map_err(|e| e.to_string())?;
        std::fs::write(
            root.join(folder).join("settings.ini"),
            "[general]\nenabled=true\n",
        )
        .map_err(|e| e.to_string())?;
    }
    // 未知附檔名須可被發現；超過單頁後仍能依offset取得剩餘項目。
    std::fs::create_dir(root.join("paging")).map_err(|e| e.to_string())?;
    for number in 0..205 {
        std::fs::write(
            root.join("paging").join(format!("{number:03}.custom_ext")),
            "text",
        )
        .map_err(|e| e.to_string())?;
    }
    let mut call = |id: &str, tool: Tool| broker.execute(id, &tool, &mut worker, &cancel);
    let listing = call(
        "list",
        Tool::ListFiles {
            path: ".".into(),
            offset: 0,
        },
    )?;
    assert!(!listing.to_string().contains(".env.json"));
    for (offset, expected, next) in [(0, 200, json!(200)), (200, 5, Value::Null)] {
        let page = call(
            &format!("listing{offset}"),
            Tool::ListFiles {
                path: "paging".into(),
                offset,
            },
        )?;
        assert_eq!(page["ok"], true, "{page}");
        assert_eq!(
            page["result"]["entries"].as_array().unwrap().len(),
            expected
        );
        assert_eq!(page["result"]["next_offset"], next);
    }
    let batch = call(
        "directories",
        Tool::RunBatch {
            tasks: ["folder_a", "folder_b"]
                .iter()
                .enumerate()
                .map(|(index, path)| company_ai::projects::files::batch::Task {
                    task_id: format!("dir{index}"),
                    tool: "list_files".into(),
                    arguments_json: json!({"path":path,"offset":0}).to_string(),
                })
                .collect(),
        },
    )?;
    assert_eq!(batch["ok"], true, "{batch}");
    for task in batch["result"]["tasks"].as_array().unwrap() {
        assert_eq!(task["outcome"]["ok"], true, "{task}");
        assert!(task["outcome"]["result"]
            .to_string()
            .contains("settings.ini"));
    }
    let directory = call(
        "directory_as_file",
        Tool::ReadFile {
            path: "folder_a".into(),
            offset: 0,
        },
    )?;
    assert_eq!(directory["ok"], false);
    assert!(directory["error"].as_str().unwrap().contains("list_files"));
    for (n, name) in [
        "main.cpp",
        "api.h",
        "index.html",
        "app.js",
        "style.css",
        "Cargo.toml",
        "Makefile",
        "CMakeLists.txt",
        "config.ini",
        "script.cmd",
        "script.ps1",
        "unknown.custom_text",
    ]
    .iter()
    .enumerate()
    {
        let read = call(
            &format!("read{n}"),
            Tool::ReadCodeSection {
                path: name.to_string(),
                first_line: 1,
                last_line: 200,
            },
        )?;
        assert_eq!(read["ok"], true, "{read}");
        assert_eq!(read["result"]["read_only"], true);
        assert_eq!(read["result"]["last_line"], 2);
        let copied = call(
            &format!("copy{n}"),
            Tool::CreateWorkingCopy {
                source: Some(name.to_string()),
                name: name.to_string(),
            },
        )?;
        assert_eq!(copied["ok"], true, "{copied}");
        // 本入口只檢查副本建立；丟棄尚未發布的副本，不改寫原件。
        call(
            &format!("discard{n}"),
            Tool::DeleteCopy {
                copy_id: copied["result"]["copy_id"]
                    .as_str()
                    .ok_or("缺少副本代號。")?
                    .into(),
            },
        )?;
    }
    let page = call(
        "large",
        Tool::ReadCodeSection {
            path: "large.cpp".into(),
            first_line: 201,
            last_line: 900,
        },
    )?;
    assert_eq!(page["ok"], true, "{page}");
    assert_eq!(page["result"]["first_line"], 201);
    assert_eq!(page["result"]["last_line"], 400);
    let previous = page["result"]["revision"].clone();
    std::fs::write(root.join("large.cpp"), "// 改版\n").map_err(|e| e.to_string())?;
    let changed = call(
        "changed",
        Tool::ReadCodeSection {
            path: "large.cpp".into(),
            first_line: 1,
            last_line: 1,
        },
    )?;
    assert_ne!(changed["result"]["revision"], previous);
    for (id, path) in [("bad", "bad.cpp"), ("secret", ".env.json")] {
        assert_eq!(
            call(
                id,
                Tool::ReadFile {
                    path: path.into(),
                    offset: 0
                }
            )?["ok"],
            false
        );
    }
    assert!(broker.finish(&[])?.is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("main.cpp")).map_err(|e| e.to_string())?,
        "// 中文\r\nconst value = 42;\r\n"
    );
    println!("PASS source code: formats, text copy creation, >200KB pagination, revision changes, binary/secret rejection, no artifacts and source preservation.");
    Ok(())
}
