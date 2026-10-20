# 0.8.11 完成回覆保留與發行驗收

使用者回報 0.8.10 完成後仍只剩正文，並提供五段英文 Markdown 全文。這個樣本已加入 `ui/fixtures/inline-reply.txt`。本機確認兩個缺口：只有 answer 的結構化結果會優先於較完整 content，且同列標題／內容與單行五段格式不能正確收合。完成 REST 結果也會取代串流文字。

0.8.11 保留結構化非空欄位優先；正文一致時從較完整結果或同任務串流補齊空欄位。正文不同、欄位衝突或無法解析時，把收到的原文另存於可展開區塊。正文及重點直接可見，來源、信心、限制與引用可點擊展開；複製及加密歷史保存完整內容。REST／串流重複提供的原文只保留一份。詳細規則見 [回覆契約](STRUCTURED_REPLY_CONTRACT.md)。

## 驗證內容

- 指定 MSVC v142 x64，初始化 `vcvars64.bat -vcvars_ver=14.2`，實際編譯探針 `_MSC_FULL_VER=192930159`，VCToolsVersion 14.29.30133，linker 同版 Hostx64/x64。
- Rust 1.98.1、Windows SDK 10.0.19041.0；fmt、Clippy 及 90 項 Rust 測試。依現有 Cargo.lock、vendor、.cargo/config.toml，使用空 Cargo 快取執行 `cargo build --workspace --release --frozen`。
- Rust 回歸：使用者單行與分行全文、只有 answer 的 payload、result 較完整、answer 內含五段全文、最終明確信心不被舊值覆寫、正文改變的原文保留、重複原文去重、程式碼／引用／不完整格式不冒充完整欄位。
- 背景與串流兩模式經正式 TaskStatus／apply_reply，去重後寫入 DPAPI 歷史，再讀取確認正文、兩項重點、100% 信心、來源及限制完整保留。
- WebView2 自檢以 Rust 解析的樣本經原生訊息橋送入：串流切換只有正文的完成結果、正文不同的原文保存、點擊展開後區塊實際有可見高度、複製含所有章節、歷史序列化與訊息重繪保留展開狀態。
- NSIS 實際安裝、更新、PID 交接／重啟、鎖定檔案失敗時保留舊 EXE、未知檔案及 VNC 設定保留、解除安裝、正式 Setup 安裝後 WebView2 自檢。模擬缺少 Runtime 時，安裝可完成，主程式啟動才提供既有提示。
- EXE／NSIS 更新清單簽署後以內建公鑰驗證版本、平台、大小、SHA256 及簽章；另做 JavaScript 語法與 Git 差異檢查。

首次可見高度斷言發現自檢仍停在前一組 Outlook 頁面，已在對話測試前切回聊天，保留原斷言。2026-09-23 最終版本重新完整執行 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`，上述檢查全部通過。本機日誌為 `.build/release-0.8.11.log`；機器記錄為 `offline/exe-verification.json`、`offline/installer-verification.json` 及 `offline/environment.txt`。

## 成品

| 檔案 | 大小（bytes） | SHA256 |
|---|---:|---|
| dist/LM_AI.exe | 4,905,472 | `07fd4891281881df722ba1b34728065034d0ee8ec8afcd52bc557ea61d3582a9` |
| dist/LM_AI_Setup.exe | 1,974,169 | `4bf382bfb02eb7ecac3246a447638d264c7d6f0a35c21b360b9f0a9347975011` |

兩者檔案版本均為 0.8.11.0；對應清單為 `dist/update-manifest-exe.json` 及 `dist/update-manifest.json`，version 均為 0.8.11。二進位檔以 Git LFS 保存，與對應原始碼及驗證文件一起交付。

## 驗收界線

本機未連線公司快速模型或取得真實 SSE／REST 封包，也未拍攝介面截圖；上述測試驗證已提供樣本與明確缺漏情境，不代表公司實機已驗收。若伺服器從未傳出其他欄位，桌面無法自行產生。既有歷史只存正文且原文已不存在的訊息，不會自動重新查詢或還原。

沿用三花貓圖示與既有單純安裝方式；不重製歷史離線 ZIP、不恢復 SFX。Git 推送不等於內網網站已更新檔案與版本公告。EXE／NSIS 均須和各自更新清單成對部署；網站可公告 latest_version 0.8.11，不需提高 minimum_version。
