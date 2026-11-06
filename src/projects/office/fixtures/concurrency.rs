//! 真實 COM 並行回歸：只建立本次測試文件，所有 guard 留在取得它的 STA 執行緒。
use super::*;
use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

fn save(session: &Session, path: &Path) -> AppResult<()> {
    invoke(
        session.document()?,
        if session.ext == "docx" {
            "SaveAs2"
        } else {
            "SaveAs"
        },
        vec![
            path.to_string_lossy().as_ref().into(),
            save_format(&session.ext)?.into(),
        ],
        false,
    )?;
    Ok(())
}
fn word(path: &Path, text: &str) -> AppResult<Session> {
    let session = Session::start(path, true)?;
    set(&child(session.document()?, "Content")?, "Text", text.into())?;
    Ok(session)
}

pub fn verify(folder: &Path) -> AppResult<serde_json::Value> {
    let _apartment = Apartment::sta().map_err(|e| e.to_string())?;
    if busy_before_dispatch()?.is_some() {
        return Err("並行驗收前請先關閉使用中的 Office 文件。".into());
    }
    let a_path = folder.join("task-a.docx");
    let b_path = folder.join("task-b.docx");
    let excel_path = folder.join("independent.xlsx");
    let a = word(&a_path, "Task A only")?;
    let cancel = Arc::new(AtomicBool::new(false));
    let (wait_tx, wait_rx) = mpsc::channel();
    let (opened_tx, opened_rx) = mpsc::channel();
    let worker_cancel = cancel.clone();
    let b_output = b_path.clone();
    let b = std::thread::spawn(move || -> AppResult<()> {
        let _context =
            crate::resource_locks::Context::enter(Some(crate::resource_locks::Context {
                label: "B".into(),
                cancel: worker_cancel,
                notify: Arc::new(move |text| {
                    if text.is_some() {
                        let _ = wait_tx.send(text);
                    }
                }),
            }));
        let _apartment = Apartment::sta().map_err(|e| e.to_string())?;
        let session = word(&b_output, "Task B only")?;
        opened_tx.send(()).map_err(|e| e.to_string())?;
        save(&session, &b_output)
    });
    let result = (|| {
        let waiting = wait_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|e| format!("B 未進入等待：{e}"))?;
        if !waiting.unwrap_or_default().contains("Word") || opened_rx.try_recv().is_ok() {
            return Err("Word 擁有權未序列化。".into());
        }
        let output = excel_path.clone();
        std::thread::spawn(move || -> AppResult<()> {
            let _apartment = Apartment::sta().map_err(|e| e.to_string())?;
            let session = Session::start(&output, true)?;
            let sheet = item(&child(session.document()?, "Worksheets")?, 1)?;
            let cell = obj(invoke(&sheet, "Range", vec!["A1".into()], false)?)?;
            set(&cell, "Value2", 42i32.into())?;
            if integer(&cell, "Value2")? != 42 {
                return Err("Excel 讀回不符。".into());
            }
            save(&session, &output)
        })
        .join()
        .map_err(|_| "Excel 驗收執行緒中止")??;
        // Excel 已完成，此時 Word A 仍持有鎖，B 必須尚未開檔。
        if opened_rx.try_recv().is_ok() {
            return Err("B 提前開啟 Word。".into());
        }
        save(&a, &a_path)
    })();
    if result.is_err() {
        cancel.store(true, Ordering::Relaxed);
    }
    drop(a);
    let b_result = b.join().map_err(|_| "Word B 驗收執行緒中止")?;
    result?;
    b_result?;
    for (path, expected) in [(&a_path, "Task A only"), (&b_path, "Task B only")] {
        let session = Session::open(path)?;
        let actual = string(&get(&child(session.document()?, "Content")?, "Text")?)?;
        if actual.trim() != expected {
            return Err("兩份 Word 成果互相污染。".into());
        }
    }

    // 模擬使用者自己開啟 Word：不用資源鎖，僅關閉這個固定測試文件。
    struct External {
        app: IDispatch,
        document: IDispatch,
    }
    impl Drop for External {
        fn drop(&mut self) {
            let _ = invoke(&self.document, "Close", vec![false.into()], false);
            if child(&self.app, "Documents")
                .and_then(|docs| integer(&docs, "Count"))
                .ok()
                == Some(0)
            {
                let _ = invoke(&self.app, "Quit", vec![], false);
            }
        }
    }
    let class = unsafe { CLSIDFromProgID(PCWSTR(wide("Word.Application").as_ptr())) }
        .map_err(|e| e.to_string())?;
    let app: IDispatch = unsafe { CoCreateInstance(&class, None, CLSCTX_LOCAL_SERVER) }
        .map_err(|e| e.to_string())?;
    if integer(&child(&app, "Documents")?, "Count")? != 0 {
        return Err("已有使用者 Word 文件；不建立占用測試。".into());
    }
    let document = obj(invoke(
        &child(&app, "Documents")?,
        "Add",
        vec![missing(), false.into(), 0i32.into(), false.into()],
        false,
    )?)?;
    let external = External { app, document };
    // 讓 Office 完成 ROT 登記；這是前置條件，不能將未登錄的測試實例當成占用案例。
    let mut registered = false;
    for _ in 0..20 {
        if busy_before_dispatch()?.is_some() {
            registered = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if !registered {
        return Err("外部 Word 測試實例尚未登錄 ROT。".into());
    }
    match Session::open(&a_path) {
        Err(error) if error.contains(super::super::super::interaction::BUSY) => (),
        Err(error) => return Err(format!("外部 Word 占用錯誤分類不符：{error}")),
        Ok(_) => return Err("未辨識外部 Word 占用。".into()),
    }
    drop(external);
    Ok(
        serde_json::json!({"result":"PASS", "word_word":"B waits until A closes; both files reopened and checked",
        "word_excel":"Excel saved and read while A owns Word and B waits", "external_word":"existing BUSY marker preserved",
        "files":[a_path,b_path,excel_path], "company_documents":false}),
    )
}
