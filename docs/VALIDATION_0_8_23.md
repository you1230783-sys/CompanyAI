# 0.8.23 驗證紀錄

2026-09-30 執行 `scripts/Build.ps1 -EmptyCargoCache` 成功。

- MSVC v142 14.29.30133；實際 `_MSC_FULL_VER=192930159`、x64。Rust 1.98.1、Windows SDK 10.0.19041.0，靜態 CRT。
- Cargo.lock、vendor、.cargo/config.toml 齊備，依賴未變；全新空 Cargo 快取的 fmt、Clippy `-D warnings`、143 項測試、`cargo build --release --frozen`、WebView2 DOM 與 AppContainer 整合全部通過。
- EXE：8,854,528 bytes；SHA256 `59505fb4b8fc61d3c7f8f370cefa66ccba80ced026d386099456042487db2dd4`。本輪 Build 已產生並以原生公鑰驗證 EXE 簽署清單。
- 詳見 `offline/exe-verification.json` 與 `offline/environment.txt`。

## 已驗證的行為

- 143 項 Rust 測試：原有功能回歸、筆記 DPAPI 保存／重開／版本衝突／跨對話隔離／軟刪除／復原；來源變更與過期摘要拒絕；未讀區段不能生成已讀摘要；Unicode 分段與非法索引；長答案不直接送出但可分頁讀回；重試不附後來答案；摘要預算；快取來源／設定／MD 雜湊驗證與容量限制。
- `.lmai` 大小寫、巢狀路徑、根目錄選擇、私有檔案硬連結拒絕；加密 bytes 不含測試明文，原始文件保持不變。
- 真正 AppContainer／固定 Broker：列目錄不含 `.lmai`、直接存取被拒、筆記 CRUD／操作去重／復原、新 Broker 能讀到文件摘要且只讀指定 4000 字段落。
- 真正 loopback HTTP：模型 finish 附 task_summary，保存後下次上下文只有摘要，完整最終回答可經工具讀回。既有四組工具往返及八組有限續接情境均回歸。
- PDF multipart fixture：同任務重用、來源改變重轉、新 Broker 重用磁碟快取；快取損壞後重新轉換，原檔不改，成果仍為 TXT。另有既有原生 PDF 診斷與 2 MB worker 傳輸回歸。
- 通知請求／任務 ID 可跨重開識別，不隱藏無關通知；本機最終通知標记已讀不提交網站 ID。
- WebView2 DOM：內部進度不跳 toast、只有一張專案執行卡、停止按鈕送往專案 runner，並保留既有重試、展開歷程等檢查。

## 測試限制

確定性 HTTP 回覆模擬用來驗證桌面工具與上下文，不代表已測公司 GLM／Gemma 的摘要品質與成功率。沒有連線公司內網、公司加密系統或實測網站 resource_id；通知若未提供精確關聯仍會保留。未新增 UI 外觀截圖；Office／MSG 讀取方法未變，本輪未重跑真實 Office。

建議公司測試：讀論文並摘要，結束後在同對話追問指定段落；重開 App 再讀相同 PDF 應命中快取；修改來源後应重新轉換。觀察一般檔案清單是否排除 `.lmai`，以及每次使用者要求只收到最終通知。PDF 快取是 Windows 帳號 DPAPI 加密資料，不是可直接雙擊的 MD。

只交付 EXE 與其簽署清單，NSIS 保留 0.8.15、歷史 ZIP 不重製。Git 推送不代表內網部署完成。
