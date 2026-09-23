# 結構化回覆與章節收合修正：本機驗證

2026-09-23，依使用者提供的背景回答 JSON，修正完成結果只讀 choices 正文、漏掉結構化欄位的問題。`src/protocol/reply.rs` 優先讀取 response_payload_json 或直接 payload，保存正文、重點、來源、信心、限制與引用；背景與串流完成後共用同一解析。前端直接依欄位呈現，不再猜測結構化正文內的標題。

同時修正舊格式從 Sources 起將後面所有內容收合的行為，只接受編號章節，標籤完整對齊使用者提供的中英文清單。正文／重點留在可見區；來源、信心、限制及引用放入可展開區。舊格式原文不改寫；結構化結果另產生完整 Markdown 供複製及後續聊天使用。詳見 [結構化回覆契約](STRUCTURED_REPLY_CONTRACT.md)。

## 已執行

功能開發先以 `Build.ps1 -EmptyCargoCache -ValidateOnly` 驗證；使用者要求打包後進版 0.8.10，再以 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 全部通過：

- vcvars64 的 `-vcvars_ver=14.2`，MSVC 14.29.30133；實際 C 編譯探針 `_MSC_FULL_VER=192930159`、x64，linker 指定同版 Hostx64/x64。
- Rust 1.98.1、Windows SDK 10.0.19041.0；fmt、Clippy、85 項 Rust 測試。
- 空 Cargo 快取、現有 Cargo.lock／vendor／.cargo/config.toml 的 `cargo build --workspace --release --frozen`。
- Rust 新增協定與保存測試：直接 payload、result 包裝、物件／JSON 字串 response_payload_json、message payload、content 內完整 JSON、明確欄位優先於舊正文；異常格式備援、空欄位、引用保留、非顯示欄位不傳入 UI。兩種執行模式都驗證任務套用、串流部分結果取代、去重與 DPAPI 加密歷史存檔後重新讀取。
- WebView2 新增結構化端到端自檢：使用者範例由 Rust 解析後經正式訊息橋傳入，確認正文及兩項重點直接顯示、來源與限制可展開、sections.confidence 的 `High (高)` 優先於外層 `medium`。背景／串流完成切換與複製按鈕均保留完整結果；另測空章節、引用物件唯讀呈現及 Markdown 安全渲染。
- WebView2 DOM 自檢：九種編號制式回覆（清單、標題、粗體、中英文、大小寫／空白／冒號、章節重排、重複正文／重點）。另逐一驗證網頁端提供的 21 個標籤與冒號後同列內容，來源／信心／限制仍收合，正文／重點不在收合區內。
- 程式碼、引用、巢狀清單及非制式內容不誤收合；來源內子標題保留，同級未知標題之後的補充正文可見。
- 經正式 UI 接收入口，串流切換完成訊息後渲染一致；摘要可點擊展開，下一次訊息重繪保留展開狀態，原始 content 完整不變。
- `node --check` 檢查修改的 JavaScript 檔案，`git diff --check` 通過。

先前在受限環境啟動 WebView2 得到 `0x8000FFFF`，因此驗證使用正常本機權限。此次整合期間修正編譯檢查的測試型別／未使用匯入錯誤；既有快捷鍵自檢兩次在固定兩幀等待後誤報，改為等待 Rust 實際回覆（五秒上限，保留原斷言）。新增樣本原先以 fetch 載入，被既有 `connect-src 'none'` 正確擋下，改由原生自檢訊息橋提供，不放寬 CSP。最後完整建置命令全部通過，未略過測試。功能建置記錄在本機 `.build/reply-sections-validation.log`，0.8.10 發行記錄為 `.build/release-0.8.10.log`。

## 測試產物與界線

- 發行 EXE：`dist/LM_AI.exe`，檔案版本 0.8.10.0。
- SHA256：`01c70ccbf2de86f3fcfcabade9b5516b1a084fd596d25c0d3ff144913e969d2c`。
- 使用者已要求打包推送，交付 EXE、NSIS、兩份簽署更新清單與原始碼文件；NSIS 驗證及成品資訊見 [0.8.10 驗收](VALIDATION_0_8_10.md)。歷史離線 ZIP 不重製。
- 使用者提供的背景 JSON 已作為共用測試樣本（識別碼改為虛構值）。未連線公司快速模型，未取得實際 SSE 封包，也沒有做畫面截圖驗收；本機兩模式完成結果驗證不代表公司服務的串流實機驗收。
- 串流期間仍依既有 delta 顯示文字，完成後取 REST 結構化結果；不猜測未完整傳回的 JSON。舊歷史正文不自動重新查詢或補寫，新的完整結果會隨歷史保存。
