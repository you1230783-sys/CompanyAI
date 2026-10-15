//! 程式與設定檔唯讀能力；與既有副本寫入白名單分開，避免加副檔名就連帶開放編輯。
use super::*;

pub(super) const MAX_BYTES: usize = 4 * 1024 * 1024;

/// 產物、版本庫與常見機密檔不作為一般程式閱讀來源；明確路徑也不能繞過。
pub(super) fn excluded(path: &Path) -> bool {
    path.components().any(|part| {
        let name = part.as_os_str().to_string_lossy().to_ascii_lowercase();
        matches!(
            name.as_str(),
            ".git"
                | ".svn"
                | ".hg"
                | ".lmai"
                | "node_modules"
                | "target"
                | "__pycache__"
                | ".venv"
                | "venv"
                | "bin"
                | "obj"
                | ".aws"
                | ".ssh"
                | "credentials.json"
                | "secrets.json"
        ) || name == ".env"
            || name.starts_with(".env.")
    })
}

pub(super) fn supported(path: &Path) -> bool {
    if excluded(path) {
        return false;
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "c" | "cc"
            | "cpp"
            | "cxx"
            | "h"
            | "hh"
            | "hpp"
            | "hxx"
            | "inl"
            | "ipp"
            | "tpp"
            | "ixx"
            | "cppm"
            | "c++"
            | "h++"
            | "cu"
            | "cuh"
            | "rc"
            | "rc2"
            | "idl"
            | "py"
            | "pyi"
            | "rs"
            | "cs"
            | "java"
            | "kt"
            | "go"
            | "html"
            | "htm"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "ts"
            | "tsx"
            | "css"
            | "scss"
            | "less"
            | "vue"
            | "svelte"
            | "json"
            | "yaml"
            | "yml"
            | "toml"
            | "xml"
            | "ini"
            | "cfg"
            | "cmake"
            | "sln"
            | "vcxproj"
            | "props"
            | "targets"
            | "csproj"
            | "ps1"
            | "psm1"
            | "bat"
            | "cmd"
            | "sh"
            | "sql"
            | "rb"
            | "php"
            | "swift"
            | "vb"
            | "lua"
            | "r"
            | "jl"
    ) || matches!(
        name.as_str(),
        "makefile"
            | "gnumakefile"
            | "dockerfile"
            | "cmakelists.txt"
            | "cargo.lock"
            | ".gitignore"
            | ".gitattributes"
            | ".editorconfig"
            | ".dockerignore"
    )
}

/// PY 保留原有已驗證的副本編輯；這次新加入的格式都只有讀取能力。
pub(super) fn read_only(path: &Path) -> bool {
    supported(path)
        && !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_formats_do_not_expand_copy_write_permissions() {
        for name in [
            "main.cpp",
            "API.H",
            "app.py",
            "app.js",
            "page.html",
            "style.css",
            "Cargo.toml",
            "Cargo.lock",
            "CMakeLists.txt",
            "Makefile",
            "Dockerfile",
        ] {
            assert!(supported(Path::new(name)), "{name}");
            if !name.ends_with(".py") && name != "CMakeLists.txt" {
                assert!(extension(Path::new(name)).is_err(), "{name}");
            }
        }
        for name in [
            ".git/config",
            "node_modules/index.js",
            "target/main.rs",
            ".env.json",
            ".ssh/config",
            "credentials.json",
            "secret.pem",
            "object.obj",
            "image.png",
        ] {
            assert!(!supported(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn bounded_source_decoding_preserves_content_and_rejects_binary() {
        let source = "// 中文程式\r\n".repeat(30_000);
        assert!(text::decode(source.as_bytes()).is_err());
        assert_eq!(
            text::decode_with_limit(source.as_bytes(), MAX_BYTES)
                .unwrap()
                .0,
            source
        );
        assert!(text::decode_with_limit(b"text\0binary", MAX_BYTES).is_err());
        assert!(text::decode_with_limit(&vec![b'a'; MAX_BYTES + 1], MAX_BYTES).is_err());
    }
}
