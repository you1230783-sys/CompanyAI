# 0.8.53 驗證紀錄

2026-10-14，使用者要求先提供進版EXE／NSIS作公司實測，完整驗證留待確認後補做。本版修改一般聊天413拒絕後繼續GET而衍生404的流程、連續三次相同查詢錯誤的停止條件，以及分析摺疊和圖中文字編輯介面。

## 完整驗證結果（最新）

使用者回報「測試 OK」後，於 **2026-10-14 16:05** 完成 `scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller`，結束碼 0，結果 **PASS**。完整流程無須修正功能程式碼，EXE 與 NSIS 的 SHA256 均與先前測試包相同，維持 0.8.53。

- **工具鏈與測試：** MSVC v142 `14.29.30133` x64，實際 C 探針 `_MSC_FULL_VER=192930159`；Rust `1.98.1`、SDK `10.0.19041.0`。空 Cargo 快取、專案 vendor／Cargo.lock／.cargo/config.toml 與 `--frozen`；fmt、Clippy、326 項單元測試、release、正式 WebView2／原生輸入區／5 組任務追蹤自檢通過。
- **專案及長流程：** AppContainer 檔案／網路隔離、副本修改與原件保護、來源搜尋／固定版本比較、30 份合計約 301 MiB LOG、原生工具、有限重試／恢復、兩組各 68 輪長任務、DPAPI 中斷續接、操作去重、圖片 13 組、PDF 讀取／快取失效／修改交付通過。重試時程使用測試時鐘；未把 24 小時政策測試描述為實際執行 24 小時。
- **Python：** 實際並行程序、草稿／版本綁定、程式功能驗收、Big5 大檔分段、來源與證據核對、Excel COM → CSV → pandas、XLSX 成果、檔案／網路／程序隔離、環境白名單、主動取消及實際逾時終止通過。
- **Office：** Word／Excel／PowerPoint 新舊格式讀寫及新建文件的樣式、保存重開；不同欄位順序的三份 Excel × 五時段共 15 張精確圖表；萬筆資料、錯誤／缺值、原始來源保護；PNG 嵌入及移除原圖後重開通過。
- **網路路徑：** 本機 SMB UNC 與暫時映射磁碟，唯一專案目錄、TXT 讀寫、DPAPI 重載、Word／Excel 保存重開、越界拒絕通過。這不是公司檔案伺服器的實測。
- **NSIS：** 實際交付 Setup 安裝、更新替換／重啟、鎖檔失敗保留舊版、Python 沿用／缺檔／同大小損壞／指紋不同時修復、失敗回復、缺少 WebView2 的啟動提示、安裝後自檢及解除安裝保留使用者檔案均通過。更新清單簽章與包體核對通過。
- **前端：** 本輪重跑圖表互動 8 組及分析摺疊／內容比較 31 項，均通過；明／暗／窄版內容比較與圖中文字畫面已目視核對。

本輪日誌：`.build/build-0.8.53-full.log`。報告：[整體驗證](../offline/exe-verification.json)、[NSIS](../offline/installer-verification.json)、[Python](../offline/python-verification.json)、[並行](../offline/batch-verification.json)、[程式驗收](../offline/code-verification.json)、[Office](../offline/office-verification.json)、[SMB](../offline/network-verification.json)、[LOG](../offline/log-verification.json)、[任務追蹤](../offline/task-tracking-verification.json)、[原生輸入區](../offline/composer-verification.json)、[工具鏈](../offline/environment.txt)。

## 行為與範圍

- POST等待回應且尚無task_id時不預先GET；明確4xx拒絕（408除外）或 `task_accepted:false` 停止本機追蹤並保存原HTTP狀態與伺服器訊息。已確認受理的任務、網路失聯／408／未明示拒絕的5xx仍只查原request_id，不自動換ID重送。
- GET連續三次相同錯誤，當輪models端點HTTP200且清單有效，才自動停止；成功或查詢路由ID改變重設計數。停止先保存片段及DPAPI狀態，晚到事件不能復活，停止不另發GET／取消。
- 完成答案的分析依據預設摺疊；同對話重繪保留手動展開及內層證據。圖表移除上方重複文字清單，新增入口在工具列，圖內雙擊編輯／刪除。
- 網站回報的413可能與目前及歷史附件文件ID合計上限有關，但缺回應本文尚未定案。本版修正桌面錯誤處理，不更改網站附件限制。415網站端已調整，實際PDF仍需公司測試。

## 初次基礎驗證（歷史階段）

2026-10-14 14:52 完成 `scripts/Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller`，結束碼 0，結果為 **PASS_BASIC**。EXE 與 NSIS 檔案版本均為 `0.8.53.0`；建置完成後使用者追加授權，將原始碼、產物與基礎驗證紀錄一併提交至既有 Git main。

- 實際 MSVC v142 `14.29.30133` x64 編譯器與 linker，C 探針確認 `_MSC_FULL_VER=192930159`；Rust `1.98.1`、SDK `10.0.19041.0`。
- 專案 `Cargo.lock`／`vendor`／`.cargo/config.toml`、新建空 Cargo 快取與 `--frozen`；fmt、Clippy 全 target、release 編譯通過。
- 聊天與附件相關 Rust 測試 21 項通過。包含背景／串流模式透過實際本機 HTTP 回傳 413、原始 HTTP 錯誤保留與敏感 token 遮蔽。
- 正式 EXE 的 WebView2、原生輸入區及 5 組任務追蹤自檢通過：POST 等待中不查詢、413 停止及 DPAPI 重啟恢復、第三次相同 404 且模型正常才停止、服務不正常時繼續／成功及變更錯誤時重設、忽略舊路由與停止後晚到事件。部分回答保存通過。
- Python runtime 準備／自檢、Outlook 輔助程序管線與 EOF 退出（未讀信箱）、DLL 相依檢查通過。
- Edge 圖表互動回歸 8 組及內容比較／分析摺疊 31 項通過；涵蓋圖中文字雙擊編輯／刪除、工具列新增入口，以及答案完成收起、手動展開、同對話重繪。
- NSIS 打包成功；正式 EXE 核對更新清單簽章與安裝包通過。兩份更新清單內容相同，均指向本版 NSIS。這是更新清單簽章核對，不是 Windows Authenticode 簽署聲明。

### 交付檔案

| 檔案 | 大小（bytes） | SHA256 |
| --- | ---: | --- |
| `dist/LM_AI.exe` | 17731584 | `f70db124f6e0a16015d3d1d8bd5779938fbe12cb9712e9655a867ef982ee36c0` |
| `dist/LM_AI_Setup.exe` | 28714300 | `1d97a28697d182cae0e6a19fc6713295ee4c976290420497a5b180139eee6c26` |

初次建置紀錄：`.build/build-0.8.53-basic.log`；當時的 `PASS_BASIC` 報告保存在 Git 提交 `6c5a4cf`。上方列出的 `offline` 報告現已更新為完整驗證結果。前端報告：`.build/chart-layout-review.json`、`.build/review52-ui.json`。

## 公司實測與界線

使用者已回報本版「測試 OK」，因此補做並通過上述完整開發環境驗證。未取得逐項公司測試清單，不將回報擴大為所有加密 PDF、模型品質、公司 SMB／防毒或真實 Outlook 信箱均已驗收。自動化聊天／圖片／伺服器 PDF 案例使用受控測試服務，Outlook 使用 COM 測試資料；本輪未另外執行實際 UltraVNC Viewer 連線，也未重製歷史離線 ZIP。
