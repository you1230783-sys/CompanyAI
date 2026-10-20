# 0.8.60 iCAD 尺寸樣式修正驗證

狀態：PASS_BASIC，2026-10-15。EXE／NSIS 0.8.60.0；完整驗證及公司 CAD 實機待補。

## 修改

- 佑達 iCAD 原生 DimensionStyles.Add 與直接樣式屬性，避免缺少的 DimStyles／CopyFrom。尺寸字高3、箭頭3、偏移1、ISO與文字樣式設定立即讀回及重開核對。
- 一般文字樣式與尺寸樣式分開，文字操作不依賴尺寸集合。
- 樣式前置準備早於幾何繪製／既有圖元刪改；目前圖面的前置寫入保留於Undo群組內。
- 保留原生尺寸、引線；維持文件身分、未知額外圖元、未儲存成果與不重播限制。
- README 開頭最多保留三版；技能提醒核對距邊座標，不將25mm誤述為15mm。

## 驗證界線

已完成：MSVC v142 x64實際探針、空Cargo快取／--frozen、fmt／Clippy、選定Rust契約、Python合成契約、原生基本檢查、NSIS及更新清單。

本機沒有佑達iCAD 2008／公司GstarCAD 2022實機驗收；新版文件中的原生介面仍需在公司版本測試。未知額外圖元的來源仍未確認，完整安裝生命週期、Office／SMB與完整CAD流程驗證待補。

## 最終交付

- MSVC 14.29.30133，實際 _MSC_FULL_VER=192930159 x64；SDK 10.0.19041.0；Rust 1.98.1。
- Cargo.lock／vendor／.cargo/config.toml、空 Cargo 快取、--frozen、fmt／Clippy 與 49 項選定 Rust 契約通過。
- 35 項 Python 合成契約通過；新增原生DimensionStyles與嚴格屬性替身、尺寸樣式讀回不符拒絕、一般文字不依賴尺寸集合、前置失敗保留既有圖元並結束Undo群組。
- 原生 WebView2 基礎檢查、正式及 dist CAD runtime／唯讀登錄管線、Python runtime、Outlook 無效要求管線（未讀信箱）、任務追蹤／程式碼唯讀基本回歸通過。
- NSIS 包裝、簽署更新清單及原生產物核對通過；未執行安裝生命週期完整測試。
- 紀錄：.build/build-0.8.60-release.log；offline/exe-verification.json、offline/cad-integration-verification.json。

| 產物 | bytes | SHA256 |
|---|---:|---|
| LM_AI.exe | 18599936 | a8dead1347d0e0aadcf7d51f7cd56dee5a0aae9ed76709778704ba331a616390 |
| LM_AI_Setup.exe | 36102347 | fd15df5bc123e5f28c9eb5bc162fa79535038f7dfde890adc8037a496f407d4f |
| update-manifest.json | 1050 | 216fb8fe5f0c6305d04932323fee7f86cbd1ce731d45223a2d0b4fb19be1eaea |
| update-manifest-exe.json | 1050 | 216fb8fe5f0c6305d04932323fee7f86cbd1ce731d45223a2d0b4fb19be1eaea |
