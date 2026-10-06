# 0.8.37：Outlook 資料夾選擇、前文比對與建議閱讀

## 使用者範圍

本版使用者要求實作後進版、編譯 EXE 並部署 Git main，供公司實測。沿用 EXE 發行，不重建 NSIS／離線 ZIP；PowerShell、圖示與其他專案不變。包含本機已完成、尚未提交的 0.8.36 CSV／上下文整理變更。

## 資料夾選擇

- 專案首次使用 Outlook 工具，在背景執行緒列出已開啟 Classic Outlook 的資料夾名稱供本機確認；不讀郵件內文、不向模型回傳完整清單。最多 100 個資料檔、5000 個資料夾、64 層、60 秒；無法完整列出則停止。
- 第一次預設全選；之後沿用上次選擇，尚未確認的新資料夾不自動授權。取消父層同時取消所有子層；選取子層會選取祖先。確認只開放範圍，不立即上傳全部郵件。
- `outlook-folders.dpapi` 綁定目前 Windows 使用者保存，跨專案及 Outlook 助理共用。模型與 WebView 不接收 StoreID／EntryID；模型看不到未勾選資料夾名稱、路徑或其郵件。可在下一次專案 Outlook 授權畫面修改。
- 在列舉、讀取、快取正文回傳、單封傳送、批次傳送及 MSG 匯出前核對權限；以目前祖先鏈拒絕移到未授權分支的資料夾。規則損壞不退回全開。
- 暫停续接重新確認；範圍改變後拒絕沿用舊 Outlook 快照，需新對話。新規則不追溯刪除已送出的對話、筆記、使用者提供的檔案或已匯出成果；允許郵件中既有引用也可能含其他郵件文字，資料夾排除不是內文去識別化。
- 專案仍僅支援本地 PST 收信／規則資料夾與 Exchange／OST 寄件備份，勾選不會擴大既有工具能力；不寄信、不更改信件、不讀附件。

## 本機比對與 AI 閱讀

- 新增 `outlook_compare(mail_ids, offset)` 與 `outlook-coverage` 技能；按需啟用，透過既有 desktop-agent-v1 通用契約。工具總數 44，不新增伺服器路由／資料庫。
- 只接受本次已列出標題的郵件代號，同次任務最多比對 1000 個不同郵件版本。一次比較最多 64 MiB 原始文字、120 秒，單封最多 256000 UTF-8 bytes；COM 呼叫間檢查取消／時間，不能強行中斷正在執行的 Outlook COM 呼叫。
- 內文在本機記憶體比對，原文不放入比較結果、progress_note、操作簿或 checkpoint。比較只保留 mail_id、字元數、涵蓋代號、相鄰前文是否匹配及建議閱讀狀態；每頁最多 30 筆。
- 優先同 Store 的 ConversationID 分組；缺少時以去回覆前綴的主旨及參與者作保守候選。同主旨／字數增加／地址去重不可用，都不是前文完整或逐封閱讀的依據。
- 從最新往前，只在完整正規化文字含於已選中較新郵件，且兩端為文字邊界時，才可略過。只整理換行、空白與行首引用符號，不刪數字、否定詞、簽名或內文。格式差異可能多推薦舊信，不以模糊相似度靜默遺漏內容。
- 支援多次前文中斷、不同回覆分支及後來重新引用。`previous_text_present=false` 只表示未匹配，不能断言人為刪改。失敗、超量或未掃描者保留推薦狀態、`complete=false`；不宣稱取得整段郵件歷史。
- 本機比較與 AI 閱讀獨立計數；AI 最多讀取 50 封不同內文。分頁／重讀同一版本不加封數；同組 mail_ids 的比較續頁重用摘要，內文閱讀仍核對來源。工具回傳剩餘額度。
- 所有 Outlook 專案技能優先最新信，再按缺口及相關性補讀。週報一開始載入方法、確認舊週報結構；逐主題留下短結論／來源／待辦，資料足夠即製作，不讀滿額度、不反覆查相同操作。

## 驗證狀態

2026-10-06 已完成 `scripts/Build.ps1 -EmptyCargoCache -TestOffice`，退出碼 0。使用 Rust 1.98.1、MSVC 14.29.30133／實際編譯器 19.29.30159（`_MSC_FULL_VER=192930159`）、x64、Windows SDK 10.0.19041.0；以空 Cargo 快取、既有 vendor／Cargo.lock／`.cargo/config.toml` 及 `--frozen` 建置。

- 格式、Clippy、234 項 Rust 測試、release 編譯全部通過。
- WebView2 DOM 自檢、10 項原生控制器案例、AppContainer 檔案／網路隔離、文件成果保存、HTTP 工具流程及 68 輪中斷續接通過。第一次沙箱內 WebView2 初始化失敗；在一般 Windows 環境重跑完整指令後通過，未略過檢查。
- 30 個約 10 MiB LOG 的搜尋、分頁、CSV 來源追蹤通過；真實 XLS／XLSX 的 CSV 精度、型別、錯誤／空白／合併格及一萬筆圖表資料通過。
- Office 16.0.20430.20092：DOC／DOCX／DOCM／XLS／XLSX／XLSM／XLSB／PPT／PPTX／PPTM 讀寫、格式／公式保留、新建文件、PNG 嵌入及移除來源圖片後重新開啟通過。
- `dist/LM_AI.exe` 檔案版本 0.8.37.0、13,630,976 bytes。EXE 更新清單已簽署，主程式內建公鑰驗證通過；清單、EXE 與驗證紀錄的版本／大小／SHA256 一致。未重建 NSIS／ZIP。

EXE SHA256：`4091a43d755f2367c872f7813abe31e139a4f0f1025191965e4c084287a55495`。

機器紀錄：`offline/exe-verification.json`、`offline/environment.txt`、`offline/composer-verification.json`、`offline/log-verification.json`、`offline/office-verification.json`。

已新增的回歸覆蓋：資料夾祖先／偽造代號／跨對話確認、DPAPI 保存及損壞拒絕、名稱不當 HTML 執行、前文中斷與分支、字數增加但語意改變、1000／50 獨立上限、比較原文不出現在工具結果／Saved、重複比對快取、取消與失敗不冒充完整。

真實公司 Outlook 大信箱的速度、引用格式相容性、GLM-5.3-Flash 的選材與 Word 週報品質需使用者實测。記憶體 COM fixture 不等於真實公司 Outlook；DOM／原生控制器自檢不等於外觀截圖驗收。
