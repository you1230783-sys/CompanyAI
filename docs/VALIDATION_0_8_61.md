# 0.8.61 基礎驗證

狀態：PASS_BASIC，2026-10-15。EXE／NSIS 0.8.61.0；完整驗證及公司 CAD 實機待補。

## 修改

引線使用原生文字參數、剖面選取集先清空才刪除、COM列舉重設與逐操作診斷。新增受限 CAD 指令，維護操作核對可讀圖元且目前文件不儲存。

介面依據：[AddLeader](https://www.progesoft.com/products/progecad-professional/manual?mp=developer-reference/api-reference/methods/addleader-method-intellicad-2)、[SelectionSet](https://www.progesoft.com/products/progecad-professional/manual?mp=developer-reference/api-reference/objects/selectionset-collection/members)、[Document](https://www.progesoft.com/products/progecad-professional/manual?mp=developer-reference/api-reference/objects/document-object/members)。較新 API 文件用於語意對照，非2008實測證明。

限制：無本機 CAD 實機，圖塊定義及不可讀屬性非完整核對；AUDIT N 呼叫返回不是無錯誤證明。NSIS完整安裝生命週期及公司服務整合驗證待補。

## 最終交付

- MSVC 14.29.30133，實際 _MSC_FULL_VER=192930159 x64；SDK 10.0.19041.0；Rust 1.98.1。
- Cargo.lock／vendor／.cargo/config.toml、空 Cargo 快取、--frozen、fmt／Clippy 與 49 項選定 Rust 契約通過。
- 40 項 Python 合成契約通過；新增原生引線文字、剖面邊界保留、列舉器重設、指令語法拒絕、維護指令內容不變與忙碌狀態拒絕；未啟動 CAD。
- 原生 WebView2 基礎檢查、正式及 dist CAD runtime／唯讀登錄管線、Python runtime、Outlook 無效要求管線（未讀信箱）、任務追蹤／程式碼唯讀基本回歸通過。
- NSIS 包裝、簽署更新清單及原生產物核對通過；未執行安裝生命週期完整測試。
- 紀錄：.build/build-0.8.61-release.log；offline/exe-verification.json、offline/cad-integration-verification.json。

| 產物 | bytes | SHA256 |
|---|---:|---|
| LM_AI.exe | 18607104 | 2e81c8500a423bf13b480e90ae5893ed20f42ca1ae336d1f3cf69eb24c284dd6 |
| LM_AI_Setup.exe | 36100655 | f3afd76f7d9b6cfcff507c8991a37b64d0f6855ad8a0f63ea1190eabbdec9500 |
| update-manifest.json | 1050 | db930b0413ec66dd46571ed295f82403dbdf9c2d57c35529679e9306cae84333 |
| update-manifest-exe.json | 1050 | db930b0413ec66dd46571ed295f82403dbdf9c2d57c35529679e9306cae84333 |
