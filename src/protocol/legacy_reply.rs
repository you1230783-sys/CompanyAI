//! 保守辨識完整的五段編號回覆，用於結構化欄位缺漏時補齊。
//! 只接受 Answer 開頭且五個章節各出現一次；不從一般敘述、引用或程式碼找關鍵字。
use super::reply::{ReplyPayload, ReplySections};

/// 回傳章節位置及標題後的內容；標題可為 Markdown heading、粗體或編號文字。
fn heading(line: &str) -> Option<(usize, &str)> {
    if line.starts_with("    ") || line.starts_with('\t') {
        return None;
    }
    let mut text = line.trim_start();
    if text.starts_with('#') {
        let count = text.bytes().take_while(|byte| *byte == b'#').count();
        if count > 6 || !text[count..].starts_with(char::is_whitespace) {
            return None;
        }
        text = text[count..].trim_start();
    }
    text = text.strip_prefix("**").unwrap_or(text);
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    text = &text[digits..];
    text = text
        .strip_prefix('.')
        .or_else(|| text.strip_prefix(')'))
        .or_else(|| text.strip_prefix('、'))?
        .trim_start();
    let aliases: [&[&str]; 5] = [
        &["answer", "content", "回答", "內容"],
        &["key points", "keypoints", "keypoint", "回答重點"],
        &[
            "references",
            "sources",
            "source",
            "引用資料庫",
            "內容引用處",
            "來源摘要",
            "來源",
        ],
        &["confidence", "信心度"],
        &["limitations", "limitation", "回答限制", "限制"],
    ];
    for (section, names) in aliases.iter().enumerate() {
        for name in *names {
            if !text
                .get(..name.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
            {
                continue;
            }
            let rest = &text[name.len()..];
            if !rest.is_empty()
                && !rest.starts_with(char::is_whitespace)
                && !rest.starts_with([':', '：'])
                && !rest.starts_with("**")
            {
                continue;
            }
            let rest = rest.trim_start_matches([' ', '\t', ':', '：']);
            let rest = rest.strip_prefix("**").unwrap_or(rest);
            return Some((section, rest.trim_start_matches([' ', '\t', ':', '：'])));
        }
    }
    None
}

/// 展開被壓在同一行的 Markdown 章節；必須整行具有完整五段，且不含 code span。
/// 其他行保持原狀，避免把正文中的 #、程式碼範例或不完整串流當成章節切開。
fn expand_compact_line(line: &str) -> Vec<&str> {
    if line.contains('`') || heading(line).map(|entry| entry.0) != Some(0) {
        return vec![line];
    }
    let mut positions = vec![0];
    for (index, _) in line.match_indices('#') {
        if index > 0
            && line[..index].ends_with(char::is_whitespace)
            && heading(&line[index..]).is_some()
        {
            positions.push(index);
        }
    }
    if positions.len() != 5 {
        return vec![line];
    }
    positions.push(line.len());
    positions
        .windows(2)
        .map(|pair| line[pair[0]..pair[1]].trim())
        .collect()
}

/// 把條列內容轉成文字項目；沒有條列時保留整段 Markdown，不猜測句子邊界。
fn items(text: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if !text.contains('\n') && text.starts_with("* ") {
        return text[2..].split(" * ").map(str::to_string).collect();
    }
    if text
        .lines()
        .all(|line| line.trim().is_empty() || line.starts_with("- ") || line.starts_with("* "))
    {
        return text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line[2..].to_string())
            .collect();
    }
    vec![text.to_string()]
}

/// 只有能確認完整格式時才回傳欄位；辨識失敗時呼叫端必須保留完整原文。
pub(super) fn parse(text: &str) -> Option<ReplyPayload> {
    let mut bodies: [String; 5] = std::array::from_fn(|_| String::new());
    let mut seen = [false; 5];
    let mut current: Option<usize> = None;
    let mut fence = None;
    for original in text.lines() {
        let trimmed = original.trim_start();
        let marker = if trimmed.starts_with("```") {
            Some('`')
        } else if trimmed.starts_with("~~~") {
            Some('~')
        } else {
            None
        };
        if fence.is_some() || marker.is_some() {
            let section = current?;
            bodies[section].push_str(original);
            bodies[section].push('\n');
            if fence == marker {
                fence = None;
            } else if fence.is_none() {
                fence = marker;
            }
            continue;
        }
        for line in expand_compact_line(original) {
            if let Some((section, body)) = heading(line) {
                if seen[section] || (current.is_none() && section != 0) {
                    return None;
                }
                current = Some(section);
                seen[section] = true;
                bodies[section].push_str(body);
                bodies[section].push('\n');
            } else if let Some(section) = current {
                bodies[section].push_str(line);
                bodies[section].push('\n');
            } else if !line.trim().is_empty() {
                return None;
            }
        }
    }
    if !seen.into_iter().all(|value| value) || bodies[0].trim().is_empty() || fence.is_some() {
        return None;
    }
    Some(ReplyPayload {
        answer: bodies[0].trim().to_string(),
        sections: ReplySections {
            key_points: items(&bodies[1]),
            sources: items(&bodies[2]),
            confidence: (!bodies[3].trim().is_empty()).then(|| bodies[3].trim().to_string()),
            limitations: items(&bodies[4]),
        },
        citations: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_and_multiline_replies_keep_every_section() {
        let compact = include_str!("../../ui/fixtures/inline-reply.txt");
        for text in [compact.to_string(), compact.replace(" ### ", "\n\n### ")] {
            let reply = parse(&text).unwrap();
            assert_eq!(reply.sections.key_points.len(), 2);
            assert_eq!(reply.sections.confidence.as_deref(), Some("100%"));
            assert!(reply.sections.limitations[0].contains("未涉及任何實際的文件分析"));
        }
        for text in [
            format!("```text\n{compact}\n```"),
            format!("範例說明：\n{compact}"),
            format!("> {compact}"),
        ] {
            assert!(parse(&text).is_none());
        }
        assert!(parse("### 1. Answer 正文\n### 2. Key points 尚未結束").is_none());
    }
}
