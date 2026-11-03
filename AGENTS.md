# CompanyAI 專案指引

## 目前狀態與授權

- 目前發行版為 **0.8.77**，提交 `fa2833f641694554aac25483979becc17cb5c59d`；EXE／NSIS 已推送並以固定提交下載核對。驗證範圍與限制見 docs/VALIDATION_0_8_77.md。
- 2026-11-03 使用者授權整理本機及 Git：0.8.74 及以前的版本專屬成品與歷史紀錄移至 `Y:\Rust\舊版留存\CompanyAI\`，不提交封存內容；保留 0.8.75～0.8.77、現行規格及建置依賴。
- 本次只整理檔案與文件，提交／推送既有 origin/main；不進版、不編譯、不重製 EXE／NSIS，也不操作使用者正在測試的應用程式。Git 歷史不改寫，整理政策見 docs/ARCHIVE_POLICY.md。
- 使用者正在實測。後續待討論事項是多成果任務進度、工具識別資訊及 CAD 未存檔成果的完成判定；本次未授權實作這些功能。

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

- 只有當次授權發行才進版、更新 dist、封裝及推送。正式交付同步 Cargo版本、公告、文件、成品與兩份簽署清單；核對遠端 main 及固定提交 LFS 下載雜湊。
- 更新私鑰、個人設定、Token、模型對話、.build、.codex、dist/onboarding-state.json、icon.png 與本機「舊版留存」不提交。
- 不因檔案建立較早就移走現行原始碼、授權、vendor 或重建輸入。offline/GstarCAD-Probe.zip 與 python-inputs.zip 是目前重建依賴，保留。
- 封存保留原相對路徑、搬移清單與檔案核對結果；不得靜默覆寫已有留存。歷史指示以最新使用者授權為準，不把舊交接的等待／暫停當成當前狀態。
