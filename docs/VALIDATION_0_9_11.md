# 0.9.11 原始碼交付驗證

本輪依使用者公司實測回饋實作並推送 Git；沒有產生 0.9.11 安裝包或 Release。安裝、升級及公司 EIS／DRT 長任務不能沿用前一版本的 PASS。變更對照見 [實作清單](IMPLEMENTATION_0_9_11.md)。

## 重跑入口

- `scripts/Build.ps1 -ValidateOnly -EmptyCargoCache -TestOffice -SkipCad`：v142 探針、fmt、all-targets Clippy、非 CAD Rust、release panic、EXE、Python、WebView2、專案、Office 與排程整合。使用本專案 Cargo.lock、vendor、.cargo/config.toml；不更新 dist 或簽署清單。
- `scripts/Test-PythonRevision.ps1`：較短的真 Broker／AppContainer 合成 BOM CSV → source_revision → SciPy NNLS → CSV／XLSX／PNG 回歸；來源實際變更仍拒絕。
- `scripts/Test-ExportFormat.ps1 -Report <本次 document-work-verification.json>`：用真正 Word 重開合成匯出文件，核對粗斜體、UTF-16 emoji 範圍、等寬程式碼、縮排／空行／背景與後續段落格式重設。
- `scripts/Test-ScheduleBusy.ps1 -Exe <本次EXE> -Probe <schedule_smoke.exe>`：獨立 Excel 合成工作簿占用時不派送工具、保留重複排程及略過審計。
- `scripts/Test-DependencyAudit.ps1`：刷新 RustSec 與 12 項政策案例。既有 ttf-parser 停止維護的精確期限例外仍公開保留，不能宣稱零警告。
- 瀏覽器入口：`Test-Conversations`、`Test-DocumentWork`、`Test-Schedules`、`Test-PetTarot`、`Test-DraftTools`、`Test-Workbench`、`Test-BuildFeatures`、`Test-Preferences`、`Test-ReliabilityUI`、`Test-ReplyTables`、`Test-UiAuth`、`Test-Review`、`Test-Settings`、`Test-ChartLayout`（scripts 中的 `.cjs`）。

本次採合併證據，**不是單次 Build 全程零退出**：`validate-0.9.11-r4.log` 完成 Rust／EXE／UI／專案，`continue-0.9.11.log` 完成 Python 與 Office，`document-tail-0.9.11.log` 完成 DOCX／可抽取文字 PDF，`export-tail-0.9.11.log` 完成格式／圖片／交付／排程。四段使用相同整合 EXE，SHA256 為 `0beaef156a7b950c140f9e0531fee2c7af09e6c40dd96ab5a880aa6b875cab1b`；原始日誌雜湊與分段結果見[合併索引](../offline/source-verification-0.9.11.json)。

整合後僅補 `ProjectRuntime::received_reply` 的 `#[cfg(feature = "pet")]`，排除核心配置未使用方法；不改預設功能行為。最終預設 EXE 為 `2ed2a4ba49c8300c09bd2e28638acc05661420ec3a861a3460f2fb43ca5ed4de`，v142 空快取 frozen 建置、核心配置及原生 UI 自檢見[最終建置紀錄](../offline/final-build-verification-0.9.11.json)；不能把不同 EXE 雜湊混為同一產物。

## 開發中失敗與修正

以下均保留在本機 `.build` 日誌，不能把最初失敗描述為一次通過：

