# 0.8.65 基礎驗證

狀態：PASS_BASIC，2026-10-15。EXE／NSIS 0.8.65.0；完整驗證與公司實測待補。

變更見[技能整理](SKILLS_0_8_65.md)、[跨任務內容](PENDING_PROJECT_CONTEXT.md)、[初階教學](PENDING_ONBOARDING.md)、[CAD連線](PENDING_CAD_CONNECTION.md)、[程式碼唯讀](PENDING_CODE_SEARCH.md)。
第三項CAD公司測試連續超時三次後由使用者停止，不列為成功。新教學可由設定→一般→使用教學重看；EXE旁旗標可個別重設chat.basics，重看不需刪除旗標。
本機不執行CAD繪圖；Python合成測試不代替2008／2022實機驗收。

## 已完成

- MSVC v142 14.29.30133，實際_MSC_FULL_VER=192930159 x64、SDK10.0.19041.0、Rust1.98.1。
- 空Cargo快取、Cargo.lock／vendor／.cargo/config.toml、--frozen、fmt、Clippy全target與63項選定Rust契約通過。
- 57項CAD Python合成契約通過，包含同worker保留連線、不自動啟動CAD、引線文字左下錨點。
- 教學Rust旗標／版本範圍、Node模擬DOM的登入門檻／單卡／重看／登出／更新摘要通過；尚未做人眼版面驗收。
- 正式EXE的WebView2／原生任務／唯讀程式碼基本檢查、CAD及Python runtime、Outlook匿名管線／EOF通過；未讀信箱或操作CAD。
- NSIS包裝、簽署更新清單與原生更新套件核對通過；未做完整安裝／更新／回復／移除生命週期。
- 初次受限環境Python暫存目錄拒絕存取；改用正常桌面權限後完整重跑相同基礎腳本通過，沒有略過失敗項目。
- 紀錄：.build/build-0.8.65-release.log、offline/exe-verification.json、offline/cad-integration-verification.json。

## 產物

| 檔案 | bytes | SHA256 |
|---|---:|---|
| LM_AI.exe | 18630656 | 58f9b2f15c258613b2179394808a8deb6f92ad3748b68b7d36bd43cfbc9c39dd |
| LM_AI_Setup.exe | 36105383 | 1d56556a9a88a6dfe8ff5b2dbf9753a21423f1b4bae8118a43d861fb9b336aa4 |
| update-manifest.json | 1050 | 664b534fb60020140c5eb9ebe925bc2113e8f9769933e7841a173b6cd3402914 |
| update-manifest-exe.json | 1050 | 664b534fb60020140c5eb9ebe925bc2113e8f9769933e7841a173b6cd3402914 |
