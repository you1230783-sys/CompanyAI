#![windows_subsystem = "windows"]
//! 雙擊 EXE 直接開啟視窗；--demo 提供本機流程示範，--self-check 供編譯腳本驗證。
use company_ai::{demo::DemoServer, ui, AppResult};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if matches!(
        arguments.first().map(String::as_str),
        Some("--local-ai-test" | "--local-cad-probe")
    ) {
        let result = (|| {
            let probe = arguments[0] == "--local-cad-probe";
            if arguments.len() != if probe { 4 } else { 3 } {
                return Err("用法：--local-ai-test <connection.json> <全新資料夾>；--local-cad-probe 另加 <request.txt>".into());
            }
            let local = company_ai::local_test::LocalTest::load(
                std::path::Path::new(&arguments[1]),
                std::path::Path::new(&arguments[2]),
            )?;
            if probe {
                local.probe(std::path::Path::new(&arguments[3]))
            } else {
                ui::run_local(local)
            }
        })();
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    // 離線驗收只代替模型產生固定測試計畫，仍走正式 Broker 與 CAD worker。
    if arguments.first().map(String::as_str) == Some("--cad-offline-test") {
        if arguments.len() != 3 {
            eprintln!("用法：LM_AI.exe --cad-offline-test <報告根目錄> <CAD ProgID>");
            std::process::exit(2);
        }
        if let Err(error) =
            company_ai::cad::offline::run(std::path::Path::new(&arguments[1]), &arguments[2])
        {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if arguments.first().map(String::as_str) == Some("--verify-update-package")
        && arguments.len() == 3
    {
        let result = company_ai::deployment::verify_release(
            std::path::Path::new(&arguments[1]),
            std::path::Path::new(&arguments[2]),
        );
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if arguments.as_slice() == ["--project-worker"] {
        if company_ai::projects::sandbox::run_worker().is_err() {
            std::process::exit(1);
        }
        return;
    }
    if arguments.as_slice() == ["--outlook-worker"] {
        if company_ai::outlook::process::run_worker().is_err() {
            std::process::exit(1);
        }
        return;
    }
    if arguments.as_slice() == ["--prepare-python-runtime"]
        || arguments.as_slice() == ["--python-self-check"]
    {
        let result = if arguments[0] == "--prepare-python-runtime" {
            company_ai::projects::python::prepare_runtime()
        } else {
            company_ai::projects::python::self_check()
        };
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if arguments.as_slice() == ["--cad-self-check"] {
        if let Err(error) = company_ai::cad::self_check() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let smoke = arguments.iter().any(|argument| argument == "--self-check");
    let result: AppResult<()> = (|| {
        if arguments
            .iter()
            .any(|argument| !matches!(argument.as_str(), "--demo" | "--self-check"))
        {
            return Err("支援的參數為 --demo 或 --self-check；直接開啟則使用公司連線設定。".into());
        }
        let demo = if arguments.iter().any(|argument| argument == "--demo") {
            Some(DemoServer::start()?)
        } else {
            None
        };
        ui::run(demo.as_ref(), smoke)
    })();
    if let Err(error) = result {
        if smoke {
            eprintln!("{error}");
        } else {
            ui::show_fatal_error(&error);
        }
        std::process::exit(1);
    }
}
