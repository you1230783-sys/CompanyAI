# 0.8.34 LOG、Outlook 與長任務

## 使用方式

在專案對話說：「找 2026-06-23、Z01-CY 的 connection LOG，分析 12:25～12:33 的連線中斷，附行號及前後文。」AI 可按需載入 log-analysis，先按檔名篩選，再逐頁查詢。

檔名支援 `YYYYMMDD_分類_站別.log`，例如 `20260622_connection_Z01-CY.log`；分類及站別完整比對、不分大小寫。內文支援 `2026/06/23, 15:25:48.084`、斜線／橫線日期、ISO T 分隔及純時間。內文日期優先，純時間以最近明確日期或檔名日期補足；不自行猜測翌日。HH:MM 結束時間包含整分鐘；跨午夜用時間 OR 條件，另指定日期時只限該日期。

執行途中可按「補充指示」，例如「只看 Z01-CY，12:28 有人手動重啟」。顯示等待接收／已接收／已帶入請求，待接收時可修改或撤回。指示持續到本次任務結束，原始要求與相容條件保留。已送出的模型請求不重送；收到新指示後，尚未執行的舊候選操作回報未執行並重新規劃。已開始的操作先完成。結束界線後拒絕新提交，輸入框保留文字。

## 界線與完整性

- 唯讀 LOG、OUT、ERR、JSONL、TXT，每檔最多 32 MiB，一次查詢最多 30 檔。程式原始檔與 shell 執行未納入本版。
- list_logs 每頁 100 檔，同資料夾最多檢查 20000 項。read_log 一基行號、零基字元位置，每頁最多 200 行／12000 字，長行可接續；單行上限 128 KiB，超過明確失敗。
- search_logs 最多 12 個任一字面關鍵字，最多 5 行前後文，每頁有界掃描／命中／摘錄。沒有命中但仍有 next_cursor 時必須續頁。摘錄超長標示 excerpt_truncated，可回查 read_log 原文。
- 無時間續行沿用上一筆時間並標示 time_inherited；無法辨識的時間、缺日期、讀取失敗列入不完整狀態。before/after 可能超出要求時間範圍。JSONL 目前只作文字搜尋，不解讀內嵌時間欄位。
- 每頁核對所有來源的 SHA256；來源變動拒絕舊游標。保留原件唯讀鎖，不接管正在寫入的紀錄；被占用時請使用已輪替或另存的 LOG。此版不支援即時 tail 監看。
- 補充只由原生 UI 命令進入，逐次核對登入／版本／對話／run。原文 DPAPI 保存，暫停及上下文縮減仍保留。每次任務最多 30 則／48 KB，每則最多 4000 字。
- 操作開始的界線之後才送達的指示，於下一輪處理；停止仍用原有停止按鈕。訊息帶入請求不代表模型必然遵守，最終品質需公司模型實測。

## Outlook 與互動

- Outlook 第一次讀取工具前由原生 UI 同意；拒絕不建立 COM 連線。授權不寫入 checkpoint，重新續接再次詢問。
- local_inbox 為已載入 PST 的分層郵件資料夾；online_sent 為 Exchange／OST 信箱寄件備份。每次只列一層，先挑資料夾再取標題，內文按 mail_id 分頁讀取。選定範圍內依寄出時間、寄件地址及 To／CC／BCC 集合去重；地址不完整保守保留。
- 標題日期範圍最多 93 天、每次來源有界掃描 10000 項／最多取 2000 標題，單次 30 秒；結果有頁面及不完整提示。每任務最多保存 4000 標題／4 MiB 標題快照、5000 資料夾、30 封 256 KB 內文。
- 不讀附件或修改／寄信，不掃磁碟 PST。線上信箱經 Outlook 可為離線快取；實際同步與公司郵箱仍待測試。
- Office／檔案占用先提示關閉，當輪重試或保存暫停；保留原模型請求，不強制關閉使用者程式。
- 圖表異常文字預設 skip，原空白維持 gap；「查看異常值」每頁 100 筆，顯示統計、X/Y、原始值、來源列／格及處理方式。只有原生預檢可附來源紀錄，模型不得自填假證據。

