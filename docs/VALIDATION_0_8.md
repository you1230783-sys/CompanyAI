# 0.8 驗證範圍

交付以 `offline/verification.json`、`offline/environment.txt` 與 `offline/installer-verification.json` 的實際結果為準。`Prepare-Delivery.ps1` 必須在全部檢查成功後才更新離線 ZIP。

## 本機自動驗證

- 實際 MSVC v142 x64 編譯探針，Rust fmt、Clippy、單元／本機 HTTP 測試及 release build。
- 更新 CNG 簽章測試：正確簽章通過，內容或簽章修改後拒絕；不信任 manifest 提供的新金鑰。
- 更新中繼資料：跨來源、降版、不支援平台／包裝、錯誤長度／雜湊／簽章不能啟動安裝。
- 本機 HTTP：二進位逐段下載、長度上限／截斷、HTML、轉址拒絕，不傳登入憑證。
- 強制門檻保存後重載，以及後續伺服器降低門檻仍保持鎖定。
- 真正 WebView2 DOM 自我檢查：強制更新對話框、Escape 無法解除、下載期間禁用重複操作、下載完成切換第二步安裝按鈕。
- NSIS 使用同一份正式安裝原始碼與隔離的 v142 原生測試 payload：安裝／捷徑／解除安裝登錄、等待舊 PID 退出、檔案替換、新版自動啟動、占用檔案造成失敗時保留舊版、解除安裝保留未知檔案。
- 發行機生成的實際清單與完整 Setup，交由本次 Rust 主程式 `--verify-update-package` 驗證簽章、大小與 SHA256；不啟動安裝。
- ZIP 解壓到新目錄，使用包內固定 Rust 工具鏈、空 Cargo 快取及 `--frozen` 再次完成 Build 與 NSIS 安裝測試；NSIS 工具本身隨包提供且核對固定 SHA256。

NSIS 測試寫入 `%LOCALAPPDATA%\Programs\LM_AI_Installer_Test`、對應測試捷徑／登錄；不覆蓋正式 LM_AI 安裝或資料。失敗保留測試現場供診斷，正常完成移除本次測試產物。需要允許目前使用者目錄及登錄寫入。

## 公司仍需驗收

1. 部署 [新版契約](UPDATE_0_8_CONTRACT.md)，先手動安裝 0.8 作為原生更新起點。
2. 建立較高版本的正式發行包與對應簽章，確認支援中的舊版不會自行下載；第一次取消無下载，第二次取消不退出，退出亦不套用。
3. 接受兩次確認，驗證公司網路下載、完整 Setup 交接、WebView2、重啟與資料保留。
4. 提高最低版本，確認聊天／Outlook／選字等無法操作；取消、斷線及重開仍鎖定，新版達標才恢復。
5. 分別測試公司防毒對正式主程式、Setup、Microsoft Runtime 與原生 Windows API 的政策；本機測試不能取代這項驗收。

這次沿用已確認可用的 NSIS 方法，但不把小工具通過防毒視為整套 LM_AI 已在公司通過。未實際連接公司新版更新端點，也沒有宣稱已完成公司網路上的跨版本端到端升級。
