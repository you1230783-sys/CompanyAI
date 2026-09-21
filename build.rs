//! 將前端及第三方離線資源嵌入 EXE；編譯不需要 Node、npm 或外部 CDN。
use std::{
    env, fs,
    path::{Path, PathBuf},
};
fn collect(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read bundled UI directory") {
        let path = entry.expect("read UI entry").path();
        if path.is_dir() {
            collect(root, &path, files);
        } else if path.starts_with(root) {
            files.push(path);
        }
    }
}
fn main() {
    // 多尺寸 ICO 嵌入 EXE，檔案總管／視窗／工作列／托盤共用資源 ID 1。
    // 圖示由 scripts/Convert-AppIcon.ps1 產生，離線編譯直接使用已交付的 ICO。
    let icon = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"))
        .join("assets/app.ico");
    println!("cargo:rerun-if-changed=assets");
    assert!(
        icon.is_file(),
        "Required application icon assets/app.ico is missing"
    );
    {
        let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
        let source = output.join("app-icon.rc");
        let resource = output.join("app-icon.res");
        fs::write(
            &source,
            format!(
                r#"1 ICON "{}"
1 VERSIONINFO
 FILEVERSION {version}
 PRODUCTVERSION {version}
 FILEFLAGSMASK 0x3fL
 FILEFLAGS 0x0L
 FILEOS 0x40004L
 FILETYPE 0x1L
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040404b0"
  BEGIN
   VALUE "CompanyName", "Largan, Inc."
   VALUE "ProductName", "LM_AI"
   VALUE "FileDescription", "{description}（Dev: 1230783）"
   VALUE "FileVersion", "{display_version}.0"
   VALUE "ProductVersion", "{display_version}.0"
   VALUE "LegalCopyright", "Copyright © 2026 Largan, Inc. All rights reserved."
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x0404, 1200
 END
END
"#,
                icon.to_string_lossy().replace('\\', "/"),
                version = format!(
                    "{},0",
                    env::var("CARGO_PKG_VERSION")
                        .expect("package version")
                        .replace('.', ",")
                ),
                display_version = env::var("CARGO_PKG_VERSION").expect("package version"),
                description = "公司 AI 助理"
            ),
        )
        .expect("write icon resource script");
        let status = std::process::Command::new("rc.exe")
            .args(["/nologo", "/c65001", "/fo"])
            .arg(&resource)
            .arg(&source)
            .status()
            .expect("Windows SDK rc.exe is required when assets/app.ico is provided");
        assert!(status.success(), "Icon resource compilation failed");
        println!("cargo:rustc-link-arg-bins={}", resource.display());
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"))
        .join("ui");
    println!("cargo:rerun-if-changed=ui");
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort();
    let mut generated = String::from("pub static ASSETS: &[(&str, &[u8])] = &[\n");
    for file in files {
        let name = file
            .strip_prefix(&root)
            .expect("UI relative path")
            .to_string_lossy()
            .replace('\\', "/");
        generated.push_str(&format!(
            "({:?}, include_bytes!({:?})),\n",
            format!("/{name}"),
            file.to_string_lossy()
        ));
    }
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory")).join("assets.rs"),
        generated,
    )
    .expect("write asset table");
}
