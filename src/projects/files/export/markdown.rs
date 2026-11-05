//! 有界的常用Markdown行內格式；無法配對的標記原字保留，不解析HTML或載入網址。
use crate::projects::office::TextRun;

pub(super) fn inline(text: &str) -> Vec<TextRun> {
    let mut runs = Vec::new();
    parse(text, false, false, 0, &mut runs);
    runs
}
fn push(runs: &mut Vec<TextRun>, text: &str, bold: bool, italic: bool, code: bool) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = runs
        .last_mut()
        .filter(|last| last.bold == bold && last.italic == italic && last.code == code)
    {
        last.text.push_str(text);
    } else {
        runs.push(TextRun {
            text: text.into(),
            bold,
            italic,
            code,
        });
    }
}
fn closing(text: &str, marker: &str) -> Option<usize> {
    let mut at = 0;
    while at < text.len() {
        let tail = &text[at..];
        let ch = tail.chars().next()?;
        if ch == '\\' {
            at += 1;
            if let Some(next) = text[at..].chars().next() {
                at += next.len_utf8();
            }
            continue;
        }
        if tail.starts_with(marker) && at > 0 && !text[..at].ends_with(char::is_whitespace) {
            return Some(at);
        }
        at += ch.len_utf8();
    }
    None
}
fn parse(text: &str, bold: bool, italic: bool, depth: usize, runs: &mut Vec<TextRun>) {
    if depth >= 8 {
        push(runs, text, bold, italic, false);
        return;
    }
    let mut at = 0;
    while at < text.len() {
        let tail = &text[at..];
        let ch = tail.chars().next().unwrap_or(' ');
        if ch == '\\' {
            if let Some(next) = tail[1..]
                .chars()
                .next()
                .filter(|ch| ch.is_ascii_punctuation())
            {
                push(runs, &tail[1..1 + next.len_utf8()], bold, italic, false);
                at += 1 + next.len_utf8();
                continue;
            }
        }
        if ch == '`' {
            let length = tail.bytes().take_while(|ch| *ch == b'`').count();
            let marker = &tail[..length];
            if let Some(end) = tail[length..].find(marker) {
                push(runs, &tail[length..length + end], bold, italic, true);
                at += length * 2 + end;
                continue;
            }
        }
        let mut matched = false;
        for marker in ["***", "___", "**", "__", "*", "_"] {
            if !tail.starts_with(marker) {
                continue;
            }
            if marker.starts_with('_') && text[..at].ends_with(char::is_alphanumeric) {
                continue;
            }
            let rest = &tail[marker.len()..];
            if rest.starts_with(char::is_whitespace) {
                continue;
            }
            if let Some(end) = closing(rest, marker) {
                parse(
                    &rest[..end],
                    bold || marker.len() >= 2,
                    italic || marker.len() != 2,
                    depth + 1,
                    runs,
                );
                at += marker.len() * 2 + end;
                matched = true;
                break;
            }
        }
        if !matched {
            push(runs, &tail[..ch.len_utf8()], bold, italic, false);
            at += ch.len_utf8();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_formats_preserve_unicode_code_and_unmatched_markers() {
        let runs = inline("中文 **粗體😀**、*斜體*、***兩者***、`a_*b`、\\*原文\\*、a_b與**未完");
        let text: String = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(text, "中文 粗體😀、斜體、兩者、a_*b、*原文*、a_b與**未完");
        assert!(runs
            .iter()
            .any(|run| run.text == "粗體😀" && run.bold && !run.italic));
        assert!(runs
            .iter()
            .any(|run| run.text == "兩者" && run.bold && run.italic));
        assert!(runs.iter().any(|run| run.text == "a_*b" && run.code));
    }
}
