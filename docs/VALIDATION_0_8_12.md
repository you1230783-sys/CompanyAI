# 0.8.12 統一完成結果驗收

使用者確認網站背景模式原先未完整回傳資料，並指定背景與串流最終 result 統一使用 Chat Completions 物件。0.8.12 新增 `choices[0].message.content` 正文搭配同層 `sections`、`citations` 的解析，不再要求該物件一定包含 answer。使用者範例保存於 `ui/fixtures/completion-reply.json`，由 Rust 與 WebView2 共用。

既有 response_payload_json 優先規則、answer payload、舊格式及串流原文保留不變。新格式正規化為既有顯示 DTO；正文與重點直接顯示，來源、信心、限制與引用可展開，完整文字用於複製及歷史保存。任務外層 task_id、client_request_id、state 仍須符合原契約，result.id 不取代任務關聯驗證。

## 驗證範圍

- 指定 MSVC v142 x64，以 vcvars64 的 `-vcvars_ver=14.2` 初始化，實際探針驗證 `_MSC_FULL_VER=192930159`；VCToolsVersion 14.29.30133，linker 為同版 Hostx64/x64。
- Rust 1.98.1、Windows SDK 10.0.19041.0；fmt、Clippy 與 93 項 Rust 測試。Cargo.lock、vendor、.cargo/config.toml 搭配空 Cargo 快取執行 `cargo build --workspace --release --frozen`。
- 協定回歸：使用者原樣結果與 result 包裝、非空字串／物件引用、只有 citations 的結果、明確 payload 非空欄位優先及空欄位補齊、無效正文拒絕、內部工具資料不進顯示 DTO；標題及 Outlook 專用解析仍只回傳正文。
- 背景／串流兩模式經 TaskStatus 及 apply_reply 完成、去重，寫入 DPAPI 加密歷史再讀取，逐欄比較 sections、citations。串流僅提供正文，背景沒有部分文字，確保完整欄位直接來自完成 result。
- WebView2 自檢：舊 answer 格式與新的 result 格式均由 Rust 解析後透過訊息橋交給前端；兩模式從任務切換完成訊息、歷史序列化、重點可見、其他欄位可點擊展開且具有可見高度、複製保留完整文字。
- NSIS 安裝／更新／PID 交接／重啟、鎖定檔案失敗時保留舊程式、VNC 設定與未知檔案保留、解除安裝、正式 Setup 安裝後 WebView2 自檢及子程序限定缺少 Runtime 模擬。
- EXE／NSIS 更新清單簽署後依內建公鑰驗證版本、平台、大小、SHA256 及簽章；JavaScript 語法與 Git 差異檢查。

2026-09-23 執行 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`，上述完整發行驗證全部通過。本機日誌為 `.build/release-0.8.12.log`，機器記錄為 `offline/exe-verification.json`、`offline/installer-verification.json` 及 `offline/environment.txt`。

## 成品

| 檔案 | 大小（bytes） | SHA256 |
|---|---:|---|
| dist/LM_AI.exe | 4,912,640 | `499bfe3e8deac2db9dc6495b9a7694d58cc98fe541ab5dd812e8fd0e35673f55` |
| dist/LM_AI_Setup.exe | 1,975,130 | `cd2f30c2b08bfde1a8bfa5f608ac2851ca9bc2fa6708531236071e3477580057` |

兩者檔案版本均為 0.8.12.0；對應 `dist/update-manifest-exe.json` 及 `dist/update-manifest.json` 的 version 均為 0.8.12。完成後另核對實際檔案版本、大小及 SHA256 與清單一致。二進位檔以 Git LFS 保存。

## 界線

本機測試使用使用者提供的 JSON 與模擬任務狀態，未連線公司服務、取得實際完成封包或拍攝介面截圖。公司仍須用更新後的網站實測；Git 推送不等同內網部署。未新增實際 Outlook／VNC 操作驗收。

沿用三花貓圖示及單純 NSIS 安裝，不重製歷史離線 ZIP。EXE／NSIS 須與各自更新清單成對部署；網站可公告 latest_version 0.8.12，不需提高最低版本。既有只存正文的歷史不自動重新查詢。
