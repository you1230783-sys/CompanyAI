# Python原始碼分段編輯（試用）

來源.py唯讀，1–200 KB。先建立create_working_copy，再修改相關小段；不要等讀完整份檔案才開始。copies列出現有copy_id、revision及draft_path，續接必須沿用。

find_text會回傳行號。read_code_section(path=copy_id,first_line,last_line)一次最多200行／6000字，回傳完整檔案revision與該段section_hash。edit_code_section(copy_id,revision,first_line,last_line,section_hash,replacement)不必重送整段舊文；沿用精確行號與雜湊。區段以1起算、包含尾行；檔尾插入使用total_lines+1。CRLF檔會保留換行；每次修改後舊行號可能位移，下一次只讀相關區段。

一次修改imports或一個函式，立即提交。修改成功時桌面已把副本寫到同一份_AI_Output草稿，不建立_1、_2連續版本；即使程式暫時語法不完整也保存。原件不變，草稿不能宣稱已完成。外部修改草稿或版本不符時停止覆寫，不能強行重試。

完成所有必要小段後check_python(path=copy_id,revision)。語法錯誤按行號修正附近區段；不重讀全文。syntax_valid=true後save_copy將同一路徑標為完成成果，不另建立檔案。舊版檢查在任何修改後失效。編碼宣告必須符合實際bytes，保留縮排、UTF-8/BOM/Big5與coding宣告。

內建Python 3.13只AST解析與compile，不exec／import原文，也不產生__pycache__。缺少套件仍可能通過語法，不可稱功能測試已通過。交付明示語法結果與未執行的功能測試。

2–4個互不依賴的閱讀可用run_batch，最多2個子程序同時工作；不並行修改同一副本。各項回傳結果個別核對，只補失敗項。技能不會在呼叫read_file後自行撤回，只有明確load_skill切換階段才收回。
