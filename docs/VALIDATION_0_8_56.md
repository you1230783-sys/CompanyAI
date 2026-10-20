# 0.8.56 CAD 相容性與復原修正驗證

狀態：PASS_BASIC，2026-10-14。EXE／NSIS 0.8.56.0；完整驗證依使用者要求延後。

## 修正範圍與實機證據

- 使用者回報 0.8.55 已在佑達 iCAD 2008 連線、新建矩形、四圓及兩項尺寸；第一條引線在 Points.Item 取值失敗，尚未 SaveAs。這不是磁碟上已有部分 DWG 的證據。
- 本版 Points.Add 直接傳入 x/y/z，同時涵蓋引線與剖面種子點；不對所有 COM 集合改索引。使用較新 IntelliCAD 官方方法定義作為依據，2008 實機仍待重測。
- 未繪圖的明確錯誤可重試；已建立圖面且收到 E_INVALIDARG 可修改計畫另建圖面。部分失敗計畫以 version/operations 雜湊記錄，換檔名不可重播，每任務六次上限，狀態隨任務保存恢復。
- 未知結果、SaveAs／重開失敗、逾時、取消仍保留鎖定。失敗不會關閉或修改部分圖面；新增計畫另建文件。
- 預定路徑、實際存在、已儲存、已返回操作數及交付驗證分開。失敗不提供成功 path；模型提示不得宣稱預定位置已有 DWG。
- 佑達產品不要求安裝目錄名稱含品牌；已知 CLSID／ProgID／版本與實際路徑核對保留，未知 IntelliCAD 不自動宣稱相容。

## 基礎流程

`scripts/Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller`

MSVC v142 x64 實際探針、空 Cargo 快取、vendor／Cargo.lock／.cargo/config.toml 與 --frozen、fmt／Clippy、選定 Rust 契約、Python CAD 合成契約、CAD runtime 唯讀登錄與匿名管線、既有基本原生流程、NSIS 打包及更新清單核對。

Python 測試覆蓋直接加入舊版座標、已知／未知錯誤復原與安裝資料夾品牌無關的身份辨識；替身測試不等於實機 CAD 相容性。

## 尚未驗證

公司 iCAD 2008／GstarCAD 2022 實際繪圖、AI 端到端重試、引線箭頭與顏色外觀、完整 Office／SMB／長流程、NSIS 安裝／更新／回復／解除安裝。

舊版鎖定任務不自動解除；更新後請建立新任務測試。本版發生可復原錯誤時才適用新的同任務復原規則。

CPython 3.13.12 與 pywin32 311 使用既有官方鎖定輸入，不宣稱為本機 v142 編譯；未重製歷史離線 ZIP。使用者授權推送既有 GitHub main，包括來源、產物與驗證文件。

## 實際交付結果

- v142 14.29.30133，_MSC_FULL_VER=192930159 x64，SDK 10.0.19041.0，Rust 1.98.1。
- 空 Cargo 快取／--frozen、fmt／Clippy、42 項選定 Rust 測試、13 項 Python CAD 合成契約均通過。
- CAD 正式與 dist runtime 唯讀自檢、Outlook 管線（未讀信箱）、Python runtime、WebView2／原生 UI／任務停止追蹤、程式碼唯讀流程通過。
- NSIS 打包與原生更新清單／產物核對通過。未進行安裝生命週期與實際 CAD 繪圖測試。
- 建置記錄：.build/build-0.8.56.log；機讀紀錄：offline/exe-verification.json、offline/cad-integration-verification.json。

| 產物 | SHA256 |
|---|---|
| LM_AI.exe | 7f2d3a45deaae24a51c6305c64f2f26aad247db81059f15c3a787b3d35dc762c |
| LM_AI_Setup.exe | bfc95ecb41cae075d12031be18c6b561452573d17e6b4f35663fb8769427ead5 |
| update-manifest.json | 8307979c0020dd88e15a47d1388ad65effbb7e27b69c909981d9940a7c07f525 |
| update-manifest-exe.json | 8307979c0020dd88e15a47d1388ad65effbb7e27b69c909981d9940a7c07f525 |
