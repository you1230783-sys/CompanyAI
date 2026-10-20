# 0.8.57 CAD 連線與副本編輯測試版

狀態：PASS_BASIC，2026-10-14。最終 EXE／NSIS 0.8.57.0；完整驗證延後。

## 本輪範圍

- 連線失敗由原生控制器／UI 事件通道等待使用者選擇開啟、重試、取消；回覆核對對話／任務／請求 ID；取消、期限及視窗關閉不視為同意。
- 途中已寫入的 COM 斷線只允許使用者選擇另建新副本重做，不從失敗操作接續、不關閉使用者文件。取消不自動重送。
- 直徑字串使用 %%c；實機使用者確認手動輸入 %%c10 可顯示。程式送出、存檔及重開仍待公司驗證。
- 新圖元核對以唯一 Handle 集合及屬性為準，Count 差異留紀錄；未知額外／缺失圖元仍停止。未取得本次真實圖元集合差異，不預先宣稱已修好所有計數問題。
- 新增 cad_read／cad_edit，原檔鎖定唯讀，先複製後開啟；讀取分頁與修改以來源 revision、索引 SHA256、已讀 Handle 為依據，來源變動即拒絕。
- 修改支援文字、ACI、位移及新增線／圓／文字／引線；只驗證 ModelSpace 可讀屬性。圖塊、代理物件、外部參照及圖紙空間不做完整語意驗證。

## 基礎驗證流程

`scripts/Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller`

包含 v142 實際 x64 編譯器探針、空 Cargo 快取與 --frozen、fmt／Clippy、選定 Rust 契約、Python 合成 CAD 操作／存檔重開、來源唯讀保護、原生 CAD runtime／唯讀管線、既有基本 UI／聊天與程式碼唯讀流程、NSIS 打包與更新清單核對。

新增 UI 合成案例覆蓋 CAD 開啟按鈕、部分操作只允許另建副本及 Escape 取消；是否實際執行以本輪建置紀錄為準。

## 尚未驗證

公司 iCAD 2008／GstarCAD 2022 真實連線視窗、DWG 開啟／修改／儲存重開、大型圖塊檔效能、公司加密、實際 AI 工具選擇與視覺排版。完整 Office／SMB／長任務／NSIS 安裝更新回復解除安裝延後。

CPython 3.13.12／pywin32 311 為既有官方鎖定二進位，不宣稱本機 v142 編譯。保留歷史離線 ZIP 與未追蹤 icon.png。

## 最終交付結果

- MSVC 14.29.30133／_MSC_FULL_VER=192930159 x64，SDK 10.0.19041.0，Rust 1.98.1。
- 空 Cargo 快取／--frozen、fmt／Clippy、43 項選定 Rust 測試及 17 項 Python 合成 CAD 契約通過。
- 原生 WebView2／CAD 對話框合成檢查、正式與 dist CAD runtime／唯讀管線、Python runtime、Outlook 管線（不讀信箱）、程式碼唯讀及任務停止追蹤基本流程通過。
- NSIS 打包與原生更新清單／產物核對通過。公司 CAD 實際繪圖與安裝生命週期未測。
- 最終建置紀錄：.build/build-0.8.57-final.log；機讀紀錄：offline/exe-verification.json、offline/cad-integration-verification.json。

| 產物 | SHA256 |
|---|---|
| LM_AI.exe | 3386ff4c7587d88a2d98d7e91fb5dba09cdf266463fd2468e59bfb1862d65d65 |
| LM_AI_Setup.exe | e3e5ef54ffd8f543b90b0490dd83e7309e5b82137c5a990d59e1551fa95b1ced |
| update-manifest.json | d7c0c22ccf192eda66690ae19a0ac03154acb9e8040d6a7df00c41524e251b5b |
| update-manifest-exe.json | d7c0c22ccf192eda66690ae19a0ac03154acb9e8040d6a7df00c41524e251b5b |
