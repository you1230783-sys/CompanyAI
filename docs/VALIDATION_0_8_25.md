# 0.8.25 驗證紀錄

2026-09-30 執行 `scripts/Build.ps1 -EmptyCargoCache` 成功。

- MSVC v142 14.29.30133，實際 `_MSC_FULL_VER=192930159` x64；Rust 1.98.1、Windows SDK 10.0.19041.0，靜態 CRT。
- Cargo.lock、vendor、.cargo/config.toml 齊備，依賴未變；使用全新空 Cargo 快取完成 fmt、Clippy `-D warnings`、158 項測試及 `cargo build --workspace --release --frozen`。
- WebView2 DOM self-check、真正 AppContainer 檔案／網路隔離與工具整合通過。
- EXE：8,901,120 bytes；SHA256 `df7b1222b370330dcab6556305ff72949f4dbd324365c24dfe7bb4904502763c`，檔案版本 0.8.25.0。EXE、簽署清單及機器驗證紀錄一致；發行腳本已使用 EXE 內建公鑰驗證清單簽章及檔案。

## 本次驗證

- 新增八項文字工具協定測試，以及先前待編譯的三項缺漏工具欄位回歸；全數通過。
- 驗證字串與物件 arguments、引號／反斜線／換行／Unicode、原 ID 與雙版本保留、筆記欄位、finish／ask_user、空白完成、混合格式、多呼叫拒絕、未知工具及缺漏參數。
- 工具目錄涵蓋 18 項既有操作及兩項交付／詢問工具；HTTP 保持 messages 文字與 skills:false，不新增原生 tools 欄位。
- 真實 loopback HTTP 跑完八個情境：舊格式正常與包裝說明、有限停止、舊詢問、新字串參數修訂交付、新物件參數修訂交付、新詢問，以及多呼叫拒絕後修復。ID 與結果對應，修改／儲存各執行一次，原件不變。
- 長文件 23 段、138,000 字無筆記正常完成 24 輪；含自願筆記及兩次壞回覆的續接完成 26 輪。去重、取消、錯誤身分、未知提交及無進展上限回歸通過。
- .lmai 邊界、加密筆記 CRUD／復原、文件摘要、PDF 本機解析、伺服器 multipart、跨任務快取与來源變更失效回歸通過。

第一次在受限執行環境中，WebView2 啟動檢查回報 0x8000FFFF；改在允許 Windows 子程序正常啟動的環境完整重跑後通過，未略過自檢或修改應用程式權限。此過程不代表公司電腦的部署環境已驗證。

機器紀錄：`offline/exe-verification.json`、`offline/environment.txt`。完整 system 靜態範例見 [PROJECT_REQUEST_0_8_25_EXAMPLE.json](PROJECT_REQUEST_0_8_25_EXAMPLE.json)。

未連線公司 GLM／Gemma；以上使用確定性測試回覆，不宣稱模型成功率已改善。Office／MSG、VNC 方法未改，本輪未重跑真實 Office／VNC；沒有外觀截圖，不宣稱視覺驗收。

只交付 EXE、EXE 簽署清單、原始碼及文件；不重製 NSIS／離線 ZIP。Git 推送不代表公司內網部署完成。
