# 程式碼唯讀分析



來源從texts[name]讀取，可用re做正規表示式、splitlines列行號、跨檔比較字串及計數。不能import或exec來源、不能編譯來源。不要用inspect_excel開CPP。一般find_text/search_files已支援程式碼；run_python適用較大來源、正規表示式或大量統計，明示涵蓋檔名／筆數及失敗範圍。

程式碼預設嚴格UTF-8，舊Big5來源明確指定encoding=big5；UTF-16暫不在Python文字快照支援範圍，不以忽略錯誤方式解碼。Python revision是原始bytes的sha256，與read_code_section的文字revision不同，不混用；可省略第一次revision，續用Python回傳版本。單檔32MiB及合計60MiB限制不因純程式碼批次增加而放寬。

未知副檔名也可嘗試一般文字閱讀或run_python(kind=text)，依實際編碼驗證；不要因副檔名未列在範例就拒絕。二進位／無法確認編碼須如實回報，專用文件／CAD／圖片走原工具。修改使用一般文字工作副本（200KB），原件唯讀、不自動執行來源。list_files按next_offset續頁，null代表結束。
