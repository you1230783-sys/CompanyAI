# 0.8.58 基礎驗證

狀態：PASS_BASIC，2026-10-15。EXE／NSIS 0.8.58.0；完整驗證及公司 CAD 實機待補。

本版範圍：目前圖面不儲存修改、共用副本操作、Excel 座標計畫；不包含 CAD 指令／圖塊清理。

已完成：指定 MSVC v142 x64 實際編譯器探針、空 Cargo 快取／vendor／Cargo.lock／--frozen、fmt／Clippy、選定 Rust 契約、Python 合成 CAD 契約、原生 WebView2／CAD runtime 自檢、NSIS 包裝及簽署更新清單。

公司實測重點：2008/2022 文件 HWND 與 Undo API、切換圖面拒絕舊計畫、不存檔連續修改、手動 Undo、Excel 真實資料的欄位／單位／顯示外觀。未執行完整版 Office／SMB／NSIS 安裝更新回復解除安裝。本機合成測試不表示真實 CAD 相容性。

上游 CPython 3.13.12／pywin32 311 為鎖定官方預編譯檔，不宣稱由本機 v142 編譯。無新增 Rust crate，歷史離線 ZIP 不重製。

## 最終交付

- MSVC 14.29.30133，實際 _MSC_FULL_VER=192930159 x64；SDK 10.0.19041.0；Rust 1.98.1。
- Cargo.lock／vendor／.cargo/config.toml、空 Cargo 快取、--frozen、fmt／Clippy 與 48 項選定 Rust 契約通過。
- 26 項 Python 合成契約通過：目前圖面不 Save／Close、Undo 群組、不支援 Undo 時未修改、過期內容／不同文件拒絕、半徑與刪除、圓弧端點、副本儲存重開及原有繪圖契約。
- 原生 WebView2（含目前圖面確認與提示切換）、正式及 dist CAD runtime／唯讀登錄管線、Python runtime、Outlook 無效要求管線（未讀信箱）、任務追蹤／程式碼唯讀基本回歸通過。
- 原生 UI 測試抓到延遲 close 事件取消新 CAD 提示的競態，已修正並重跑通過；工具目錄另有小於 64000 bytes 的回歸檢查。
- NSIS 包裝、簽署更新清單及原生產物核對通過；未執行安裝生命週期完整測試。
- 最終建置紀錄：.build/build-0.8.58-release.log；機讀紀錄：offline/exe-verification.json、offline/cad-integration-verification.json。

| 產物 | bytes | SHA256 |
|---|---:|---|
| LM_AI.exe | 18597888 | 5a916a6123c1fd19a584d1ca5ee1e7830c42daa0cb519ca523bbbe026a870ba6 |
| LM_AI_Setup.exe | 36096332 | 2f3d88b795e3b8e65c4d5d53c5f9ca8bce08120c9026c8c66a55acfaa416b480 |
| update-manifest.json | 1050 | ea8f55284443f709a24bb36c2fe1ea589f75183bb7994106ce9763888f83a9c3 |
| update-manifest-exe.json | 1050 | ea8f55284443f709a24bb36c2fe1ea589f75183bb7994106ce9763888f83a9c3 |
