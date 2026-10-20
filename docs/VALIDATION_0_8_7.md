# 0.8.7 EXE 驗收

## 範圍

精簡側欄、最近對話操作與右上通知／任務入口，品牌使用與 EXE 相同來源的黑貓 ICO。對話操作以各列 ID 執行，避免修改非目前對話時誤操作目前對話；刪除仍需確認，鍵盤聚焦可顯示操作圖示，側欄收合時仍能新增對話。

更新使用者指定的 Outlook 說明，完整內容匯出授權分行並以紅色呈現，支援深色模式。每次進入 Outlook 助理重查模型清單；沒有 quality 時禁用自動補充，顯示維護提示。查詢失敗與未登入使用不同提示；恢復時不自動勾選。基本資訊分析不依賴 quality，既有 MSG 範圍及最多 20 封限制不變。

## 驗證

使用 `scripts/Build.ps1 -EmptyCargoCache`，以 v142 x64 編譯器探針、fmt、Clippy、Rust 測試、空 Cargo 快取 `cargo build --workspace --release --frozen`、WebView2 DOM 自檢與 EXE 更新清單簽章／SHA256 驗證。依賴維持 Cargo.lock、vendor、.cargo/config.toml，未新增套件。

WebView2 自檢包含黑貓資源載入、導覽位置、對話列目標 ID／不切換對話、鍵盤操作、收合側欄新增對話，以及品質模型查詢／停用／恢復／失敗、基本資訊分析、授權紅字段落、確認期間模型被撤回後禁止自動匯出。

2026-09-22 本機驗證通過：MSVC 14.29.30133、實際 `_MSC_FULL_VER=192930159` x64，fmt、Clippy、81 項 Rust 測試、空快取 frozen release、WebView2 自檢及更新清單驗證皆成功。初次介面自檢因前一案例收合側欄、後一案例未展開即嘗試鍵盤聚焦而失敗；補齊獨立測試的前置狀態後重跑完整 Build 成功。

成品版本 `0.8.7.0`，大小 `4,657,664` bytes，SHA256：`f2dff675832d5b662fa51262aa279a0b995156743c11fa53670b7ca712294b18`。本次未取得外觀截圖：隱藏啟動的本機 demo 未提供可擷取視窗，瀏覽器工具未提供可用瀏覽器；不宣稱截圖驗收通過。

## 交付界線

本輪先提供 `dist/LM_AI.exe`、`dist/update-manifest-exe.json`、原始碼與本版驗證文件，NSIS 保留 0.8.5，離線 ZIP 不更新。`offline/vnc-verification.json` 是 0.8.6 的歷史 Viewer 測試，不能視為本版重測；使用者已回報 VNC 正常，本版未改 VNC 呼叫流程。

真實公司模型停用／恢復、Outlook 實際信箱與最終外觀仍由使用者驗收。Git 推送不代表內網已部署。
