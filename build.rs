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
