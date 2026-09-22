//! 僅使用隔離的虛構機台檔；不啟動真實 Viewer 或連線公司機台。
use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".build/vnc-tests")
            .join(crate::jobs::new_id().unwrap());
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn path(&self) -> PathBuf {
        self.0.join("machines.json")
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        fs::write(self.0.join(name), bytes).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const SAMPLE: &str = r#"{"A":[{"name":"機台1","ip":"192.168.1.101","password":"test-secret","note":"保留"}],"B":[]}"#;

#[test]
fn legacy_format_round_trips_without_exposing_passwords() {
    let fixture = Fixture::new();
    fixture.write("machines.json", SAMPLE.as_bytes());
    fixture.write("user_config.json", br#"{"vnc_path":"C:\\UltraVNC\\vncviewer.exe","options":{"fullscreen":true,"viewonly":true,"autoscaling":false,"custom":4},"legacy":true}"#);
    let mut manager = Manager::load(fixture.path()).unwrap();
    assert!(manager.config.options.fullscreen && manager.config.options.viewonly);
    assert!(!manager.config.options.autoscaling);
    assert!(!manager.public_groups().to_string().contains("test-secret"));
    assert_eq!(
        manager.public_groups()[0]["machines"][0]["has_password"],
        true
    );
    let mut machines = manager.machines.clone();
    machines.get_mut("A").unwrap()[0].name = "新名稱".into();
    manager.save_machines(machines).unwrap();
    manager.save_config(manager.config.clone()).unwrap();
    let stored: Value = serde_json::from_slice(&fs::read(fixture.path()).unwrap()).unwrap();
    assert_eq!(stored["A"][0]["name"], "新名稱");
    assert_eq!(stored["A"][0]["password"], "test-secret");
    assert_eq!(stored["A"][0]["note"], "保留");
    assert_eq!(stored["B"], json!([]));
    let config: Value =
        serde_json::from_slice(&fs::read(fixture.0.join("user_config.json")).unwrap()).unwrap();
    assert_eq!(config["legacy"], true);
    assert_eq!(config["options"]["custom"], 4);
}

#[test]
fn absent_files_stay_absent_until_an_explicit_save() {
    let fixture = Fixture::new();
    let mut manager = Manager::load(fixture.path()).unwrap();
    assert!(manager.machines.is_empty());
    assert!(manager.config.options.autoscaling);
    assert!(!fixture.path().exists());
    assert!(!fixture.0.join("user_config.json").exists());
    manager
        .save_machines(decode(SAMPLE.as_bytes()).unwrap())
        .unwrap();
    assert_eq!(
        Manager::load(fixture.path()).unwrap().machines["A"][0].name,
        "機台1"
    );
}

#[test]
fn malformed_json_never_replaces_original_files_or_leaks_values() {
    let fixture = Fixture::new();
    let broken = br#"{"A":[{"name":"secret-to-hide"}]}"#;
    fixture.write("machines.json", broken);
    let error = Manager::load(fixture.path()).err().unwrap();
    assert!(!error.contains("secret-to-hide"));
    assert_eq!(fs::read(fixture.path()).unwrap(), broken);
    fixture.write("machines.json", SAMPLE.as_bytes());
    fixture.write("user_config.json", br#"{"options":[]} broken"#);
    assert!(Manager::load(fixture.path()).is_err());
    assert_eq!(fs::read(fixture.path()).unwrap(), SAMPLE.as_bytes());
}

#[test]
fn external_changes_reject_saves_and_preserve_memory() {
    let fixture = Fixture::new();
    fixture.write("machines.json", SAMPLE.as_bytes());
    let mut manager = Manager::load(fixture.path()).unwrap();
    fixture.write("machines.json", b"{\"external\":[]}");
    assert!(manager.save_machines(Machines::new()).is_err());
    assert!(manager.machines.contains_key("A"));
    assert_eq!(fs::read(fixture.path()).unwrap(), b"{\"external\":[]}");
    fixture.write("user_config.json", b"{}");
    assert!(manager.save_config(UserConfig::default()).is_err());
}

#[test]
fn write_failure_does_not_change_loaded_machines() {
    let fixture = Fixture::new();
    fixture.write("machines.json", SAMPLE.as_bytes());
    let mut manager = Manager::load(fixture.path()).unwrap();
    // 佔住 atomic_write 的暫存檔位置，模擬無法寫入而不依賴本機 ACL。
    fs::create_dir(
        fixture
            .path()
            .with_extension(format!("{}.tmp", std::process::id())),
    )
    .unwrap();
    assert!(manager.save_machines(Machines::new()).is_err());
    assert!(manager.machines.contains_key("A"));
    assert_eq!(fs::read(fixture.path()).unwrap(), SAMPLE.as_bytes());
}

#[test]
fn viewer_arguments_preserve_python_order_and_password_argument_boundary() {
    let mut machine: Machine =
        decode(br#"{"name":"test","ip":"host::5901","password":"p a&ss\"word"}"#).unwrap();
    let args = connection_args(&machine, &Options::default()).unwrap();
    assert_eq!(
        args,
        [
            "/password",
            "p a&ss\"word",
            "/shared",
            "/autoscaling",
            "/autoreconnect",
            "5",
            "/reconnectcounter",
            "3",
            "/quickoption",
            "7",
            "host::5901"
        ]
    );
    machine.password.clear();
    let options = Options {
        fullscreen: true,
        viewonly: true,
        autoscaling: false,
        ..Options::default()
    };
    assert_eq!(
        connection_args(&machine, &options).unwrap(),
        [
            "/shared",
            "/fullscreen",
            "/viewonly",
            "/autoreconnect",
            "5",
            "/reconnectcounter",
            "3",
            "/quickoption",
            "7",
            "host::5901"
        ]
    );
}

#[test]
fn server_rejects_commands_but_accepts_legacy_host_forms() {
    for server in [
        "192.168.1.101",
        "machine-1",
        "machine:1",
        "machine::5901",
        "[::1]::5900",
    ] {
        assert!(validate_machine("A", "機台", server, "").is_ok());
    }
    for server in [
        "",
        "/listen",
        "-listen",
        "host /password x",
        "host&calc",
        "host\n",
    ] {
        assert!(validate_machine("A", "機台", server, "").is_err());
    }
}

#[test]
fn utf8_bom_and_missing_options_are_supported() {
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend_from_slice(SAMPLE.as_bytes());
    let machines: Machines = decode(&bytes).unwrap();
    assert_eq!(machines["A"][0].ip, "192.168.1.101");
    let config: UserConfig = decode(br#"{"options":{"viewonly":true}}"#).unwrap();
    assert!(config.options.viewonly && config.options.autoscaling);
    assert!(!config.options.fullscreen);
}

#[test]
fn manual_order_survives_save_reload_without_name_sorting() {
    let fixture = Fixture::new();
    let mut manager = Manager::load(fixture.path()).unwrap();
    let mut machines = Machines::new();
    let list = machines.entry("A".into()).or_default();
    for name in ["機台10", "備用機", "機台2", "機台1"] {
        list.push(Machine {
            name: name.into(),
            ip: "127.0.0.1".into(),
            password: String::new(),
            extra: Map::new(),
        });
    }
    list.swap(0, 1);
    list[1].password = "changed-without-moving".into();
    manager.save_machines(machines).unwrap();
    let reloaded = Manager::load(fixture.path()).unwrap();
    assert_eq!(reloaded.machines["A"][0].name, "備用機");
    assert_eq!(reloaded.machines["A"][1].name, "機台10");
    assert_eq!(reloaded.machines["A"][1].password, "changed-without-moving");
}
