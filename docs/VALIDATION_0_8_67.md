# 0.8.67 基礎驗證與 CAD 實機測試

狀態：PASS_BASIC_AND_CAD_DATA，2026-10-16。EXE／NSIS 0.8.67.0。

本版以固定離線計畫經正式 Broker／runtime／COM 測試本機佑達 ICAD 2008 5.0.20.0 與浩辰 ICAD 2022（GStarCAD.Application.22）。不連公司服務，不能將結果視為 AI 提示詞／工具選擇測試。

修正依據：2008 缺少型別庫登錄；2022 標準 LIN 位於 UserDataCache/Support；2008 縮放引起約 1e-14 的座標末位差異，原生尺寸首次重繪後才提供 TextString。後者記錄首次可讀欄位，已可讀文字改變與其他集合差異仍會停止。

本機安裝修復已經使用者授權，備份留在 `.build/icad-registration-before.json`，不納入安裝包自動執行。

## 範圍限制

- 圖形／顏色／線型／文字屬性與存檔重開為資料核對；中文外觀、箭頭及排版仍需人工驗收。
- 2008 的原生尺寸 Measurement 若回 E_NOTIMPL，會標為未驗證，不捏造量測通過。
- 本輪未驗證一般圖元的目前文件修改／Undo、任意既有 DWG、大型圖塊、Excel 匯入、公司服務與完整 NSIS 安裝／更新／回復／移除生命週期。
- 測試成功自動刪除測試圖檔；失敗圖面經人工定位後清理，保留診斷。原使用者文件不作為修改目標。

## 已完成

- MSVC v142 14.29.30133（_MSC_FULL_VER=192930159 x64）、空 Cargo 快取、vendor／--frozen、fmt、Clippy。
- 63 項選定 Rust 契約、61 項 CAD Python 合成契約通過。
- 正式 EXE 基本／WebView2／唯讀程式碼驗證、Python runtime、Outlook 匿名管線及 NSIS／簽署更新清單核對通過。
- 兩套 CAD 均以 dist/LM_AI.exe 0.8.67 完成六個正式工具流程：inventory、active_read、active_edit（僅 ZOOM E）、draw、read、edit。新建 68 個圖元，副本修改後 69 個；各輪清理 4 個 DWG／備份檔。測試目錄已無 DWG／BAK／DWL／DWL2。
- 本機初次驗證遇到離線 CAD 測試與建置同時執行造成檔案占用；結束實機測試後，原有教學保存測試通過，未修改儲存邏輯。最終建置及正式 EXE 實測依序執行。
- 詳細本機報告路徑與 SHA256 保存在 offline/cad-integration-verification.json；原始報告留本機，不納入公開 Git。

## 產物

| 檔案 | bytes | SHA256 |
|---|---:|---|
| LM_AI.exe | 18779648 | 3822678cc852eef8d570946bfcf2fe03538733e73ec77dbc0583aafb5238e499 |
| LM_AI_Setup.exe | 36144261 | bcbc836848ea1a95644e20c84ab49b7d1757149fc1b06f5b3c6e8cd8074d3faa |
| update-manifest.json | 1050 | 02dcd48df70980456321c59dc8bb1ccde804bd1bb49311030b83cafa98e16536 |
| update-manifest-exe.json | 1050 | 02dcd48df70980456321c59dc8bb1ccde804bd1bb49311030b83cafa98e16536 |
