# 0.8.22 驗證紀錄

2026-09-30 執行 `scripts/Build.ps1 -EmptyCargoCache` 成功，只交付 EXE 與簽署清單，不製作 NSIS／ZIP。

- MSVC v142 14.29.30133；實際探針 `_MSC_FULL_VER=192930159` x64。Rust 1.98.1、Windows SDK 10.0.19041.0，靜態 CRT。
- Cargo.lock、vendor、.cargo/config.toml 齊備，未增加依賴。全新空 Cargo 快取、fmt、Clippy `-D warnings`、135 項測試及 `cargo build --release --frozen` 全部通過。
- WebView2 DOM 自檢、真正 AppContainer 的外部檔案／網路拒絕、Unicode 編輯、原件保護、重播去重、成果讀回及越界拒絕通過。
- 既有四組 HTTP 工具往返回歸通過，含工具說明、截斷 JSON 的兩次修復及說明＋ask_user。
- 新增八組真實 loopback HTTP 代理測試，均啟動真正 worker 並保持原件不變：
  - 23 段／138,000 字的來源，正常閱讀 24 次模型請求（23 次工具＋完成），成功跨過原 20 輪限制。
  - 同一來源中途注入裸 done、server completed 卻無 result；兩次修復後完成，總共 26 次請求。第二次續接保留累積筆記、兩段已摘要及全部未摘要原文，原始需求與補充始終存在。
  - 修改後注入兩份 save_copy JSON，再注入空 result；兩份候選均未執行。重播原 edit operation_id 後只有一次插入，接著僅儲存一份檔案；空 finish 再修復後正常交付，共九次請求。
  - 連續裸 done 在原回覆＋兩次修復後停止，共三次請求。
  - POST 回傳不同 request ID 立即停止，一次請求，不執行工具。
  - server cancelled 停止，一次請求，不當成模型格式錯誤重送。
  - POST 503、原請求 GET 404，保留結果未知狀態，一次 POST，不換 ID 重送。
  - 重複同一 list_files，首個結果後連續八次無進展便停止，共九次請求。
- 單元測試涵蓋讀取區間聯集／缺口、版本改變撤銷筆記、三段新閱讀／六次有效操作門檻、頻繁筆記忽略、六次全任務修復上限、不靜默捨棄未摘要證據、一般聊天與專案訊息上限分開。
- 原生 PDF 診斷、2 MB 管線、PDF multipart broker／記憶體快取／來源失效／TXT 修改與儲存回歸通過。
- EXE：8,320,000 bytes；SHA256 `d173dbc995d1edefb63370dbbd88e72615f0a356ea60db881109f9d5eeac6acb`。簽署清單與原生公鑰驗證一致。

機器建置紀錄：`offline/exe-verification.json`、`offline/environment.txt`。測試執行器在受限開發環境無法建立 AppContainer，改在允許建立 AppContainer 的環境完成上述正式測試；應用程式沒有加入降低隔離的備援。

## 公司測試與範圍

使用者已回報 0.8.21 專案 PDF 可以閱讀，但真實模型偶有錯誤 JSON／空白完成。本輪使用確定性的模型回覆模擬來核對桌面續接行為，沒有連線公司 GLM／Gemma，不能據此宣稱實際摘要品質或成功率已改善多少。

建議重跑先前失敗的論文摘要，觀察「已更新任務筆記」「正在修復模型回覆」「正在依筆記與進度接續任務」及最終摘要；若仍失敗，保留操作歷程及當時回覆以供調整。模型筆記可能遺漏重點，重要結論仍應回原文核對。

Office／MSG 實作未變更，本輪未重新執行真實 Office。未做新 UI 外觀截圖，不宣稱視覺驗收。NSIS 仍為 0.8.15，既有 Office／文件驗證檔保留其歷史版本；Git 推送不代表公司內網已部署。
