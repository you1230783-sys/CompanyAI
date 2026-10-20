# Office 建立與編輯

Office：新建時 create_working_copy(source:null,name:檔名.docx/xlsx/pptx)；既有 DOC/DOCX/DOCM、XLS/XLSX/XLSM/XLSB、PPT/PPTX/PPTM 副本保留格式。read_file.text 是 scope/structure/formats/blocks JSON（block.format 是共用 formats 索引），可分段讀取。edit_office 修訂既有文字；office_action 新增段落／表格／工作表／範圍內容／投影片或套用格式。Word 單段文字不含換行，用 word_paragraph 插入正文段落；word_table 建立簡單等寬表格。格式優先用樣式，未指定屬性保留。結構變動後重讀區塊 ID，使用新 revision。Excel 公式與合併格不覆寫，數字用 JSON 數字、文字為字面值；格式操作不代表能新增公式。PPT 使用 title/content/two_column 版面，文字仍需人工檢查溢出；已開啟 PowerPoint 時需先儲存關閉。不改巨集、外部連結、圖表、SmartArt、頁首頁尾。所有操作只修改工作版本，最後 save_copy 發布；不要把匯入純文字當成原 Office 格式。

圖片：僅 PNG。先 export_chart_png 取得 path（或選用專案既有 PNG），再 office_action(copy_id,revision,operation:{kind:"insert_image",path,target,width})，最後 save_copy。target 為 {kind:"word",before:null或段落ID}、{kind:"excel",sheet:1,cell:"H2"} 或 {kind:"ppt",slide:1,left:40,top:100}；width 為點（72 點＝1 英吋），保持比例，不可超出頁面。圖片嵌入，不建立外部連結；原 PNG 在發布前必須保持不變。圖片只是插入，不代表模型看過圖片內容。修改必須使用 create_working_copy 真正回傳的 copy_id 和最新 revision，不可填空字串。快速模型自行使用 read_file 等閱讀工具，不呼叫 summarize_document。

office_batch 每批 1–20 個操作，整批成功才提交工作版本；失败不重做已成功的其他批次。
