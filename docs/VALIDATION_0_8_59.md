# 0.8.59 CAD 相容修正驗證

狀態：PASS_BASIC，2026-10-15。EXE／NSIS 0.8.59.0；完整驗證及公司 CAD 實機待補。

## 修改範圍

- 目前圖面缺少 Document.HWND 時，改探測應用視窗、程序建立時間與唯一文件名稱／路徑，寫入仍核對快照及桌面確認；缺少可用身分仍提供唯讀。
- 讀取介面不支援時回報失敗並禁止同任務反覆呼叫相同版本。
- 獨立尺寸／文字樣式：尺寸及引線字高3、箭頭3、ISO（DIMTIH=0、DIMTOH=1）、文字偏移1、simplex.shx。一般文字依明確 height 參數，技能預設3。保留 BigFont，或採已安裝的 chineset.shx；缺字不假裝驗收成功。
- 2008／2022保留原生引線。僅接納COM明示關聯且本次新增的空白／同內容文字，儲存重開再核對其關聯。未知額外物件仍拒絕。
- 逐項新增圖元與額外圖元描述寫入報告；未知額外圖元不忽略、不自動刪除。尚未證明使用者原報告多出物件的類型。
- 已讀尺寸／引線開放限定刪除後重建，不開放任意修改其屬性。

## 驗證界線

已完成：v142 x64 實際探針、空 Cargo 快取／--frozen、fmt／Clippy、選定 Rust／Python 合成契約、原生基本介面、CAD runtime／唯讀管線、NSIS 打包與清單核對。

待公司實測：2008／2022 樣式 COM 支援與中文字形、額外圖元是否仍產生、目前圖面閱讀／逐輪修改／Undo、存檔重開及 Excel→CAD。未執行完整 Office／SMB 與 NSIS 安裝更新回復解除安裝流程。

## 最終交付

- MSVC 14.29.30133，實際 _MSC_FULL_VER=192930159 x64；SDK 10.0.19041.0；Rust 1.98.1。
- Cargo.lock／vendor／.cargo/config.toml、空 Cargo 快取、--frozen、fmt／Clippy 與 49 項選定 Rust 契約通過。
- 32 項 Python 合成契約通過；新增缺少 HWND 唯讀／應用視窗備援、禁止不支援介面重試、獨立樣式與預設還原、BigFont、原生引線附屬物件與未知圖元拒絕、尺寸限定刪除。
- 原生 WebView2 基礎檢查、正式及 dist CAD runtime／唯讀登錄管線、Python runtime、Outlook 無效要求管線（未讀信箱）、任務追蹤／程式碼唯讀基本回歸通過。
- NSIS 包裝、簽署更新清單及原生產物核對通過；未執行安裝生命週期完整測試。
- 紀錄：.build/build-0.8.59-release.log；offline/exe-verification.json、offline/cad-integration-verification.json。

| 產物 | bytes | SHA256 |
|---|---:|---|
| LM_AI.exe | 18599424 | bed6ff33289b383154fe91a0dc336f5b45fa772570bb8a95479fe08f79783111 |
| LM_AI_Setup.exe | 36098532 | 18c97caeedfc3ab7e7c0e07bbcdda04ca2f19a0a48fb0fd8492025a6b24e2129 |
| update-manifest.json | 1050 | c0bcf49a5f841a7239288049fff6db39067a202bb7a1f7d3eeecbffc6422b32b |
| update-manifest-exe.json | 1050 | c0bcf49a5f841a7239288049fff6db39067a202bb7a1f7d3eeecbffc6422b32b |
