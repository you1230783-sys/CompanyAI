# 0.8.63 基礎驗證

狀態：PASS_BASIC，2026-10-15。EXE／NSIS 0.8.63.0；完整驗證及公司 CAD 實機待補。

## 修改

缺COM HWND時增加程序完整路徑／唯一PID／建立時間與文件快照備援。修正Documents重複列舉；目前圖面失敗明示身分原因，指南禁止為縮放反覆讀完整圖元／操作紀錄。

Windows介面依據：[QueryFullProcessImageNameW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew)、[GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes)。有限查詢權限，不提升CAD權限。

限制：備援僅能確認唯一可查詢程序、文件名稱路徑與可讀內容，不等同文件專屬HWND；仍依賴使用者確認及操作中不切換文件。公司iCAD2008／2022目前圖面指令待實測。五分鐘AI推論連線失敗未在此版修正。完整安裝生命週期與公司服務整合待補。

## 最終交付

- MSVC 14.29.30133，實際 _MSC_FULL_VER=192930159 x64；SDK 10.0.19041.0；Rust 1.98.1。
- Cargo.lock／vendor／.cargo/config.toml、空 Cargo 快取、--frozen、fmt／Clippy 與 49 項選定 Rust 契約通過。
- 49 項 Python 合成契約通過；新增程序完整路徑、唯一性、桌面視窗、缺COM HWND、同名文件及程序重啟拒絕舊授權；含本機目前程序實際Win32查詢，其他CAD行為為合成契約，未啟動CAD。
- 原生 WebView2 基礎檢查、正式及 dist CAD runtime／唯讀登錄管線、Python runtime、Outlook 無效要求管線（未讀信箱）、任務追蹤／程式碼唯讀基本回歸通過。
- NSIS 包裝、簽署更新清單及原生產物核對通過；未執行安裝生命週期完整測試。
- 紀錄：.build/build-0.8.63-release.log；offline/exe-verification.json、offline/cad-integration-verification.json。

| 產物 | bytes | SHA256 |
|---|---:|---|
| LM_AI.exe | 18610176 | a50d751fa08a170f23e167a33ad54728f2c201182b6fe8c24651057f5d562d9f |
| LM_AI_Setup.exe | 36104990 | c2fcfe5258a905f41aab1f01ab5e15888f5d04e3a32c2a02a368fd3da4eea37d |
| update-manifest.json | 1050 | 00f5e95dcf999451cc302c08635a0ff6d3cb4bd3ecdef2ca0c5b7765e7f0c3c7 |
| update-manifest-exe.json | 1050 | 00f5e95dcf999451cc302c08635a0ff6d3cb4bd3ecdef2ca0c5b7765e7f0c3c7 |
