# 0.8.18 成果與 Office 契約

## 成果

`_AI_Output/YYYYMMDD_HHMMSS/易讀檔名.ext`，以本機第一次儲存時間建立目錄。同秒任務目錄與同名檔案均使用 `_2`、`_3`；原子 create_dir/create_new 防覆寫，重送相同副本版本回傳既有路徑。

專案對話中成果 code/Markdown 連結變為原生定位動作。前端送 `project.command={action:"reveal",conversation,path}`。原生從 conversation 查授權專案，限制 `_AI_Output` 相對路徑，拒絕越界、Junction、符號連結、硬連結、目錄及不存在檔案，經 SHOpenFolderAndSelectItems 選取文件。沒有 ShellExecute 或任意 explorer 命令字串。一般對話及已移除專案不再具有這個授權。

## Office 固定工具

一般聊天／Outlook／標題的路由與 skills 值均不變。專案仍使用 skills=false，由桌面提供技能與工具結果。

原工具 read_file/create_working_copy/save_copy 延伸支援 DOCX、XLSX、PPTX。read_file 的 text 回傳序列化的 `{scope,blocks:[{id,label,text,kind}]}`，仍依 offset 分頁。copy_id 工作副本也用相同格式閱讀。新 `edit_office(copy_id,revision,block_id,expected,replacement)` 以完整區塊原文和版本比對；edit_text 拒絕 Office 封裝。實際磁碟儲存前重新確認來源雜湊與文字結構。

| 格式 | 支援 | 保留／省略 |
|---|---|---|
| DOCX | 主文段落，含表格內段落 | 不編輯段落末端結構；不含頁首頁尾、文字方塊、註解。含欄位或行內物件的段落唯讀 |
| XLSX | UsedRange 中字面文字及有限數值 | 公式顯示公式文字並維持原公式；合併格、布林、錯誤值唯讀；不含圖表、註解、資料連線 |
| PPTX | 投影片一般文字框、標題 | 不含群組、SmartArt、表格、圖表、備忘稿、圖片辨識 |

PowerPoint 最多 200 張投影片／2000 個圖形；每檔 50 MB、2000 區塊、200 KB 序列化文字。不能從空白建立 Office、不能修改版面或任意執行巨集。XLSX 文字以字面值寫入，`=1+1` 不會變成公式；數字按數字保存。新增換行不適用 Word，以免改變既有段落身分。混合字型／文字框換行仍需使用者核對，不承諾完全等同人工排版。

## 本機及加密界線

需要已安裝且可用的桌面 Office。每次 COM 操作建立自己的執行個體，原件 ReadOnly 開啟、不加 MRU；關閉文件且沒有其他文件時退出。PowerPoint 單一實例已使用中便拒絕。AutomationSecurity=ForceDisable；Excel 關閉事件與連結更新，Word 暫停開啟時更新連結。暫改的設定在成功及錯誤結束時還原。Office 信任中心、公司加密、保護檢視／授權限制維持有效，沒有網路沙箱或企業 DLP 合規保證。

AppContainer 僅處理既有文字工具。Office COM 在受信任 broker 背景執行緒執行，不是受限 AppContainer，不能宣稱整個 Office 已隔離。取消在當前 COM 呼叫返回及下一區塊檢查時生效；若 Office 有等待視窗，使用者可能需要處理該視窗，沒有強殺使用者 Office 的 fallback。

儲存時 Office 先在本次輸出目錄下 `.office_<id>` 另存。一般祖先目錄仍使用原有嚴格 pin；Office 暫存使用不允許改名的目錄 handle 與不可移除 anchor，使目錄非空而不能設成 reparse point。固定 broker 從驗證 handle 複製到 create_new 保留的成果 handle，關閉後以 Office 重新讀取、比對所有涵蓋的區塊；finish 再驗證。此複製是否符合公司加密系統仍需實測，失敗不會以純文字替代 Office 檔案。

正常結束清除程式建立的暫存檔與空目錄，不遞迴刪除、不覆寫原件。崩潰或 Office 拒絕釋放檔案時可能留下 `.office_` 暫存及未驗證的空白／部分成果，回覆列出可能的成果，交由使用者檢查。已發布成果不允許 AI 刪除。

參考：[Word Open](https://learn.microsoft.com/en-us/office/vba/api/word.documents.open)、[Excel Open](https://learn.microsoft.com/en-us/office/vba/api/excel.workbooks.open)、[PowerPoint Open](https://learn.microsoft.com/en-us/office/vba/api/powerpoint.presentations.open)、[Windows reparse 非空目錄限制](https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/fsctl-set-reparse-point)。
