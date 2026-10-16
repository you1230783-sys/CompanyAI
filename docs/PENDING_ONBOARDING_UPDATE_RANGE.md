# 本版更新日誌與教學

本批修改納入 0.8.68；發行建置與驗證狀態以 [VALIDATION_0_8_68.md](VALIDATION_0_8_68.md) 為準。
下列未驗證項目描述的是進版前狀態。

2026-10-16 要求取代先前的跨版本起點修正。後續為使用者授權的 localhost Luna 實測建立了
隔離測試 EXE；仍不進版、不封裝、不推送。

## 最終行為

- 設定中的「重看更新日誌」開啟對話框，只列目前版本的使用者相關公告。
- 移除設定下方的重複日誌；取消更新後自動提示及「從某版本更新到某版本」。
- assets/release-notes.json 只保留目前發行版本；每次發行直接替換成本版內容，歷史開發紀錄留在 CHANGELOG，不嵌入應用程式。
- 移除原生版本追蹤、更新前版本保存與 NSIS update-transition.ini 寫入。既有 INI 不再讀取。
- 保留 14 張教學、原本自動教學與個別重看，以及既有 ID／revision／完成紀錄。
- 舊 onboarding-state.json 可正常讀取，忽略 last_version／pending_from，保存時只保留教學紀錄。

## 驗證

- node tools/test_onboarding.cjs 通過：教學切換、重看、返回設定、登入限制；公告不自動彈出、手動開啟只顯示目前版本且不送已讀指令。
- git diff --check 通過。
- Rust 回歸案例更新為舊資料相容與本版公告檢查，隨 localhost 基礎建置通過。
- 設定頁瀏覽器測試契約已更新但未執行；WebView2、NSIS 安裝及正式更新流程未實測。
- target 下的測試 EXE 包含本批修改；dist／安裝包未更新，不能宣稱完整驗收或已發行。

## CAD 問題分析範圍

使用者回報第三項控制盒需求常逾時、第一項法蘭測試正常。本輪只檢視程式處理，
沒有修改 CAD 或模型參數，也未對公司服務重送測試。
runner 已區分空白 finish_reason=length 與其他失敗；不能由逾時現象判定輸出額度耗盡。
需要失敗當次的請求狀態、耗時、最後工具步驟、finish_reason 與 usage 才能進一步定位。
