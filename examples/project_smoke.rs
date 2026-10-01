//! 使用真正的 AppContainer EXE 驗證文件工具。僅在指定測試資料夾建立測試檔，不操作使用者文件。
#[path = "project_smoke/continuation.rs"]
mod continuation;
#[path = "project_smoke/interruption.rs"]
mod interruption;
#[path = "project_smoke/memory.rs"]
mod memory;
#[path = "project_smoke/pause.rs"]
mod pause;
#[path = "project_smoke/pdf.rs"]
mod pdf;
#[path = "project_smoke/roundtrip.rs"]
mod roundtrip;
#[path = "project_smoke/server_pdf.rs"]
mod server_pdf;

use company_ai::{
    projects::{files::Broker, sandbox::Worker, text, Project, Tool},
    AppResult,
};
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::AtomicBool};

fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().collect();
    let exe = PathBuf::from(args.get(1).ok_or("需要待測 EXE 路徑。")?);
    let root =
        PathBuf::from(args.get(2).ok_or("需要測試根目錄。")?).join(company_ai::jobs::new_id()?);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let cancel = AtomicBool::new(false);
    let mut worker = Worker::start(&exe, &cancel)?;
    let source = "原始😀文字\r\n保留原檔。";
    std::fs::write(root.join("source.txt"), source).map_err(|e| e.to_string())?;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let isolation = worker.inspect_isolation(
        &root.join("source.txt"),
        listener.local_addr().map_err(|e| e.to_string())?.port(),
        &cancel,
    )?;
    assert_eq!(isolation["read_denied"], true, "{isolation}");
    assert_eq!(isolation["write_denied"], true, "{isolation}");
    assert_eq!(isolation["network_denied"], true, "{isolation}");
    let project = Project {
        id: "smoke".into(),
        name: "測試".into(),
        root: root.clone(),
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project, "task1".into())?;
    let mut execute = |id: &str, tool: Tool| broker.execute(id, &tool, &mut worker, &cancel);
    let result = execute(
        "read",
        Tool::ReadFile {
            path: "source.txt".into(),
            offset: 0,
        },
    )?;
    assert_eq!(result["result"]["text"], source);
    let copy = execute(
        "copy",
        Tool::CreateWorkingCopy {
            source: Some("source.txt".into()),
            name: "修訂.txt".into(),
        },
    )?;
    let id = copy["result"]["copy_id"]
        .as_str()
        .ok_or_else(|| copy.to_string())?
        .to_owned();
    let edit = Tool::EditText {
        copy_id: id.clone(),
        revision: text::revision(source),
        start: 2,
        expected: "😀".into(),
        replacement: "修改".into(),
    };
    let modified = execute("edit", edit.clone())?;
    assert_eq!(modified["ok"], true, "{modified}");
    assert_eq!(execute("edit", edit)?, modified, "重播不得再次修改");
    let revision = modified["result"]["revision"]
        .as_str()
        .ok_or("缺少版本。")?
        .to_owned();
    let saved = execute(
        "save",
        Tool::SaveCopy {
            copy_id: id.clone(),
            revision,
        },
    )?;
    assert_eq!(saved["ok"], true, "{saved}");
    assert_eq!(
        execute(
            "delete_original",
            Tool::DeleteCopy {
                copy_id: "source.txt".into()
            }
        )?["ok"],
        false
    );
    assert_eq!(
        execute(
            "delete_published",
            Tool::DeleteCopy {
                copy_id: id.clone()
            }
        )?["ok"],
        false
    );
    assert_eq!(
        execute(
            "outside",
            Tool::ReadFile {
                path: "../outside.txt".into(),
                offset: 0
            }
        )?["ok"],
        false
    );
    std::fs::hard_link(root.join("source.txt"), root.join("hardlink.txt"))
        .map_err(|e| e.to_string())?;
    assert_eq!(
        execute(
            "hardlink",
            Tool::ReadFile {
                path: "hardlink.txt".into(),
                offset: 0
            }
        )?["ok"],
        false
    );
    assert_eq!(
        std::fs::read_to_string(root.join("source.txt")).map_err(|e| e.to_string())?,
        source
    );
    let paths = broker.finish(&[id])?;
    assert_eq!(paths.len(), 1);
    println!("PASS: AppContainer handshake, OS file/network isolation, Unicode edit, replay, source protection, publish/readback, path escape and hardlink rejection. Fixture: {}", root.display());
    drop(worker);
    roundtrip::verify(&root)?;
    continuation::verify(&root)?;
    pause::verify(&root)?;
    interruption::verify(&root)?;
    memory::verify(&exe, &root)?;
    pdf::verify(&exe, &root)?;
    server_pdf::verify(&exe, &root)?;
    Ok(())
}
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--project-worker") {
        if company_ai::projects::sandbox::run_worker().is_err() {
            std::process::exit(1);
        }
        return;
    }
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
