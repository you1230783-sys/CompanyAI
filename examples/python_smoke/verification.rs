//! 實際執行交付副本的需求測試；涵蓋錯誤被發現、修正、隔離與證據版本。
use super::*;

pub(super) fn run(project: &Project, worker: &mut Worker) -> AppResult<()> {
    let root = project.root.join("code-verification");
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    let project = Project {
        id: "verification".into(),
        name: "verification".into(),
        root: root.clone(),
        imports: Default::default(),
    };
    let original = "def convert(value=None):\n    return value * 2\n\ndef fetch():\n    from external_service import request\n    return request()\n";
    std::fs::write(root.join("source.py"), original).map_err(|e| e.to_string())?;
    let mut broker = Broker::new(project, "verification".into())?;
    call(
        &mut broker,
        worker,
        "skill",
        json!({"tool":"load_skill","id":"python-edit"}),
    )?;
    let copy = call(
        &mut broker,
        worker,
        "copy",
        json!({"tool":"create_working_copy","source":"source.py","name":"changed.py"}),
    )?;
    let id = copy["copy_id"].as_str().unwrap();
    call(
        &mut broker,
        worker,
        "plan",
        json!({"tool":"plan_code_change","copy_id":id,"requirements":[{"id":"R1","description":"convert預設為0，正常值乘2","origin":"user"},{"id":"R2","description":"隔離目錄內可建立測試輸出","origin":"preserve"},{"id":"R3","description":"fetch保留外部服務回傳值與錯誤","origin":"preserve"}]}),
    )?;
    call(
        &mut broker,
        worker,
        "syntax",
        json!({"tool":"check_python","path":id,"revision":copy["revision"]}),
    )?;
    let cases = json!([{"id":"defaults","requirement_ids":["R1"],"mocked_dependencies":[],"code":"class Defaults(unittest.TestCase):\n def test_default(self):\n  target=load_target()\n  self.assertEqual(target.convert(),0)\n def test_value(self):\n  self.assertEqual(load_target().convert(3),6)\n"}]);
    let bad = call(
        &mut broker,
        worker,
        "bad-test",
        json!({"tool":"test_python","copy_id":id,"revision":copy["revision"],"tests":cases}),
    )?;
    assert_eq!(bad["test_report"]["tests"][0]["status"], "failed", "{bad}");
    assert!(bad.to_string().contains("TypeError"));
    call(
        &mut broker,
        worker,
        "inspect",
        json!({"tool":"review_code_change","copy_id":id,"revision":copy["revision"],"checks":[]}),
    )?;
    let lie: Tool = serde_json::from_value(json!({"tool":"review_code_change","copy_id":id,"revision":copy["revision"],"checks":[
        {"requirement_id":"R1","status":"tested","evidence":"fake","first_line":1,"last_line":2,"test_ids":["defaults"]},
        {"requirement_id":"R2","status":"reviewed","evidence":"not run","first_line":1,"last_line":2,"test_ids":[]},
        {"requirement_id":"R3","status":"unverified","evidence":"not run","first_line":4,"last_line":6,"test_ids":[]}]})).unwrap();
    assert_eq!(
        broker.execute("reject-fake-test", &lie, worker, &AtomicBool::new(false))?["ok"],
        false
    );
    let section = call(
        &mut broker,
        worker,
        "section",
        json!({"tool":"read_code_section","path":id,"first_line":1,"last_line":2}),
    )?;
    let fixed = call(
        &mut broker,
        worker,
        "fix",
        json!({"tool":"edit_code_section","copy_id":id,"revision":copy["revision"],"first_line":1,"last_line":2,"section_hash":section["section_hash"],"replacement":"def convert(value=0):\n    return value * 2\n"}),
    )?;
    call(
        &mut broker,
        worker,
        "syntax-fixed",
        json!({"tool":"check_python","path":id,"revision":fixed["revision"]}),
    )?;
    let mut cases = cases.as_array().unwrap().clone();
    // AppContainer 的封鎖可能表現為逾時，而不是固定 Winsock 錯誤碼。
    // 以父程序可連線的真實 listener 作對照，並核對子程序未建立連線。
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let address = listener.local_addr().map_err(|e| e.to_string())?;
    let control = std::net::TcpStream::connect_timeout(&address, Duration::from_secs(1))
        .map_err(|e| e.to_string())?;
    let (accepted, _) = listener.accept().map_err(|e| e.to_string())?;
    drop((control, accepted));
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    cases.push(
        json!({"id":"isolation","requirement_ids":["R2"],"mocked_dependencies":[],"code":format!(r#"
import pathlib, socket
class Isolation(unittest.TestCase):
 def test_workspace(self):
  load_target()
  output=pathlib.Path(workspace)/'result.txt'
  output.write_text('result',encoding='utf-8')
  self.assertEqual(output.read_text(encoding='utf-8'),'result')
  with self.assertRaises(OSError):
   pathlib.Path({source_path}).read_text()
  with self.assertRaises(OSError):
   socket.create_connection(('127.0.0.1',{port}),timeout=1)
"#,port=address.port(),source_path=serde_json::to_string(&root.join("source.py").to_string_lossy()).unwrap())}),
    );
    cases.push(json!({"id":"missing","requirement_ids":["R1"],"mocked_dependencies":[],"code":"class Missing(unittest.TestCase):\n def test_missing(self):\n  load_target()\n  import lmai_absent_dependency_fixture\n"}));
    cases.push(json!({"id":"service","requirement_ids":["R3"],"mocked_dependencies":["external_service：僅模擬回傳與錯誤，未連線"],"code":r#"
import types
class Service(unittest.TestCase):
 def test_result_and_error(self):
  target=load_target()
  service=types.ModuleType('external_service')
  service.request=mock.Mock(return_value=42)
  with mock.patch.dict('sys.modules', {'external_service':service}):
   self.assertEqual(target.fetch(),42)
   service.request.assert_called_once_with()
   service.request.side_effect=RuntimeError('fixture failure')
   with self.assertRaises(RuntimeError):
    target.fetch()
"#}));
    let good = call(
        &mut broker,
        worker,
        "good-test",
        json!({"tool":"test_python","copy_id":id,"revision":fixed["revision"],"tests":cases}),
    )?;
    assert_eq!(
        good["test_report"]["tests"][0]["status"], "passed",
        "{good}"
    );
    assert_eq!(
        good["test_report"]["tests"][1]["status"], "passed",
        "{good}"
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(
        good["test_report"]["tests"][2]["status"], "unavailable",
        "{good}"
    );
    assert_eq!(
        good["test_report"]["tests"][3]["status"], "passed",
        "{good}"
    );
    let no_evidence = call(
        &mut broker,
        worker,
        "no-evidence",
        json!({"tool":"test_python","copy_id":id,"revision":fixed["revision"],"tests":[
            {"id":"no-source","requirement_ids":["R1"],"mocked_dependencies":[],"code":"class Empty(unittest.TestCase):\n def test_unrelated(self):\n  self.assertTrue(True)\n"},
            {"id":"no-cases","requirement_ids":["R1"],"mocked_dependencies":[],"code":"load_target()"}
        ]}),
    )?;
    assert!(no_evidence["test_report"]["tests"]
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t["status"] == "unavailable"));
    call(
        &mut broker,
        worker,
        "inspect-fixed",
        json!({"tool":"review_code_change","copy_id":id,"revision":fixed["revision"],"checks":[]}),
    )?;
    call(
        &mut broker,
        worker,
        "review-fixed",
        json!({"tool":"review_code_change","copy_id":id,"revision":fixed["revision"],"checks":[
        {"requirement_id":"R1","status":"tested","evidence":"預設及正常案例通過；缺少外部依賴案例仍未驗證。","first_line":1,"last_line":2,"test_ids":["defaults"]},
        {"requirement_id":"R2","status":"tested","evidence":"私有目錄寫入讀回，來源檔及網路存取被拒絕。","first_line":1,"last_line":2,"test_ids":["isolation"]},
        {"requirement_id":"R3","status":"tested","evidence":"僅模擬外部服務回傳與錯誤，未連線真實服務。","first_line":4,"last_line":6,"test_ids":["service"]}]}),
    )?;
    call(
        &mut broker,
        worker,
        "save",
        json!({"tool":"save_copy","copy_id":id,"revision":fixed["revision"]}),
    )?;
    assert_eq!(broker.finish(&[id.into()])?.len(), 1);
    assert_eq!(
        std::fs::read_to_string(root.join("source.py")).unwrap(),
        original
    );
    std::fs::write(
        root.join("verification.json"),
        serde_json::to_vec_pretty(
            &json!({"result":"PASS","parent_loopback_control":true,"child_loopback_connected":false,"detected":bad,"corrected":good,"no_evidence":no_evidence}),
        )
        .unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!("PASS Python acceptance: real source default bug detected and corrected, private workspace, no original/network access, missing dependencies distinguished, false evidence rejected.");
    Ok(())
}