Outlook 實作參考 Microsoft 原始文件：[資料存放區](https://learn.microsoft.com/en-us/office/vba/api/outlook.store.isdatafilestore)、[預設資料夾](https://learn.microsoft.com/en-us/office/vba/api/outlook.store.getdefaultfolder)、[收件地址](https://learn.microsoft.com/en-us/office/client-developer/outlook/pia/how-to-get-the-e-mail-address-of-a-recipient)、[日期篩選](https://learn.microsoft.com/en-us/office/vba/outlook/how-to/search-and-filter/filtering-items-using-a-date-time-comparison)。

## 長任務與資料持久化

- 正式原生任務每段最長 24 小時，包含排隊與 UI 等待。60 工具／80 回覆為自動換批界線，不重設期限或安全計數；手動繼續才開始新一段。舊文字協定 checkpoint 保持手動次數續接相容性。
- 65% 工具文字預算或 30 對工具歷史時整理，先分檔 DPAPI 封存原文，再保留最近兩對與工作筆記／實際狀態。提前提示模型在 progress_note 整理目標／決策／已完成／目前步驟／待辦／失敗；模型未回筆記時僅使用可核對的程式狀態，不編造語意摘要。
- 原始用戶要求與補充不裁切。下輪對話附最近三輪要求及最終答案、工具次數；過大舊答案節錄並指向 read_task_result。read_work_log 預設索引，指定 operation_id 分頁原文；read_task_result 的 operations 可按頁查過去任務細節。
- checkpoint 在 HTTP 提交前記住原請求 ID，未知只 GET 查回；工具成功後保存副本及實際版本。唯讀中斷可以重試；修改開始前使舊 checkpoint 失效，未知寫入不能直接重播。取消／正常完成關閉續接入口。
- 完整操作結果分檔保存、校驗 SHA256；缺檔或篡改明確停止。診斷只帶最近 8 份請求／80 次操作，完整工具原文由專用工具查回。
- 時限不保證電腦休眠、關機、登入到期或服務端異常期間持續運作；恢復需使用者按繼續。保留有限錯誤及無進展停止，不保證模型語意永不遺漏。

## 驗證狀態

2026-10-05 執行 `scripts/Build.ps1 -EmptyCargoCache -TestOffice` 完整通過，退出碼 0：

- MSVC v142 14.29.30133，實際 `_MSC_FULL_VER=192930159`、x64；Rust 1.98.1／SDK 10.0.19041.0。空 Cargo 快取，既有 Cargo.lock、vendor、.cargo/config.toml，`cargo build --release --frozen` 成功。
- 格式、Clippy 禁止警告、216 項單元測試全部通過。包含 Outlook 假資料／去重／分頁／拒絕、補充指示、上下文精簡、原始資料查回、操作校驗與超過 8 MB 的加密分塊。
- WebView2 DOM 自檢：Outlook 同意／拒絕、占用提示／重試／保存、補充編輯／撤回、圖表預設 skip、異常值 101 筆分頁及 HTML 文字安全通過。沒有把 DOM 自檢當成公司視覺驗收。
- 30 份 LOG，每份 10,526,720 bytes，時間篩選分 9 頁取得 60 筆預期命中；長行續讀、站別、日期與來源變更拒絕通過，本輪耗時約 113 秒。
- 21 種原生 HTTP 代理情境通過。正常長任務與 checkpoint 中斷恢復各完成 65 份證據、68 次 POST；跨過 60 次工具界線並成功查回已封存原文，中斷恢復沒有重做前 60 次。完成後不可再續接。
- 真實檔案鎖：關閉後當輪讀取、先保存再續接讀取、關閉後建立副本均通過；前兩者各僅 2 次 POST，原模型請求不重送。
- 舊文字協定、60 工具加密暫停／未存副本恢復／保存去重、錯誤身分不清除既有 checkpoint、逾時／遲到完成／未知請求、摘要委派、PDF 與 AppContainer 隔離回歸通過。
- 實際 Word／Excel／PowerPoint 十種新舊格式讀取、修改副本、儲存與重開；新建文件、結構及樣式通過。XLS／XLSX 異常資料、等待後續接、來源變更、一萬筆圖表、非連續欄位與 100 列分頁通過。
- Word／Excel／PPT 的 PNG 嵌入、版本、去重、來源變更拒絕、儲存及重開通過；移除測試用原 PNG 後重開仍有內嵌圖片。
- EXE 0.8.34.0，12,926,464 bytes；SHA256 `31be4a7642010e3336cb58ee9d0d54d3f8ec9b508979887e612981d525a8a463`。dist 與 target 成品一致，簽署清單經主程式內建公鑰實際驗證成功。

機器紀錄：[EXE](../offline/exe-verification.json)、[Office](../offline/office-verification.json)、[LOG](../offline/log-verification.json)。

24 小時期限採可控短時鐘及常數檢查，跨批次以 65 份不同證據、68 次真實 HTTP 請求驗證；模擬在已保存 checkpoint 的邊界中斷後還原，不宣稱已連續執行 24 小時。Outlook 測試使用假資料提供者及拒絕同意的實際 runner，未讀使用者真實信箱。

本機固定資料不能取代公司加密、實際機台資料、Outlook 郵箱與模型品質驗收。沿用 EXE／簽署清單交付，不製作 NSIS／離線 ZIP。
