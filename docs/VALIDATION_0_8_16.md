# 0.8.16 驗收紀錄

## 本機驗證範圍

- Windows x64，Rust 1.98.1；使用 `vcvars64.bat -vcvars_ver=14.2` 的專案環境腳本。
- `VCToolsVersion=14.29.30133`；實際 C 探針 `_MSC_FULL_VER=192930159`、x64；link.exe 14.29。
- `scripts/Build.ps1 -EmptyCargoCache`：全新 Cargo 快取、既有 Cargo.lock/vendor/.cargo/config.toml，`--frozen` 完成格式、Clippy、112 項單元／整合測試、release 編譯。
- WebView2 DOM 自檢：原有登入／對話／附件／彈窗流程，以及專案在 VNC 下／最近對話上、專案對話不出現在最近對話、更新箭頭的顯示／隱藏。
- 真正啟動 release EXE 的 AppContainer：TokenIsAppContainer 握手、外部測試檔案讀／寫拒絕、本機 TCP 連線拒絕。
- 真正 worker 與文件 broker：Unicode 索引修改、版本檢查、操作去重、原檔不變、獨立 TXT 副本、重新讀回、原檔與已發布成果不可刪、越界與硬連結拒絕。
- 本機 HTTP 假後端五輪：讀取 → 建立副本 → 修改 → 儲存 → 交付；每輪 skills=false、無舊用途旗標、quality 模型、真實 AppContainer 與磁碟成果。測試不使用公司 Token 或服務。
- 發行 EXE 與更新清單的 SHA-256、編譯環境及執行時間以 `offline/exe-verification.json`、`offline/environment.txt` 為準。

最初在工具的受限制環境建立 AppContainer 得到 Access denied；改在一般使用者驗收环境執行後，發現精簡環境少了 Windows 必要的目錄變數。已以明確目錄變數白名單修正，保留禁止繼承自訂憑證與不降級的規則。修正後實際隔離與文字工具測試通過。

## 本次未聲稱通過的項目

- 公司加密軟體的明文讀取、寫入攔截及 TXT 副本仍受保護的實際行為。
- 自動操作記事本／Notepad++：本版沒有此功能，提供使用者明確匯入文字快照的方式。
- 公司 GLM／Gemma 對工具 JSON 的遵循程度，以及公司後端 skills 路由的部署。
- 真實 Outlook 信箱、品質模型整條流程；程式改為勾選自動補充時初篩即 quality，需公司實機確認。
- NSIS 自動更新的實際安裝／重啟：依使用者指示，本次不製作、不執行安裝包。只驗證原生分支、簽章與選包規則；同意綁定指定版本，一次同意後接續既有安裝交接。
- 新版畫面的人工截圖視覺驗收；本機完成的是實際 WebView2 DOM 自檢。

## 公司測試建議

1. 建立含測試 TXT 的獨立資料夾，新增專案對話，要求閱讀。確認實際內容正確，不能只確認沒有錯誤訊息。
2. 若直接讀取失败，以記事本開啟後匯入全文；請 AI 修改一段並另存。確認原檔不變、新 TXT 內容正確且受公司加密保護。
3. 混合中文、英文、換行與非 BIG5 字元，確認不被替換成問號。明文快照輸出使用 UTF-8 BOM。
4. 停止任務、切換一般對話，確認一般對話不能使用專案工具；再次任務建立新子程序。
5. Outlook 勾選自動補充時，在後端確認初篩與最終整理均 quality；未勾選分別試 fast／quality。
6. 在有／無新版的情況確認齒輪旁箭頭。當伺服器只提供新版 EXE 時應提示手動更換，不能假裝已自動安裝。

本次交付 `dist/LM_AI.exe`、`dist/update-manifest-exe.json`、原始碼及文件。`LM_AI_Setup.exe` 與 NSIS 更新清單保持 0.8.15；歷史離線 ZIP 不更新。Git 推送不代表內網服務已部署。
