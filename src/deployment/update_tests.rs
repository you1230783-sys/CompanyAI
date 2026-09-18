//! 使用既有 0.8.0 公開簽署清單驗證選擇流程；測試不持有發行私鑰。
use super::*;
use std::{io::Write, net::TcpListener, thread};

fn fixture(kind: &str) -> UpdateArtifact {
    serde_json::from_str(match kind {
        "exe" => include_str!("fixtures/exe-0.8.0.json"),
        _ => include_str!("fixtures/nsis-0.8.0.json"),
    })
    .unwrap()
}

#[test]
fn valid_manifests_choose_exe_on_tie_and_ignore_forged_latest() {
    let config = Config::default();
    let exe = fixture("exe");
    let nsis = fixture("nsis");
    let mut forged = nsis.clone();
    forged.version = "99.0.0".into();
    for candidates in [
        vec![nsis.clone(), exe.clone(), forged.clone()],
        vec![exe.clone(), forged, nsis.clone()],
    ] {
        assert_eq!(select_latest(&config, candidates).unwrap().kind, "exe");
    }
    assert_eq!(select_latest(&config, vec![nsis]).unwrap().kind, "nsis");
    assert!(validate_artifact(&config, &exe, "0.8.1").is_err());
    let mut changed_kind = exe.clone();
    changed_kind.kind = "nsis".into();
    assert!(validate_manifest(&config, &changed_kind).is_err());
    let mut changed_url = exe;
    changed_url.url = "https://other.example/LM_AI.exe".into();
    assert!(select_latest(&config, vec![changed_url]).is_err());
}

#[test]
fn numeric_version_precedes_package_preference() {
    let mut exe = fixture("exe");
    let mut nsis = fixture("nsis");
    exe.version = "0.9.9".into();
    nsis.version = "0.10.0".into();
    assert!(preference(&nsis).unwrap() > preference(&exe).unwrap());
    exe.version = "1.0.0".into();
    assert!(preference(&exe).unwrap() > preference(&nsis).unwrap());
    nsis.version = exe.version.clone();
    assert!(preference(&exe).unwrap() > preference(&nsis).unwrap());
}

#[test]
fn discovery_checks_all_sources_without_credentials_or_binary_download() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut paths = Vec::new();
        for incoming in listener.incoming().take(3) {
            let mut stream = incoming.unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                assert_eq!(stream.read(&mut byte).unwrap(), 1);
                request.push(byte[0]);
            }
            let request = String::from_utf8(request).unwrap();
            let path = request.split_whitespace().nth(1).unwrap().to_owned();
            assert!(!request.to_lowercase().contains("authorization:"));
            assert!(!request.to_lowercase().contains("cookie:"));
            let (status, body) = if path.ends_with("update-manifest-exe.json") {
                (200, include_str!("fixtures/exe-0.8.0.json"))
            } else if path.ends_with("update-manifest.json") {
                (200, include_str!("fixtures/nsis-0.8.0.json"))
            } else {
                (404, "not published")
            };
            write!(
                stream,
                "HTTP/1.1 {status} Result\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            paths.push(path);
        }
        paths
    });
    let config = Config {
        server_url: format!("http://{address}"),
        ..Config::default()
    };
    assert_eq!(discover_latest(&config).unwrap().kind, "exe");
    assert_eq!(server.join().unwrap(), crate::config::UPDATE_MANIFEST_PATHS);
}

#[test]
fn manifest_array_bounds_and_independent_validation() {
    let exe = fixture("exe");
    let body = serde_json::json!([{"invalid": true}, exe]).to_string();
    let parsed = parse_manifests(&body).unwrap();
    assert_eq!(
        select_latest(&Config::default(), parsed).unwrap().kind,
        "exe"
    );
    assert!(parse_manifests(&" ".repeat(65_537)).is_err());
    assert!(parse_manifests(&serde_json::to_string(&vec![fixture("exe"); 17]).unwrap()).is_err());
    assert!(parse_manifests("<html>download</html>").is_err());
    assert!(select_latest(&Config::default(), vec![]).is_err());
}

#[test]
fn manual_exe_cannot_be_launched_as_an_installer_and_payload_is_verified() {
    let path = std::env::temp_dir().join(format!(
        "LM_AI-update-test-{}",
        crate::jobs::new_id().unwrap()
    ));
    let bytes = b"non-executable verification fixture";
    fs::write(&path, bytes).unwrap();
    let mut artifact = fixture("exe");
    artifact.size = bytes.len() as u64;
    artifact.sha256 = format!("{:x}", Sha256::digest(bytes));
    let mut file = fs::File::open(&path).unwrap();
    verify_file(&mut file, &artifact).unwrap();
    artifact.size += 1;
    assert!(verify_file(&mut file, &artifact).is_err());
    artifact.size -= 1;
    artifact.sha256 = "0".repeat(64);
    assert!(verify_file(&mut file, &artifact).is_err());
    assert_eq!(artifact.file_name().unwrap(), "LM_AI.exe");
    assert_eq!(fixture("nsis").file_name().unwrap(), "LM_AI_Setup.exe");
    let ready = ReadyUpdate {
        artifact,
        path: path.clone(),
        _locked_file: file,
    };
    assert!(launch_update(&ready, &Config::default(), "99.0.0")
        .unwrap_err()
        .contains("手動"));
    drop(ready);
    fs::remove_file(path).unwrap();
}
