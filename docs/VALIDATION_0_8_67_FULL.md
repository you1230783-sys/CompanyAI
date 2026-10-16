# 0.8.67 完整本機驗證補測

狀態：核心完整驗證通過；等待關閉使用中的 Office，尚未完成本輪交付。日期：2026-10-16。

使用者要求在 CAD 實測後補跑之前略過的完整驗證，全部通過後才打包與推送。
本輪目前僅修正測試程式，未改動產品功能，維持版本 0.8.67。

## 測試修正

- 工具 Schema 測試原先固定為 67 個工具；改為與正式目錄比對名稱集合，並拒絕重複或遺漏項目。
- 教學保存測試改用獨立暫存目錄，避免與專案 Broker 路徑鎖測試共用 `.build`，造成 Windows 原子重新命名失敗。未放寬正式路徑保護或改動儲存程式。
- 原生代理測試補上新增的基本程式碼搜尋／分段閱讀工具；CSV 圖表轉換先載入 `charts`，符合技能拆分後的實際契約。
- Python 批次測試依技能階段分組：資料繪圖與基本閱讀並行，Python 成功／失敗並行另行驗證；保留兩個 Python 執行時間確實重疊、去重及部分成功結果的檢查。
- 設定頁測試更新為四個分頁，加入正式 14 張教學目錄、更新日誌、窄視窗與重看後返回設定的檢查。
- 安裝生命週期測試增加安裝後 `--cad-self-check`，驗證 CAD runtime 載入及登錄查詢管線；不藉此啟動或繪製 CAD。

## 重跑方式

從專案根目錄執行：

```powershell
./scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller -TestOffice -TestVnc `
  -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build'
```

網路測試只能使用已存在的本機 localhost 分享子目錄；其他開發機需替換實際位置，不建立分享或調整安全設定。
Office 測試前不得有使用者開啟的 Word／Excel／PowerPoint；安裝測試要求正式及隔離測試安裝位置均未使用。

另外執行 `scripts/Test-Charts.js`、`Test-ReplyTables.cjs`、`Test-UiAuth.cjs`、`Test-ChartLayout.cjs`、`Test-Settings.cjs`、`Test-Review.cjs` 與 `tools/test_onboarding.cjs`，以及 `Test-OutlookTable.ps1 -SelfTest`。

## 已完成的補測

- MSVC v142 x64 實際編譯器探針、空 Cargo 快取／`--frozen`、fmt、Clippy、342 項 Rust 測試與 61 項 CAD 合成契約。
- 正式 EXE 的 WebView2、composer、任務追蹤、Python／CAD runtime 與 Outlook helper 管線自檢。
- 專案整合：AppContainer 檔案／網路隔離、唯讀程式碼分頁、30 份約 10 MB LOG、26 個原生代理案例、重試與中斷續接、圖片、技能、加密筆記、PDF 擷取及本機模擬轉檔 API。
- Python 完整獨立重測：並行、CSV／XLSX、Big5／大型 LOG、程式修訂與驗收、實際 Excel COM、來源不變、環境白名單、取消、120 秒逾時及 runtime 損毀偵測。報告在 `.build/python-full-retry-ac36fac3a2e64f80b5aa7e7587b7b298/python-verification.json`。
- 七組前端腳本通過；設定頁追加正式教學／日誌目錄的窄視窗檢查，報告 `.build/settings-full-0.8.67.log`。Outlook Table 合成 COM 的兩種 SAFEARRAY 排列與 1／100 列讀取通過，未存取信箱。
- `Build.ps1 -EmptyCargoCache -ValidateOnly -TestVnc` 最終完整通過，包含全部核心、專案、Python 與真實 UltraVNC 連線本機合成 RFB。預設及全螢幕唯讀模式都通過；未連線公司電腦。報告 `.build/full-validation-0.8.67-r4.log`，結束碼 0。

## 尚未完成

- 現有「文件1 - Word」仍開啟。`Test-Office.ps1` 明確要求 Word／Excel／PowerPoint 全部關閉；已請使用者儲存關閉，不代為結束程序。Office 全格式與圖片、SMB Office 文件驗證等待此前置條件。
- 本輪 NSIS 重打包、安裝／更新／rollback／解除安裝與 Git 推送尚未執行。`dist` 與原發行報告保留先前狀態，不以本輪部分通過覆蓋成完整發行。
- `.build/full-validation-0.8.67-r3.log` 保存完整專案通過、Python 舊技能測試失敗的歷程；不能單憑其中部分 PASS 宣稱整套通過。最後只驗證重跑記錄在 `.build/full-validation-0.8.67-r4.log`。

## 驗證界線

- CAD 2008／2022 的固定離線實機資料測試見 [原始報告](VALIDATION_0_8_67.md)；圖形外觀與中文排版仍需人工確認，2008 未實作的 Measurement 屬性不列為通過。
- 原生代理、重試與 PDF API 流程使用本機模擬伺服器；不代表公司 AI 服務、登入、企業加密文件、真實 Outlook 信箱或公司遠端電腦已驗收。
- Office、SMB、VNC 測試使用合成資料及本機端點。不得把本機通過等同公司環境通過。
- 不重製歷史 Rust 離線 ZIP；本輪交付範圍為 EXE、NSIS、簽署更新清單與對應原始碼／驗證紀錄。