1. 初次編譯的 PNG 模組可見性、測試初始化缺少 received_reply、原生導覽借用衝突；修正後 BuildOnly 成功。
2. python-analysis 主技能超過既有 1,500 字元界線；保留必要介面說明，完整科學計算範例移到 python-tables，未放寬技能／筆記上限。
3. 新範本教學的 JavaScript 換行字串與測試沿用舊版對話框數目／收縮區塊；修正腳本及新版 UI 斷言，58 個教學入口另經瀏覽器測試。
4. 原生 WebView2 自檢兩次遇到舊「專案收縮區塊／專案清單位於側欄」斷言；改為核對專案管理模態框與側欄按鈕位置，不刪除導覽測試。
5. 新增詳情內容測試曾使用缺少範圍欄位的合成 coverage；修正測試資料後，實際訊息重繪驗證工具紀錄、交付清單、分析共同收合及手動展開保留。
6. 新增 Python 回歸已產出三份檔案後，測試錯把 CSV 的 `artifacts[0].dataset.path` 寫成 `artifacts[0].path`。修正測試欄位後，以相同應用 EXE 接續 Python／Office／排程階段；沒有重播前面已通過的長流程，也沒有變更應用以迎合測試。
7. DOCX 含彩色 emoji 的匯出已通過，但本機 Word 轉 PDF 後將 U+1F600 呈現為圖像，文字抽取缺少此符號，既有完整性檢查拒絕標示完成。Windows PDF 渲染畫面確認符號可見，pypdf 抽取也確認缺字；未放寬產品檢查。DOCX 保留 emoji 座標測試，PDF 改以不含該符號的相同 Markdown 內容驗證；此為測試範圍拆分，並非宣稱彩色 emoji PDF 已通過。相關 API 背景可查 [Microsoft ExportAsFixedFormat](https://learn.microsoft.com/en-us/dotnet/api/microsoft.office.tools.word.document.exportasfixedformat?view=vsto-2022)。
8. 新格式檢查在前一步 COM 程序尚未自行退出時立即判定 Office 占用；程序隨後自行結束。兩個新增 Office 檢查入口加入最多十秒等待，逾時仍停止，未強制關閉任何 Office 或使用者文件。
9. 核心配置 Clippy 發現桌寵專用的 `received_reply` 方法未使用，補正 feature 條件後重跑；沒有以 allow(dead_code) 隱藏警告。

## 實際驗證結果

| 範圍 | 結果與界線 |
|---|---|
| 工具鏈／離線編譯 | PASS；Rust 1.98.1、MSVC v142 14.29.30133、實際 `_MSC_FULL_VER=192930159` x64、SDK 10.0.19041.0；空 Cargo 快取、`--frozen` |
| Rust／lint | PASS；非 CAD 456 項（60 項 CAD 篩除）、核心配置 472 項、fmt、兩配置 all-targets Clippy；核心配置使用另一個新空 Cargo 快取 |
| 前端 | PASS；[14 組索引](../offline/ui-verification-0.9.11.json)，含一般／專案新對話、模態工具、520px 工具列、桌寵目標／捷徑、匯出資料夾狀態、三項詳情共同收合、58 個教學入口 |
| 依賴政策 | PASS；[RustSec 紀錄](../offline/dependency-audit-0.9.11.json)，保留已審核的停止維護例外 |
| 專案／恢復 | PASS；28 組原生代理、9 組韌性、14 組圖片，含兩次 134 POST 長流程、checkpoint 中斷恢復、60 次工具操作後保存／接續、PDF 快取與來源變更失效；使用合成服務，不是公司模型品質驗收 |
| Python | PASS；[原生報告](../offline/python-verification-0.9.11.json)。BOM 版本串接、SciPy NNLS、無副檔名 CSV／XLSX／PNG、中文字型圖形目視、真 Excel COM→CSV→pandas、批次平行／去重、隔離、取消、120 秒逾時及環境／成果竄改拒絕 |
| Office／Markdown | PASS；[Office](../offline/office-verification-0.9.11.json)、[文件](../offline/document-work-verification-0.9.11.json)、[格式讀回](../offline/export-format-verification-0.9.11.json)。Word／Excel／PowerPoint 保存重開、DOCX 粗斜體／emoji 範圍／表格／等寬字／程式碼空白與背景／正文格式重設；可抽取文字 PDF 讀回及 Windows PDF 轉 PNG 目視通過，彩色 emoji PDF 限制另列 |
| 排程 | PASS；[持久執行](../offline/schedule-verification-0.9.11.json)、[實際 Excel 占用](../offline/schedule-busy-verification-0.9.11.json)。占用時零派送／零輸出、保留審計及下次排程；完成不可重播、送出前／保存後不確定中斷仍拒絕續接 |

## 公司端仍需確認

- 使用者提供的日誌證實文字 revision 與原始 bytes revision 不同，但未提供原始 CSV bytes；不能斷言公司檔案一定有 BOM。原生回歸用已知 BOM 合成檔驗證相同故障類型。
- 公司 EIS／DRT 全流程、反演方法／正則化選擇與報告品質未驗收；SciPy 可用不代表方法正確。模型生成的 NameError、錯誤換行仍須依真實 traceback 修正。
- 專案後端目前輪詢模型完成回覆；桌寵在取得第一份非空模型回覆時切成工作，並非服務端尚未暴露的串流 token 即時事件。一般對話依實際收到的文字切換。
- PDF 中被 Office 圖像化的彩色 emoji 可能無法通過完整文字讀回，程式會保留已產生檔案並提示未完成，不靜默當成成功。此情況可使用 DOCX 並人工確認 PDF；不能把本輪可抽取文字 PDF 的 PASS 當成所有字型／符號皆受支援。
- 原生通知受 Windows 通知設定、程式通知偏好及桌寵開關影響；公司端托盤通知點擊、多螢幕 DPI、休眠／鎖屏仍需實測。
- 本輪沒有新測 CAD、公司 VNC／SMB／Outlook 信箱、加密文件或 NSIS 安裝生命週期；沒有宣稱公司伺服器已部署，也沒有發布 0.9.11 Release。
