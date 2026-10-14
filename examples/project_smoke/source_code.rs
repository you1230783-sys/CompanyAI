//! 基礎驗證入口：真實唯讀檔案與原生broker，無模型服務或使用者專案。
use super::*;

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
    for name in [
        "main.cpp",
        "api.h",
        "index.html",
        "app.js",
        "style.css",
        "Cargo.toml",
        "Makefile",
        "CMakeLists.txt",
    ] {
        std::fs::write(root.join(name), "// 中文\r\nconst value = 42;\r\n")
            .map_err(|e| e.to_string())?;
    }
    let large = "// 中文大檔\n".repeat(30_000);
    std::fs::write(root.join("large.cpp"), &large).map_err(|e| e.to_string())?;
    std::fs::write(root.join("bad.cpp"), b"bad\0binary").map_err(|e| e.to_string())?;
    std::fs::write(root.join(".env.json"), "secret").map_err(|e| e.to_string())?;
    let mut call = |id: &str, tool: Tool| broker.execute(id, &tool, &mut worker, &cancel);
    let listing = call("list", Tool::ListFiles { path: ".".into() })?;
    assert!(!listing.to_string().contains(".env.json"));
    for (n, name) in [
        "main.cpp",
        "api.h",
        "index.html",
        "app.js",
        "style.css",
        "Cargo.toml",
        "Makefile",
        "CMakeLists.txt",
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
        let denied = call(
            &format!("copy{n}"),
            Tool::CreateWorkingCopy {
                source: Some(name.to_string()),
                name: name.to_string(),
            },
        )?;
        assert_eq!(denied["ok"], false, "{denied}");
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
    println!("PASS source code: formats, read-only copy rejection, >200KB pagination, revision changes, binary/secret rejection, no artifacts and source preservation.");
    Ok(())
}
