# 0.8.78：桌寵泡泡、塔羅續問與進階功能

本版將貓／狗基本款換為精細毛流插畫，12張256×256透明PNG每張約109～143KB；原圖集、提示詞與可重製分格腳本保存在assets/pet-illustrations及scripts/Create-PetDefaults.ps1。圖片由內建image_gen產生，保留生成alpha。

本版新增三種可調顏色／透明度的通知泡泡、0～5秒通知時間、桌寵偏好即時保存、可編輯及複製的生成提示詞對話框；78張牌義加長、同副牌不重複續抽與普通聊天延續、進階功能入口及兩項教學。

## 驗證範圍

使用者於初次打包中追加插畫要求，舊素材R1在編譯階段停止；新插畫與輕量設定回覆重新執行R2。2026-11-03 11:04完整流程退出0，記錄RELEASE_0_8_78_BUILD_AND_INSTALLER_PASS。

使用MSVC v142 14.29.30133、實際cl192930159 x64、空Cargo快取及Cargo.lock／vendor／.cargo/config.toml，以frozen完成建置。正式執行Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller，另跑cargo test --lib --frozen及Test-Installer.ps1。

fmt、Clippy、456個Rust函式庫測試、101個CAD合成契約、28項桌寵／塔羅UI、57個互動教學、20個對話框門檻與原生WebView2／來源唯讀核對通過。真正發行Setup的安裝、更新、重新啟動、鎖檔失敗回復、執行環境修復及解除安裝通過。

這是相關基本／原生驗證加完整Rust函式庫與NSIS生命週期，不是整體長流程重新驗收；本版沒有重跑實際Office、SMB、VNC、CAD改圖或真模型對話。基本報告維持PASS_BASIC，額外驗證另列，沒有改標為全面通過。

## 功能與限制

- 泡泡輪廓與角色透明度分開，三種輪廓、透明角落、0～100%透明度、0～5秒及舊設定預設值有原生測試；角色隱藏仍可顯示通知，0秒則不顯示泡泡。
- 快速連續調整合併最新設定，舊回覆不覆蓋新選擇；角色生成提示詞可編輯並複製，PNG匯入限制不變。
- 塔羅保留整副順序及正逆位，續抽拒絕已用位置與過期牌局；78張耗盡需洗牌。每張正逆位解讀約80～140字。
- 續問使用同一普通聊天，保護待送草稿／附件及未完成任務；已刪除聊天則另建，先前牌面仍可附入。牌桌只在當次執行期間保存，退出重開不保留牌局。
- 教學使用假資料，不呼叫模型或改動正式桌寵、牌局與剪貼簿。
- 真實公司服務、桌面通知點擊／拖曳及本版AI續問內容未做端到端驗收；不沿用0.8.77的Luna結果宣稱本版模型通過。

## 交付

- EXE SHA256：`243160e18547b127afe5d16affe7c4f3a968dd1efaa83b60f1d309c561904349`
- Setup SHA256：`21159f57d3fc332cbe68fc8acb8f1f42e8e246be3213153a54323dff1fa82f72`（55,101,857 bytes）
- [建置報告](../../offline/exe-verification-0.8.78.json)、[安裝報告](../../offline/installer-verification-0.8.78.json)、[功能與限制](../../offline/feature-verification-0.8.78.json)。
- [插畫原圖、提示詞與重製方式](../../assets/pet-illustrations/README.md)；12張小圖合計1,479,410 bytes，單張最大142,748 bytes。

Git交付採既有origin/main；推送後另核對遠端commit，從固定commit下載EXE／Setup核對上述雜湊。歷史舊版留存不發布，Git歷史不改寫。
