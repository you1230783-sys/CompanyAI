# CompanyAI 專案指引

## 目前工作

- 2026-11-04 使用者調整並授權0.9.3一次完成F01～F07：匯出、Word翻譯、文件比較、流程定時提醒、預設工作區、Word原樣式副本翻譯及新建Markdown；逐功能提交，完整驗證封裝後推送main與Releases。0.9.2已發行，證據見docs/VALIDATION_0_9_2.md。0.9.4才做純搬移拆分、Windows綁定／Cargo features、CI、LTO與漏洞檢查；以docs/IMPLEMENTATION_PLAN.md最新順序為準。
- 0.9.0本機驗證包已用v142空快取完成全Rust／原生整合與NSIS生命週期驗證；VNC入口、桌寵字級／描邊已納入。本輪已獲授權推送GitHub main；公司環境與桌寵人工DPI缺項見docs/VALIDATION_0_9_0.md。
- 完整建置與必要封裝、相關驗證成功才可稱完成；公司實機缺項如實記錄。不能把舊發行授權當作本輪測試通過證據。
- 安裝路徑 C:\largan\LM_AI、公司HTTP、machines.json明文格式與匯入匯出、公司更新同源與簽章檢查均不變。使用者於0.9.1授權GitHub Releases與歷史清理：安裝包及更新JSON移至Releases，Git不再使用LFS；CI、LTO與漏洞檢查仍延後。
- 大檔案純搬移延後至獨立commit；不混入功能變更。
- main歷史已以0.8.75的`9f8febc`重建，後續原始碼保留至0.9.1；對照見offline/history-cleanup-0.9.1.json。不要將本機備份、舊clone或改寫前提交merge／force-push回main。
- 歷史授權與驗證已移至 docs/history/AGENTS_BEFORE_0_9_0.md；逐段對照見 docs/AGENTS_MIGRATION.md。

## 專案界線與程式碼風格

- `Y:\Rust` 是共用根目錄，本專案是 `Project/CompanyAI` 內的獨立 Cargo workspace。原始碼、Cargo.lock、vendor、.cargo、target、dist 與 scripts 留在專案內。
- 不移動或修改其他專案、`.local-ai`、共用 `.tools`、安裝下載與共用環境腳本；不在共用根目錄建立 Cargo workspace 或 .cargo/config.toml。
- 優先使用明確命名、簡單控制流程與單一責任函式，避免不必要的抽象、泛型及巨集。以繁體中文註解說明用途、限制與設計原因；公開 Rust 介面使用 `///`。
- 錯誤訊息應說明原因與可採取的動作。修改邏輯同步更新註解與文件；不以無理由的 unwrap/expect 取代錯誤處理。

## 建置與驗證

- Windows 11 x64、MSVC **v142 / 14.2x**；已驗證 14.29.30133、`_MSC_FULL_VER=192930159`、SDK 10.0.19041.0。Rust 由 rust-toolchain.toml 固定。
- 使用 scripts/Enter-DevShell.ps1（vcvars64.bat -vcvars_ver=14.2）及 scripts/Build.ps1；確認實際 cl/link 與 x64，不使用新版 VS 預設工具集或 target-cpu=native。
- 離線依賴保留 Cargo.lock、vendor、.cargo/config.toml，以空 Cargo 快取及 --frozen 驗證。不要把歷史離線 ZIP 當成目前依賴。
- BuildOnly 僅編譯；ValidateOnly 不更新發行產物；BasicValidation 不代表完整驗證。Office、VNC、SMB、CAD 與 NSIS 依變更範圍及當次授權實測，缺少環境如實回報。
- 真模型、固定參數原生工具、幾何／內容及畫面核對分開記錄。保留原始參數與失敗，不由測試程式替模型補正；流程見 docs/LOCAL_MODEL_VALIDATION.md。
- CAD 使用獨立新圖或副本，保留使用者原圖。修改、Undo、保存重開及文件身分分開核對；工具成功不代表圖面設計核准。
- 0.8.77 完整建置與 NSIS 通過，但 Luna 多檔交付及 CAD 結尾仍有未完成案例；桌寵拖曳／通知點擊、公司模型、真 Outlook 及加密文件仍需現場驗收。不得把原生測試 PASS 當成上述情境通過。

## 持續有效的產品約定

- 公司內資料技能由網頁端提供，應用程式不額外載入。公司主機與路由由 src/config.rs 管理，前端不取得 Token、郵件 EntryID 或任意執行能力。
- 一般聊天與專案保存請求身分；未知提交查原 ID，已確認暫時推論失敗才可依既定退避建立新請求。不可重播成功的本機寫入。
- 原始文件預設唯讀，使用受控副本與版本核對；來源變更、權限與路徑保護不能為了讓任務完成而略過。
- Classic Outlook 經使用者授權讀取，不寄信或改信；資料夾與郵件代號受原生範圍限制。模擬資料不能代替公司信箱實測。
- 設定與私有紀錄依現有 DPAPI／帳號隔離保存；關閉視窗、托盤及快捷鍵遵循現行使用指南，快捷鍵只填草稿、不自動發送。
- 桌寵提供貓／狗及六動作，僅匯入 PNG，建議透明背景，保留原圖並壓縮至每張小於500,000 bytes；角色置前的生成提示可由使用者修改。
- 塔羅離線78牌、單／三張、正逆位；牌面旋轉呈現逆位。AI解析要求問題並使用確切抽牌結果，送一般聊天，不新增塔羅技能。
- Excel公式技能允許合法自訂公式，與常數資料分工，計算後讀回值與錯誤。技能說明不能取代程式的授權及輸入驗證。
- NSIS 固定安裝 C:\largan\LM_AI，內含 Python／CAD runtime；WebView2 分開提供。一般安裝不自動啟動，更新沿用 PID 交接與重新啟動契約，不執行使用者端 PowerShell／BAT。

## 發行與資料留存

- 只有當次授權發行才進版、更新 dist、封裝及推送。正式交付同步 Cargo版本、公告、文件、成品與兩份簽署清單；核對遠端 main 與Release標籤，下載Release資產核對SHA256。
- 更新私鑰、個人設定、Token、模型對話、.build、.codex、dist/onboarding-state.json、icon.png 與本機「舊版留存」不提交。
- 不因檔案建立較早就移走現行原始碼、授權、vendor 或重建輸入。offline/GstarCAD-Probe.zip、python-inputs.zip 與 installer/nsis-3.12.zip 仍是本機重建輸入，依新授權停止追蹤Git；本機保留，鎖定版本／雜湊及準備說明仍提交。
- 封存保留原相對路徑、搬移清單與檔案核對結果；不得靜默覆寫已有留存。歷史指示以最新使用者授權為準，不把舊交接的等待／暫停當成當前狀態。

- 本輪歷史清理授權：以0.8.75為新根，保留其後原始碼變更至0.9.1；先備份、推送0.9.1與上傳Release資產，再改寫main，正式Release標籤指向重建後main。保留獨立NSIS測試專案；舊應用分支不得繼續引用0.8.74以前歷史。GitHub伺服器LFS回收與Git引用清理分開回報。
