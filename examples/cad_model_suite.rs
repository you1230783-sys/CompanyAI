//! 本機模型的CAD情境批次入口。使用正式runner，不代模型產生任何CAD計畫。
//!
//! 本例只建立新圖，沒有桌面確認回呼；目前圖面寫入、重連及部分重做都不自動同意。
use company_ai::{
    local_test::LocalTest,
    projects::{
        runner::{self, Run},
        Store,
    },
    protocol::Message,
    AppResult,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

#[derive(Deserialize)]
struct Case {
    id: String,
    title: String,
    request: String,
    replies: Vec<String>,
}

fn run() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.as_slice() == ["--project-worker"] {
        return company_ai::projects::sandbox::run_worker();
    }
    if args.len() != 5 {
        return Err("用法：cad_model_suite <connection.json> <全新案例目錄> <cases.json> <case_id> <ProgID>".into());
    }
    let folder = PathBuf::from(&args[1]);
    if !folder.is_absolute() || folder.exists() {
        return Err("案例目錄必須是尚未存在的絕對路徑，保留每次實測。".into());
    }
    let cases: Vec<Case> = serde_json::from_slice(&fs::read(&args[2]).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let case = cases
        .into_iter()
        .find(|c| c.id == args[3])
        .ok_or("案例不存在")?;
    let product = match args[4].as_str() {
        "Icad.Application.1" => "佑達ICAD 2008（Icad.Application.1）",
        "GStarCAD.Application.22" => "浩辰CAD 2022（GStarCAD.Application.22）",
        _ => return Err("此案例集只驗證指定的2008／2022，不自動切換版本。".into()),
    };
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mut local = LocalTest::load(std::path::Path::new(&args[0]), &folder.join("app"))?;
    local.config.model = "fast".into();
    // 專案與應用資料分離，與GUI的外部專案資料夾相同，避免WebView等檔案占用干擾。
    let project_root = folder.join("project");
    fs::create_dir(&project_root).map_err(|e| e.to_string())?;
    let mut store = Store::load(&local.root)?;
    store.projects[0].root = project_root.clone();
    store.projects[0].name = case.title.clone();
    store.save(&local.root)?;
    let project = store.projects.remove(0);
    let conversation = company_ai::jobs::new_id()?;
    let mut messages = vec![Message::user(&format!(
        "我這邊用的是{product}，請用這個版本。\n{}",
        case.request
    ))];
    let started = Instant::now();
    let mut rounds = Vec::new();
    let mut preference_replies = Vec::new();
    let mut trace = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(folder.join("events.jsonl"))
        .map_err(|e| e.to_string())?;
    for exchange in 0..=case.replies.len() {
        let id = company_ai::jobs::new_id()?;
        // 模擬使用者按「繼續」：沿用正式加密檢查點，不建立新任務或重送繪圖。
        // 兩次後仍未完成就保留暫停，避免測試器無限代按。
        let mut continuation = 0;
        let answer = loop {
            let mut trace_error = None;
            let inbox = company_ai::projects::steering::Inbox::open(&local.root, &id)?;
            let answer = runner::run(
                Run {
                    id: id.clone(),
                    resume: continuation > 0,
                    project: project.clone(),
                    conversation: conversation.clone(),
                    messages: messages.clone(),
                    config: local.config.clone(),
                    session: local.session.clone(),
                    root: local.root.clone(),
                    cancel: Arc::new(AtomicBool::new(false)),
                    instructions: Some(inbox.clone()),
                    outlook_consent: None,
                    cad_waiter: None,
                    cad_progress: None,
                    file_waiter: None,
                },
                |event| {
                    let saved = (|| -> AppResult<()> {
                        writeln!(trace, "{}", json!({"exchange":exchange,"continuation":continuation,"run_id":id,"event":event}))
                            .map_err(|error| error.to_string())?;
                        // 模擬一般使用者對排版／做法說「你先安排」，不代填尺寸或提供工具參數。
                        // 尚未量測的條件維持待確認，也不授權修改其他文件或失敗後重畫。
                        for question in inbox
                            .questions()?
                            .into_iter()
                            .filter(|q| q.state == "pending")
                        {
                            let reply = ordinary_preference_reply();
                            inbox.answer(&question.id, reply)?;
                            let record = json!({"exchange":exchange,"run_id":id,"simulated_user":true,
                                "question":question.question,"answer":reply});
                            writeln!(trace, "{record}").map_err(|error| error.to_string())?;
                            preference_replies.push(record);
                        }
                        trace.flush().map_err(|error| error.to_string())
                    })();
                    if let Err(error) = saved {
                        trace_error = Some(error);
                    }
                },
            );
            if let Some(error) = trace_error {
                return Err(error);
            }
            let paused = runner::paused_available(&local.root, &id);
            rounds.push(json!({"run_id":id,"exchange":exchange,"continuation":continuation,"paused":paused,"answer":answer}));
            fs::write(
                folder.join("result.json"),
                serde_json::to_vec_pretty(&json!({
                    "case":case.id,"title":case.title,"version":args[4],"gui":false,
                    "formal_runner":true,"elapsed_seconds":started.elapsed().as_secs_f64(),
                    "messages":messages,"rounds":rounds,"paused":paused,
                    "pause_reason":if paused { answer.as_ref().ok() } else {None},
                    "preference_replies":preference_replies
                }))
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let answer = answer?;
            println!(
                "{}",
                json!({"case":case.id,"exchange":exchange,"continuation":continuation,"paused":paused,"answer":answer})
            );
            if !paused || continuation == 2 || contains_failed_report(&project_root)? {
                break answer;
            }
            continuation += 1;
        };
        // 只有真正的必要補充才送預存角色回答。工具失敗後不以新任務重送繪圖。
        if !answer.starts_with("需要你的補充：")
            || exchange == case.replies.len()
            || contains_failed_report(&project_root)?
        {
            break;
        }
        let reply = ordinary_user_reply(&answer, &case.replies[exchange]);
        messages.push(Message::assistant(answer));
        messages.push(Message::user(&reply));
    }
    Ok(())
}

/// 角色會回答實際的選擇題，不只貼尺寸；不提供CAD參數或修改既有文件的授權。
fn ordinary_user_reply(question: &str, facts: &str) -> String {
    if question.contains("目前圖面") && question.contains("新建") {
        format!("請另外新開一張，別改其他已開的圖。{facts}")
    } else {
        facts.to_owned()
    }
}

fn ordinary_preference_reply() -> &'static str {
    "維持我前面給的條件，其他排版和做法請你先安排成討論用概念圖。沒量或還沒確認的現場條件仍要註明待確認，不能當成實測，也不要保證能直接加工或安裝。只處理這張新圖，不動其他已開的圖。"
}

/// 失敗圖面保留，不因模型最後問問題就自動回覆並建立新一輪重做。
fn contains_failed_report(root: &std::path::Path) -> AppResult<bool> {
    let output = root.join("_AI_Output");
    if !output.exists() {
        return Ok(false);
    }
    for entry in fs::read_dir(output).map_err(|e| e.to_string())? {
        let report = entry
            .map_err(|e| e.to_string())?
            .path()
            .join("cad-report.json");
        if report.is_file() {
            let value: serde_json::Value =
                serde_json::from_slice(&fs::read(report).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if value["status"] != "DATA_VERIFIED_VISUAL_PENDING" {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persona_answers_new_versus_existing_without_engineering_parameters() {
        assert!(
            ordinary_user_reply("在目前圖面新增，或新建？", "牆寬60公分")
                .starts_with("請另外新開一張")
        );
        assert_eq!(
            ordinary_user_reply("請問牆有多寬？", "牆寬60公分"),
            "牆寬60公分"
        );
    }
    #[test]
    fn optional_reply_preserves_unknown_conditions_and_other_documents() {
        let reply = ordinary_preference_reply();
        assert!(reply.contains("維持我前面給的條件"));
        assert!(reply.contains("不能當成實測"));
        assert!(reply.contains("不動其他已開的圖"));
        assert!(!reply.chars().any(|character| character.is_ascii_digit()));
    }
}
