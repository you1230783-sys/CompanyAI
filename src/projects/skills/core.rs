//! 常駐方法只包含會影響當前操作的規則；完整教學由 read_skill_guide 按頁查回。
pub(super) fn guide(id: &str) -> &'static str {
    match id {
        "python-edit" => "Python修改：create_working_copy後先plan_code_change保存user/preserve/added需求。find_text定位、read_code_section讀小段、edit_code_section優先只傳copy_id/section_id/replacement，避免行號與hash配错；每段立即保存同一草稿，續接沿用copy_id，不重讀全文。check_python只檢查語法。完成後review_code_change(checks=[])看真實差異與需求，讀相關函式及呼叫處；test_python用unittest.TestCase和load_target()測真正副本，涵蓋預設/正常/邊界/錯誤與輸出，mock要列出。active_code每輪附目前區段原文、原生狀態與測試格式。資訊足夠直接提交一組最小需求測試，成功後再補其他必要案例，不為寫全部測試重讀全文／指南。同版語法結果可重用。最後review_code_change逐項提交tested/reviewed/unverified/incomplete與行號證據，才能save_copy；缺依賴或外部環境明示未驗證，不無限重試。測試失敗只修相關段，任何修改使舊驗證失效。交付不可只憑笔記或語法宣稱功能完成。",
        "chart-edit" => "圖表編輯：inspect_chart(null,null)列本輪及同對話歷史圖；指定message_index/chart_index查目前樣式，不重讀來源。edit_chart傳完整style，只改需要欄位，保留其他設定。歷史圖會另建本輪版本，原圖保留；後續用回傳chart_index、message_index=null。layout位置為圖框左上角0–1比例；annotations支援文字/字型/粗斜底線/字色底色。詳細限制見read_skill_guide(chart-edit)。PNG請重新export_chart_png。",
        "python-analysis" => include_str!("python-analysis.md"),
        "excel-read" => "Excel：先 inspect_excel 核對真實欄名與樣本，再 plan_excel_analysis 規劃 x/y/time；用原始欄字母，不重新編號。export_planned_excel 沿用計畫匯出本機CSV。時間序號不能當量測值；欄位意義有多種合理解釋才詢問。一般閱讀 read_excel_range；需要統計切 python-analysis，需要圖表切 dataset-charts。",
        "dataset-charts" => include_str!("dataset-charts.md"),
        "charts" => include_str!("charts.md"),
        "log-analysis" => include_str!("log-analysis.md"),
        "outlook-research" => include_str!("outlook-research.md"),
        "outlook-coverage" => include_str!("outlook-coverage.md"),
        "cad-new" => include_str!("cad-new.md"),
        "cad-active" => include_str!("cad-active.md"),
        "cad-copy" => include_str!("cad-copy.md"),
        "cad-excel" => include_str!("cad-excel.md"),
        "cad-drawing" => include_str!("cad-drawing.md"),
        "office-edit" => "Office：統計數字先register_report_values引用成功Python的/summary/...；建表並取得block_id後apply_report_values填入，圖表共用export_report_values的CSV。不轉抄或猜額外統計；綁定欄位及來源版本在交付時核對。create_working_copy 建立或複製；read_file 核對結構及 revision，再 office_batch/office_action 修改，最後save_copy。只有真實copy_id可用；結構改變重新核對區塊ID。原檔/公式/巨集/外部連結不改。PNG只能用已匯出的相對路徑插入，保持比例；詳細操作用read_skill_guide(office-edit)。",
        "weekly-update" => "週報：先讀舊表頭/格式及本週範圍，分清新增、主導、延續與未確認。標題不能當成果、舊計畫不能當完成。按階段切outlook-research讀信、excel-read核對欄位、office-edit改副本。保留日期/來源與使用者篩選條件於工作筆記。",
        "research" | "paper-evidence" => "研究：search_files指定明確paths與query找線索，核對coverage/omitted/errors；命中預覽含版本/行號/區塊標籤，部分查無不可當全文沒有。list_document_sections定位，read_document_section核對原文。摘要只作索引，重要數字/單位/否定條件/引文讀原文。品質模型可按需summarize_document，但委派不代表主模型讀完。完整結果留本機；要存長期方法時切notes。",
        "notes" | "context-management" => "記憶：notes可save_workflow保存成功步驟與參數插槽，list_workflows／prepare_workflow檢視後execute_workflow_step逐步重跑；新資料與權限重新核對，失敗不重播。筆記是語意摘要，不取代來源、權限與執行證據。compact_context保存目標/決策/已完成/待辦/下一步，superseded標明失效結論。不要抄入原文。需要精確細節用read_work_log查原版本。record_analysis的證據取實際operation_id與JSON Pointer，統計checks取工具數值，不自填passed。交付依原生completion與核對結果說明完成/部分/失敗/未驗證，不以少數成功宣稱整批完成。分析方法保存適用條件/步驟/驗證/限制，下次先核對新來源再重用。",
        "multi-file-excel" => "多檔Excel：先統一欄位意義/單位與來源，缺資料明示，不猜空值。讀取切excel-read，合併統計切python-analysis，既有表格修改切office-edit。保存對應來源與未完成事項。",
        "mermaid" => include_str!("mermaid.md"),
        "text-edit" => "文字：不依副檔名限制，包含BAT/CMD/PS1與未知文字格式；先以實際內容／編碼核對。find_text定位原文，create_working_copy保留原格式與編碼，再read_file(copy_id)、edit_text、save_copy。原件唯讀，副本200KB，不執行來源；PY沿用python-edit。每次沿用最新revision；二進位、未知編碼或讀取失敗不猜內容。",
        _ => "",
    }
}
