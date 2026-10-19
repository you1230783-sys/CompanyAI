//! Mermaid 原始碼是版本依據；SVG／PNG 只由固定離線呈現器產生。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;

pub const MAX_SOURCE: usize = 48_000;
pub const MAX_DIAGRAMS: usize = 12;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagram {
    pub name: String,
    pub source: String,
    pub revision: String,
    pub path: String,
    pub based_on: Option<String>,
}

impl Diagram {
    /// 加密歷史與 checkpoint 還原時也核對原始碼，不信任舊的版本標籤。
    pub fn validate(&self) -> AppResult<()> {
        validate_source(&self.source)?;
        validate_name(&self.name)?;
        if self.revision != super::text::revision(&self.source) {
            return Err("Mermaid 原始碼版本不一致。".into());
        }
        let path = super::files::relative(&self.path)?;
        if path.components().count() != 3
            || !path.starts_with("_AI_Output")
            || path.extension().and_then(|s| s.to_str()) != Some("mmd")
        {
            return Err("Mermaid 成果路徑不合法。".into());
        }
        Ok(())
    }
}

/// 限制來源大小並禁止來源覆寫呈現器設定。真正語法仍由 Mermaid 引擎驗證。
pub fn validate_source(source: &str) -> AppResult<()> {
    super::text::validate(source)?;
    if source.trim().is_empty() || source.len() > MAX_SOURCE {
        return Err("Mermaid 原始碼須為 1–48000 bytes；大型流程請拆成多張圖。".into());
    }
    if source.contains("%%{") || source.trim_start().starts_with("---") {
        return Err("請只提供 Mermaid 圖形語法，不含設定指令或 YAML frontmatter。".into());
    }
    Ok(())
}

pub fn validate_name(name: &str) -> AppResult<()> {
    let path = super::files::relative(name)?;
    if path.components().count() != 1
        || name.len() > 120
        || path.extension().and_then(|s| s.to_str()) != Some("mmd")
    {
        return Err("Mermaid 檔名須為單一 .mmd 檔名，最多 120 bytes。".into());
    }
    Ok(())
}

/// 格式不接受任意副檔名；SVG 只接受固定呈現器的結果，不接受模型提供 SVG。
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Mmd,
    Svg,
    Png,
}
impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mmd => "mmd",
            Self::Svg => "svg",
            Self::Png => "png",
        }
    }
}

pub type Renderer = Box<dyn FnMut(&str, Format, &AtomicBool) -> AppResult<Vec<u8>> + Send>;

#[derive(Clone, Serialize, Deserialize)]
pub struct Export {
    pub path: String,
    pub sha256: String,
    pub format: Format,
    /// 舊 checkpoint 沒有綁定資料時保留 None，不依檔名猜測對應圖形。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ExportSource>,
}

/// 只記錄原生工具實際匯出的目標；不授予任何額外檔案權限。
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportSource {
    pub revision: String,
    pub message_index: Option<usize>,
    pub diagram_index: usize,
}

/// WebView 的固定 API 回傳序列化字串。錯誤沿用原文，讓模型能修正語法。
pub fn decode_rendered(response: &str, format: Format) -> AppResult<Vec<u8>> {
    let value: serde_json::Value =
        serde_json::from_str(response).map_err(|_| "Mermaid 呈現結果無法解析。")?;
    if value["ok"] != true {
        return Err(format!(
            "Mermaid 呈現失敗：{}",
            value["error"]
                .as_str()
                .unwrap_or("未知錯誤")
                .chars()
                .take(2000)
                .collect::<String>()
        ));
    }
    let data = value["data"].as_str().ok_or("Mermaid 未回傳圖形。")?;
    match format {
        Format::Png => {
            let bytes = super::charts::png::decode_image_url(data)?;
            let (width, height) = super::charts::png::dimensions(&bytes)?;
            if !(16..=4096).contains(&width) || !(16..=4096).contains(&height) {
                return Err("Mermaid PNG 尺寸不合法。".into());
            }
            Ok(bytes)
        }
        Format::Svg
            if data.len() <= 8 * 1024 * 1024
                && data.starts_with("<svg")
                && data.ends_with("</svg>") =>
        {
            Ok(data.as_bytes().to_vec())
        }
        _ => Err("Mermaid 圖形格式或大小不合法。".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_and_filename_boundaries() {
        assert!(validate_source("flowchart TD\n A[開始] --> B[結束]").is_ok());
        for source in [
            "",
            "%%{init: {}}%%\nflowchart TD",
            "---\nconfig: {}",
            "flowchart TD\0",
        ] {
            assert!(validate_source(source).is_err());
        }
        assert!(validate_source(&"x".repeat(MAX_SOURCE + 1)).is_err());
        assert!(validate_name("流程.mmd").is_ok());
        for name in ["../流程.mmd", "C:/x.mmd", "x.svg", "CON.mmd", "x/y.mmd"] {
            assert!(validate_name(name).is_err());
        }
    }
    #[test]
    fn render_errors_are_not_success() {
        assert!(
            decode_rendered(r#"{"ok":false,"error":"line 3"}"#, Format::Svg)
                .unwrap_err()
                .contains("line 3")
        );
        assert!(decode_rendered(r#"{"ok":true,"data":"<script/>"}"#, Format::Svg).is_err());
        assert!(decode_rendered(r#"{"ok":true,"data":"<svg></svg>"}"#, Format::Svg).is_ok());
    }
}
