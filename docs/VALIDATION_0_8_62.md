# 0.8.62 基礎驗證

狀態：PASS_BASIC，2026-10-15。EXE／NSIS 0.8.62.0；完整驗證及公司 CAD 實機待補。

## 修改

舊版 ZOOM E/A 使用 ActiveViewport；指令能力檢查先於新圖建立（有文件時）與既有文件修改。前置缺介面明示未執行，允許修改計畫；未知 COM 失敗維持停止。

介面參考：[舊版 IntelliCAD Viewport 縮放範例](https://developer.bricsys.com/bricscad/help/en_US/V9/DevRef/source/IDR_VBA_Methods798.htm)、[Viewport 成員](https://developer.bricsys.com/bricscad/help/en_US/V22/DevRef/source/Objects_Viewport.htm)。不同產品／版本文件僅作介面對照，實際2008／2022仍待使用者驗收。

限制：無本機 CAD 實機；合成測試不代表指令或圖面外觀實測成功。完整安裝生命週期與公司服務整合待補。

## 最終交付

- MSVC 14.29.30133，實際 _MSC_FULL_VER=192930159 x64；SDK 10.0.19041.0；Rust 1.98.1。
- Cargo.lock／vendor／.cargo/config.toml、空 Cargo 快取、--frozen、fmt／Clippy 與 49 項選定 Rust 契約通過。
- 44 項 Python 合成契約通過；新增舊版Viewport／新版Application縮放、缺介面不建圖、目前圖面不進Undo、無文件延後檢查及可修正重試契約；未啟動 CAD。
- 原生 WebView2 基礎檢查、正式及 dist CAD runtime／唯讀登錄管線、Python runtime、Outlook 無效要求管線（未讀信箱）、任務追蹤／程式碼唯讀基本回歸通過。
- NSIS 包裝、簽署更新清單及原生產物核對通過；未執行安裝生命週期完整測試。
- 紀錄：.build/build-0.8.62-release.log；offline/exe-verification.json、offline/cad-integration-verification.json。

| 產物 | bytes | SHA256 |
|---|---:|---|
| LM_AI.exe | 18608128 | e9fb1b9187747ca257fb69b442a20f35253e40bfe05682460dd1c18d439460f9 |
| LM_AI_Setup.exe | 36102511 | bf3cfdfbc1f3a465c8091c8797645b41deae6937af507764bc08e942536ce0e0 |
| update-manifest.json | 1050 | f93e3fa643b598f744538661774cb99f9db332a7807725c627298dacbd2824a0 |
| update-manifest-exe.json | 1050 | f93e3fa643b598f744538661774cb99f9db332a7807725c627298dacbd2824a0 |
