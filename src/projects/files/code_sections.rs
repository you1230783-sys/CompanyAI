//! 版本綁定的小段編輯，不要求模型在修改時重送整段舊程式碼。
use super::*;

/// 行號從1開始、尾行包含在內；空檔只有第1個可插入位置。
fn section(content: &str, first: usize, last: usize) -> AppResult<(usize, usize)> {
    let lines: Vec<_> = content.split_inclusive('\n').collect();
    if first == 0
        || last < first
        || last - first >= 200
        || first > lines.len() + 1
        || last > lines.len().max(first)
    {
        return Err("請指定有效的1起算行號，每段最多200行；在檔尾插入使用總行數+1。".into());
    }
    let start = lines.iter().take(first - 1).map(|s| s.len()).sum();
    let end = lines.iter().take(last).map(|s| s.len()).sum();
    Ok((start, end))
}

impl Broker {
    pub(super) fn read_code_section(
        &mut self,
        path: &str,
        first: usize,
        last: usize,
        cancel: &AtomicBool,
        worker: &mut Worker,
    ) -> AppResult<Value> {
        let name = self
            .copies
            .get(path)
            .map(|c| c.name.as_str())
            .unwrap_or(path);
        if extension(Path::new(name))? != "py" {
            return Err("程式區段工具目前只支援PY。".into());
        }
        let content = self.content(path, cancel, worker)?;
        let (start, end) = section(&content, first, last)?;
        let value = &content[start..end];
        if value.chars().count() > 6000 {
            return Err("此區段超過6000字，請減少行數，只讀即將修改的區段。".into());
        }
        Ok(
            json!({"path":path,"revision":text::revision(&content),"first_line":first,"last_line":last,
            "total_lines":content.split_inclusive('\n').count(),"section_hash":text::revision(value),"text":value,
            "offset":content[..start].chars().count(),"next_offset":content[..end].chars().count(),"total":content.chars().count(),
            "guidance":"已足夠核對這段修改。edit_code_section帶section_hash即可，不重送expected全文；修改成功後行號與revision須沿用新結果。"}),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn edit_code_section(
        &mut self,
        id: &str,
        revision: &str,
        first: usize,
        last: usize,
        hash: &str,
        replacement: &str,
    ) -> AppResult<Value> {
        let copy = self.copies.get(id).ok_or("不是本次工作的副本。")?;
        if extension(Path::new(&copy.name))? != "py" || text::revision(&copy.text) != revision {
            return Err("PY副本版本不符；只需重新讀取要修改的小段，不重讀全文。".into());
        }
        if replacement.chars().count() > 6000 {
            return Err(
                "單段新程式碼最多6000字；先完成一個函式或一組imports，再修改下一段。".into(),
            );
        }
        let (start, end) = section(&copy.text, first, last)?;
        if text::revision(&copy.text[start..end]) != hash {
            return Err(
                "區段雜湊不符，未修改；請用read_code_section取得正確行號與section_hash。".into(),
            );
        }
        let replacement = if copy.text.contains("\r\n") {
            replacement.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            replacement.into()
        };
        let next = format!(
            "{}{}{}",
            &copy.text[..start],
            replacement,
            &copy.text[end..]
        );
        text::validate(&next)?;
        self.save_python_draft(id, &next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sections_preserve_crlf_unicode_and_eof() {
        let text = "甲\r\ndef f():\r\n    pass\r\n";
        let (a, b) = section(text, 2, 3).unwrap();
        assert_eq!(&text[a..b], "def f():\r\n    pass\r\n");
        assert_eq!(section(text, 4, 4).unwrap(), (text.len(), text.len()));
        assert_eq!(section("", 1, 1).unwrap(), (0, 0));
        assert!(section(text, 0, 1).is_err());
        assert!(section(text, 1, 201).is_err());
    }
}
