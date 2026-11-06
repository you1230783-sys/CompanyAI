# 每版核心回歸與不變條件

本清單是驗收要求，不是通過紀錄。每版以 `VALIDATION_<版本>.md` 填入實際命令、證據、結果與限制。模擬服務、原生固定參數與公司實機分開記錄。

| 核心流程 | 每版固定案例 | 自動驗證入口 | 公司實機補驗 |
|---|---|---|---|
| 登入 | 短碼、取消、過期、錯誤授權、未登入拒絕工具；診斷不依賴登入 | auth測試、原生WebView2自檢、access測試 | 公司瀏覽器短碼與實際帳號登入 |
| 一般對話 | 正常回覆、附件、停止、斷線、未知提交查原ID、不重複提交 | jobs／transport測試、原生composer／demo | 公司服務及實際模型回覆 |
| Office讀取 | Word／Excel／PPT讀取、缺失、占用、來源版本變更 | Build.ps1 -TestOffice，Office COM與原生測試 | 公司加密文件與實際Office版本 |
| 寫入副本 | 原件不變、新成果、同名拒絕覆寫、外部修改拒絕、保存重開讀回 | projects/files及office測試、project_smoke、delivery_smoke | 實際公司資料與格式核對 |
| 更新 | 同源、HTTP來源維持、簽章／雜湊／版本拒絕、安裝重啟、失敗回復 | deployment測試、Test-Installer.ps1 | lm_server發佈與舊版實機升級 |
| 中斷恢復 | 停止、重新開啟、失效授權、過期事件、已成功寫入不重播 | steering／jobs／runner／composer與原生測試 | 公司模型及長任務 |
| 非預期錯誤 | 背景panic、WndProc panic、真正WebView2回呼panic、未捕捉ABI中止前紀錄 | Test-Reliability.ps1（獨立程序） | 真實故障回報；不宣稱涵蓋原生存取違規／斷電 |
| 診斷 | 預設白名單、詳細內容警示、預覽與匯出同一快照、取消、失效ID拒絕 | errors測試、Test-ReliabilityUI.cjs | 使用者實際匯出流程 |
| 不可信內容 | 文件／郵件只當資料、偽造ID／失效revision／路徑穿越／連結拒絕、未同意Outlook不讀取 | projects/files、mail、cad與AppContainer測試 | 真模型可能受誤導，工具邊界仍須拒絕越權 |

## 每版交付核對

- 安裝 `C:\largan\LM_AI`，不更動公司防毒白名單路徑。
- 公司HTTP及更新同源／RSA簽章檢查維持；GitHub手動下載不改程式更新來源。
- machines.json明文格式及JSON匯入／匯出維持；診斷不收集其內容。
- MSVC v142 x64實際cl/link核對、Cargo.lock＋vendor＋.cargo/config.toml、空快取frozen。
- 完成該版fmt／Clippy／Rust／UI／必要原生與封裝驗證；缺環境項目列明。
- CAD只用新圖／副本；公司文件及使用者正在編輯的原件不作測試材料。
- Git只提交公開來源、文件、必要依賴與授權成品；不含私鑰、帳密、對話、私有診斷。

## 故障後行為

0.9.0的非預期panic採保守隔離：停止接受新工作、撤銷等待中的確認、要求在途工作取消、丟棄晚到事件；仍可預覽診斷、複製錯誤與退出。原生呼叫可能已完成部分寫入，不能宣稱已回復或立即重試。重新啟動後先核對原請求與成果。

panic hook不包含payload、堆疊、檔名、帳號或專案路徑，只記來源位置雜湊、行列、版本與固定類別。磁碟不可寫、程序強制終止、原生存取違規或OOM不保證可記錄／恢復。
