# 0.8.64 基礎驗證

狀態：PASS_BASIC，2026-10-15。EXE／NSIS 0.8.64.0；完整驗證及公司 CAD 實機待補。

原生線型與圖元比例、新建／已讀圖元套用及讀回；目前圖面純 ZOOM 不依賴 Undo，其他修改仍要求群組。詳細範圍見 [功能說明](PENDING_CAD_LINETYPES.md)。

限制：2008／2022 線型載入位置、比例外觀、實際縮放仍待公司實測。缺標準 LIN 檔須先由使用者載入；取樣曲線線型逐段起算。iCAD 2008 圖元編輯的 Undo 介面限制仍保留。完整安裝生命週期及公司服務驗證未進行。

## 最終交付

- MSVC 14.29.30133，實際 _MSC_FULL_VER=192930159 x64；SDK 10.0.19041.0；Rust 1.98.1。
- Cargo.lock／vendor／.cargo/config.toml、空 Cargo 快取、--frozen、fmt／Clippy 與 49 項選定 Rust 契約通過。
- 55 項 Python 合成契約通過；新增線型驗證／載入／讀回與目前圖面不儲存、純縮放無Undo及混合操作保護；CAD行為為合成契約，未啟動CAD。
- 原生 WebView2 基礎檢查、正式及 dist CAD runtime／唯讀登錄管線、Python runtime、Outlook 無效要求管線（未讀信箱）、任務追蹤／程式碼唯讀基本回歸通過。
- NSIS 包裝、簽署更新清單及原生產物核對通過；未執行安裝生命週期完整測試。
- 紀錄：.build/build-0.8.64-release.log；offline/exe-verification.json、offline/cad-integration-verification.json。

| 產物 | bytes | SHA256 |
|---|---:|---|
| LM_AI.exe | 18641920 | b1a1ce8520a289284552121167f138c6178747efc78c62c2aa7c296d207f6521 |
| LM_AI_Setup.exe | 36103133 | c927d5ded80408ad1f2b2498a53d667252ab797ca63a9017e05c4204d54a8725 |
| update-manifest.json | 1050 | 87c30401d0d19ac27a530b9079236066bedeba085b7078dd5a8cc4d57191c7d6 |
| update-manifest-exe.json | 1050 | 87c30401d0d19ac27a530b9079236066bedeba085b7078dd5a8cc4d57191c7d6 |
