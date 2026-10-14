# 0.8.53 基礎驗證測試包

2026-10-14，使用者要求先提供進版EXE／NSIS作公司實測，完整驗證留待確認後補做。本版修改一般聊天413拒絕後繼續GET而衍生404的流程、連續三次相同查詢錯誤的停止條件，以及分析摺疊和圖中文字編輯介面。

## 行為與範圍

- POST等待回應且尚無task_id時不預先GET；明確4xx拒絕（408除外）或 `task_accepted:false` 停止本機追蹤並保存原HTTP狀態與伺服器訊息。已確認受理的任務、網路失聯／408／未明示拒絕的5xx仍只查原request_id，不自動換ID重送。
- GET連續三次相同錯誤，當輪models端點HTTP200且清單有效，才自動停止；成功或查詢路由ID改變重設計數。停止先保存片段及DPAPI狀態，晚到事件不能復活，停止不另發GET／取消。
- 完成答案的分析依據預設摺疊；同對話重繪保留手動展開及內層證據。圖表移除上方重複文字清單，新增入口在工具列，圖內雙擊編輯／刪除。
- 網站回報的413可能與目前及歷史附件文件ID合計上限有關，但缺回應本文尚未定案。本版修正桌面錯誤處理，不更改網站附件限制。415網站端已調整，實際PDF仍需公司測試。

## 建置結果

2026-10-14 14:52 完成 `scripts/Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller`，結束碼 0，結果為 **PASS_BASIC**。EXE 與 NSIS 檔案版本均為 `0.8.53.0`；建置完成後使用者追加授權，將原始碼、產物與基礎驗證紀錄一併提交至既有 Git main。

- 實際 MSVC v142 `14.29.30133` x64 編譯器與 linker，C 探針確認 `_MSC_FULL_VER=192930159`；Rust `1.98.1`、SDK `10.0.19041.0`。
- 專案 `Cargo.lock`／`vendor`／`.cargo/config.toml`、新建空 Cargo 快取與 `--frozen`；fmt、Clippy 全 target、release 編譯通過。
- 聊天與附件相關 Rust 測試 21 項通過。包含背景／串流模式透過實際本機 HTTP 回傳 413、原始 HTTP 錯誤保留與敏感 token 遮蔽。
- 正式 EXE 的 WebView2、原生輸入區及 5 組任務追蹤自檢通過：POST 等待中不查詢、413 停止及 DPAPI 重啟恢復、第三次相同 404 且模型正常才停止、服務不正常時繼續／成功及變更錯誤時重設、忽略舊路由與停止後晚到事件。部分回答保存通過。
- Python runtime 準備／自檢、Outlook 輔助程序管線與 EOF 退出（未讀信箱）、DLL 相依檢查通過。
- Edge 圖表互動回歸 8 組及內容比較／分析摺疊 31 項通過；涵蓋圖中文字雙擊編輯／刪除、工具列新增入口，以及答案完成收起、手動展開、同對話重繪。
- NSIS 打包成功；正式 EXE 核對更新清單簽章與安裝包通過。兩份更新清單內容相同，均指向本版 NSIS。這是更新清單簽章核對，不是 Windows Authenticode 簽署聲明。

### 交付檔案

| 檔案 | 大小（bytes） | SHA256 |
| --- | ---: | --- |
| `dist/LM_AI.exe` | 17731584 | `f70db124f6e0a16015d3d1d8bd5779938fbe12cb9712e9655a867ef982ee36c0` |
| `dist/LM_AI_Setup.exe` | 28714300 | `1d97a28697d182cae0e6a19fc6713295ee4c976290420497a5b180139eee6c26` |

建置紀錄：`.build/build-0.8.53-basic.log`。正式報告：[基礎驗證](../offline/exe-verification.json)、[任務追蹤](../offline/task-tracking-verification.json)、[原生輸入區](../offline/composer-verification.json)、[工具鏈](../offline/environment.txt)。前端報告：`.build/chart-layout-review.json`、`.build/review52-ui.json`。其他既有 `offline` 報告保留原版紀錄，不代表本次已重新驗證。

## 延後驗證

完整workspace測試、專案／Python隔離及長任務、真實Office與SMB，以及NSIS安裝／更新／回復／解除安裝流程未納入本次基礎驗證。公司服務、PDF加密與模型實測也尚未通過。本版不得宣稱完整驗證完成。
