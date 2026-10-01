# 專案級開發指引

適用於本專案及其子目錄的程式碼撰寫、編輯與維護。

## 專案位置與共用環境

- 本專案位於 `Y:\Rust\Project\CompanyAI`，是獨立的 Cargo workspace。
- 原始碼、依賴鎖定檔、.cargo 設定、文件、target 與 dist 留在本資料夾。
- 專案環境入口是 `scripts/Enter-DevShell.ps1`：優先使用解壓後的 `toolchain/`，否則使用外層 `Y:\Rust\.tools`。編譯輸出固定在本專案內。
- `Y:\Rust\.local-ai` 與外層安裝腳本是離線編譯環境的資源，不屬於此應用程式，不要搬入或任意修改。
- 離線 ZIP 自帶固定 Rust 工具鏈與 vendor 套件；解壓到新資料夾即可編譯，但電腦仍須預先安裝指定 MSVC 與 Windows SDK。一般開發也可用 `RUST_SHARED_ROOT` 指定共用工具。

## 使用者指定的程式碼風格

- **簡單好懂、註解充足、他人看得懂且改得了、容易維護，為最優先原則。**
- 使用直觀的控制流程與明確命名；避免難讀的一行式、過度泛型、複雜巨集及不必要的抽象。
- 函式與模組保持單一、清楚的責任，讓接手者能由上而下理解執行流程。
- 主動提供足夠的繁體中文註解，說明模組與函式用途、主要步驟、輸入輸出、限制及重要設計原因。
- 對較難理解的 Rust 所有權、生命週期、並行、錯誤處理及平台相容性設定，補上容易理解的說明。
- 公開介面優先使用 `///` 文件註解，必要時提供小型範例；註解應提供有用資訊，不只重述語法。
- 修改邏輯時同步更新相關註解與文件，避免過時資訊誤導維護者。
- 錯誤訊息交代失敗原因與處理方式；一般執行流程避免無理由的 `unwrap()` / `expect()`。
- 引入套件或增加架構複雜度前，先考慮標準函式庫與較簡單的做法，以實際需求為準。

## 編譯基準

- 公司目標：Windows 11 x64、VS2019、MSVC 14.2x / v142。
- 本機已驗證：Rust 1.98.1、MSVC 14.29.30133、Windows SDK 10.0.19041.0。
- Rust 版本由 `rust-toolchain.toml` 固定；不得因另裝新版 VS 就自行改用較新 MSVC。
- 使用 `scripts/Enter-DevShell.ps1` 載入環境，使用 `scripts/Build.ps1` 檢查格式、Clippy、release 編譯與基本執行。
- VS Code 使用 `scripts/Configure-VSCode.ps1` 產生本機 Rust / MSVC 設定，再重新載入視窗。不要把含本機路徑的 `.vscode/settings.json` 提交 Git 或打進 ZIP；離線工具鏈須保留 rust-src，以支援程式碼分析。
- 不加入 `target-cpu=native` 等綁定家用 CPU 的設定。
- 公司完整 MSVC 修補版本與 SDK 版本尚待核對，不宣稱兩邊環境已完全一致。

## 工作範圍

- 本專案目前開發 LM_AI Windows 測試版：瀏覽器授權登入、30 天登入保存、OpenAI 相容 Chat Completions 請求與回覆顯示；網頁端串接規格記錄在 docs/WEB_INTEGRATION.md。
- 網頁控制介面及共用套件編譯管理由另一個 Codex 任務處理。本專案只維護自己的 EXE、原始碼與離線交付包。

## 0.7.0 產品約定

- 本版覆蓋舊版衝突描述，以 docs/DESKTOP_0_7_CONTRACT.md 為準。一般模式即 stream:true + execution_mode:stream；背景為 false + background；不保留同步聊天入口。路由維持 /lm_server 前綴。
- outlook_triage 與 auto_generate_title 必須互斥；Outlook 初篩走背景，不額外生成標題。一般標題為獨立快速模型背景任務；改名、置頂與草稿保存在本機。
- 關閉視窗及最小化均縮到托盤，托盤「離開」才退出。單一實例鎖只限正式模式；demo 與自我檢查不得喚醒正式程式。
- 選字浮動圖示預設關閉，只查 UI Automation 選區位置，使用者點擊才複製；快捷鍵有獨立啟用開關。
- 安裝包為 dist/LM_AI_Setup.exe，部署至使用者 Programs/LM_AI，資料維持 CompanyAI。0.8 以 NSIS 安裝／更新／解除安裝，禁止使用者端呼叫 CMD、PowerShell、BAT 或外部腳本。更新契約以 docs/UPDATE_0_8_CONTRACT.md 為準：下載完整 Setup，內建 RSA 公鑰驗證版本／平台／長度／SHA256 簽章。一般更新需下載前、安裝前兩次同意；真正退出不自動安裝。低於最低版本鎖住功能，已知門檻持久化直到新版達標。更新私鑰在 .private/，嚴禁提交或納入離線包。

- 正式主機、聊天、device、token、version、models 與 download 路由固定於 `src/config.rs`，不提供 UI 編輯；偏好檔只保存模型代號、快捷鍵、字體大小、側欄收合、通知提示及深色模式偏好。舊設定不得覆寫固定路由。
- 正式請求固定 Bearer 個人 Token，Chat Completions 一般模式為 `stream: true`，背景為 `stream: false`；模型選單從後端取得，UI 顯示 label、JSON 傳 id，真實模型由後端映射。
- 使用者指定：版本檢查失敗暫時允許使用；已知的強制更新跨重新啟動保存，不得被網路失敗或後續降低門檻解除。這個政策不能略過登入驗證或模型權限。
- 全新設定快捷鍵預設 Win+Esc，既有偏好保留；支援直接按鍵錄製，套用成功才生效；必須在顯示主視窗之前取得來源選字。只放入草稿，不自動送出、不覆蓋舊草稿、不背景監控剪貼簿。
- 0.4.1 擷取完成後先填入草稿再請求前景；不依賴 Ctrl+V 或強制焦點技巧。等待新剪貼簿時不限制擁有者 PID／來源 HWND，以支援 Adobe 多程序；讀取新的穩定純文字並要求使用者確認。錄製時暫停原快捷鍵，取消／逾時／離開程式時恢復。
- 0.4.0 已加入內嵌 WebView2 介面、Markdown／KaTeX／高亮、本機 DPAPI 歷史、通知 REST／WebSocket，以及 Classic Outlook 唯讀預覽／確認分析。
- 公司一律使用 Classic Outlook；以 Rust windows COM 操作，不依賴 Python 或 pywin32。不寄信、不修改郵件、正文必須經使用者明確確認後讀取。
- 0.5 契約集中於 docs/DESKTOP_0_5_CONTRACT.md。附件能力由獨立 capabilities 路由提供；文件／圖片合計最多 20 個，server 決定格式與大小。
- 附件以 JSON 預約 job_id、PUT 原始 bytes、REST 查轉檔狀態，ready 後才送 AI；本機只分塊 DPAPI 暫存，不轉 MD、OCR 或處理文件。
- stream／background 都依賴 server 持久任務與 owner + client_request_id 去重；SSE／WS 不取代 REST 結果。同帳號 principal_id 穩定，任務加密隔離保存；未知提交不可換 ID 自動重送。
- 最小化至托盤，右鍵可還原／離開，退出不取消 server 任務。圖示使用 assets/app.png 原圖與 assets/app.ico 多尺寸資源，EXE／視窗／工作列／托盤共用；更新圖片後執行 scripts/Convert-AppIcon.ps1，ICO 為必要交付資源。
- 0.6 全站鈴鐺 API 與 AI events 各有游標／快取；網站已讀與刪除成功才更改本機，deleted_ids／410 完整同步契約見 docs/DESKTOP_0_6_CONTRACT.md。
- 模型切換必須重新查 capabilities?model=alias；不能以舊模型的附件規則放行新模型。網站仍須驗證轉檔後的圖片是否可交給模型。
- 0.6 Outlook 批次 Skill 於 src/outlook/skill.md；只有本批已勾選、經使用者確認的郵件可依 AI JSON 請求匯出 MSG。允許的工具只有 outlook.export_msg，EntryID／StoreID 與磁碟路徑不提供 AI。
- 多封日期查詢預設為 Outlook 目前資料夾及子資料夾，另可選預設收件匣及其子資料夾；介面已移除所有已載入信箱／本機資料檔選項。不掃描磁碟或自動開啟資料檔。每批取最新 50 封，500 個資料夾／10,000 個項目／30 秒上限與讀取失敗須提示結果不完整；自動 MSG 補充最多 20 封並受 backend 規則限制。退出不恢復 COM 授權；最終聊天使用持久任務。
- RAG、UNC 知識庫、任意檔案工具及其他自動化尚未實作，不自行擴張本次範圍。
- 前端程式位於 ui/，build.rs 將資源嵌入 EXE；第三方資源及授權位於 ui/vendor，不使用 CDN，也不要求公司安裝 Node。
- 使用者同意附 WebView2 x64 離線安裝包，放在 dist/ 並納入 ZIP；更新時核對 Microsoft 簽章及 SHA256。
- 不向前端傳 Token、真實郵件 EntryID 或任意檔案／命令執行能力。通知不能直接觸發外部工具。
- 使用者明確要求先提供網頁契約時，可先獨立提交／推送契約文件供同步開發；必須標示程式尚未驗收，其他原始碼及 EXE 待完整交付驗證後再提交。
- 修改功能時同步更新 docs/WEB_INTEGRATION.md 與相應驗收說明。沒有實際操作 Word／Outlook 或取得外觀截圖時，不能宣稱這些驗收通過。

## 0.8.30：原生工具 Schema 相容性修正（最新）

- 使用者回報第一輪 `UNSUPPORTED_SCHEMA`：type 陣列只允許單一型別加 null。桌面轉換器須將 Office 儲存格與圖表橫軸的多型別改為非根位置 anyOf，不要求網站放寬既有契約。
- 沿用 EXE／Git 發行授權，執行 Build.ps1 -EmptyCargoCache，涵蓋 v142、空快取、全部工具 Schema 及既有原生代理整合。Office COM 實作未修改，不重跑 Office／NSIS／ZIP；公司網站聯測仍需使用者確認。

## 0.8.29：desktop-agent-v1 原生工具

- 依使用者要求實作已發布的 docs/DESKTOP_AGENT_V1_CONTRACT.md；新專案及快速摘要委派走原生代理路由。一般聊天、Outlook、標題、PDF 不變。
- 不從模型正文擷取工具 JSON；能力不支援不降級。僅已存在的舊 checkpoint 延續原協定；debug 的 legacy 測試入口不可作正式降級。
- 使用者已要求編譯後上 Git：進版 0.8.29，以 Build.ps1 -EmptyCargoCache -TestOffice 完成 v142／空快取／原生代理及 Office 驗證，提交 EXE、簽署清單、原始碼與文件至 main；不製作 NSIS／ZIP。狀態見 docs/VALIDATION_0_8_29.md。
- 使用者後續要求一併加入 Excel 選欄／分批讀取。新增 inspect_excel／read_excel_range／chart_excel_range；不變更網站通用契約，資料每批最多 2000 格、預設 100 列；完整編輯快照維持原上限。

## 0.8.28 專案技能與視覺化（最新）

- 使用者授權側欄獨立收合、Markdown 說明、三種工作技能、跨文件搜尋、批次 Office、ECharts 與品質委派快速摘要。沿用 EXE／Git 發行授權，不製作 NSIS／ZIP。
- 技能只載入內建文字；圖表固定資料結構、離線 ECharts，不執行模型腳本。Office 批次候選成功才提交；快摘要不標記主模型已讀。
- 委派保存原請求，未知結果不得重送 POST；與父任務共用取消／時限，摘要 DPAPI 快取與來源版本核對。
- 使用 Build.ps1 -EmptyCargoCache -TestOffice 驗證；公司加密、實際模型摘要品質及視覺版面需另行實測。

## 0.8.27 長任務時限與異常續接（最新）

- 使用者要求專案時限延長至兩小時，並修正異常停止只有重試、沒有繼續的情況。每段 7200 秒，60 工具／80 模型回覆不變。
- 時限、格式修復次數、無進展與文字預算上限成功 DPAPI 暫存後可手動繼續。待查模型請求保存 ID／已知狀態；續接只查原請求，不重送 POST，已完成回覆可直接沿用。
- 取消、回應身分錯誤、未知本機操作不可套用一般續接；舊版失敗且沒有 checkpoint 的任務不補造狀態。0.8.26 暫停檔向後相容。
- 沿用 EXE／Git 發行授權，進版 0.8.27，Build.ps1 -EmptyCargoCache；沒有修改 Office COM，不重跑 Office／NSIS／ZIP。時間邊界採 debug 專用短時鐘實測，不宣稱已等待兩小時。

## 0.8.26 Office 初版與暫停續接

- 使用者同意新建 Office、基本結構與排版初版，並要求工具／模型回覆上限改為保存狀態後手動「繼續」。沿用 EXE 測試版交付方式；本版不製作 NSIS／ZIP。
- 新建限 DOCX/XLSX/PPTX，既有 Office 副本維持來源格式。模型只能使用固定 office_action，禁止任意 COM／Shell／巨集。格式快照共用樣式表，數值正規化至千分之一點。
- 每段仍最多 60 工具／80 模型回覆；剩餘四次時提醒摘要，達上限 DPAPI 保存工作副本、去重紀錄與進度，釋放程序。使用者明確續接才重新授權，核對帳號／專案／版本，不自動重播未知請求。
- 404 診斷保留原 POST 錯誤與 GET 路由；同一請求有限重查，不換 ID 或重送 POST。公司那次原因未取得後端日誌不得宣稱已確認。
- 驗證使用 Build.ps1 -EmptyCargoCache -TestOffice；記錄原生 Office、新建及舊格式、真實 60 工具暫停續接與第 20 輪短暫 404。公司加密與視覺排版仍需公司實測。

## 0.8.25 專案文字工具協定（最新使用者指示）

- 使用者要求採用 OpenAI Chat Completions 的 tools／tool_calls 形狀，但定義與回覆都維持在 messages 文字中；網頁只轉送，不新增原生 tools、tool_choice、response_format 或 role=tool 的 HTTP 欄位。
- tools.json 集中定義參數，skill.md 只教一種回覆格式並精簡重複說明。每輪單一呼叫，id 映射原 operation_id，function.name／arguments 映射既有 Decision／Tool；finish／ask_user 也是桌面自訂工具。不是 API 原生工具呼叫，也不宣稱強制 Schema 保證。
- arguments 提示使用標準 JSON 字串，parser 同時容許完整物件，避免多一層引號的無效重試；保留舊 action 格式相容。多操作、混用協定、缺漏參數先修復，不猜工具／版本；未知工具與無效操作 ID 停止。版本、授權、去重與成果檢查不放寬。
- 使用者已要求編譯後放到 Git，取代先前僅改原始碼的限制。進版 0.8.25，執行 Build.ps1 -EmptyCargoCache，交付原始碼、EXE、簽署 EXE 清單與驗證文件至既有 main；不製作 NSIS／ZIP。詳見 docs/DESKTOP_0_8_25_CONTRACT.md。

## 待下次發行：缺少工具名稱的格式修正

- 使用者回報摘要工具 request 漏寫 tool，要求修正原始碼，**先不編譯、不進版**，留待其繼續測試後一起處理；本輪不打包、不提交或推送，保留 0.8.24 成品與清單。
- 缺少／非物件 request，或缺少／空白／型別錯誤的 request.tool，回傳明確格式原因，沿用既有有限修復；不自行猜測工具名稱。非空但不支援的工具仍停止。
- 補上回歸測試，僅做格式與原始碼檢查，未編譯執行的測試必須明確標示；詳見 docs/PENDING_TOOL_NAME_REPAIR.md。

## 0.8.24 可選閱讀筆記（最新）

- 使用者要求取消每三次閱讀／六次操作的筆記要求。每份文件、每個版本各自計數，有效閱讀超過四次且未完整讀完時，下一輪才附一次可選技能；可略過，不重試或阻擋。
- 讀完全文後該文件計數歸零；重讀、重播及失敗不增加計數，切換文件不混算，版本變更重新計數。分段摘要按需，完整文件摘要及 .lmai 保存流程保留。
- 自願提供 progress_note 仍可用於有限續接；沒有筆記不擅自裁掉未摘要原文。沿用 EXE／main 交付授權，進版 0.8.24，執行 Build.ps1 -EmptyCargoCache，不重製 NSIS／ZIP。

## 0.8.23 專案記憶（最新）

- 使用者授權實作 `.lmai` 筆記、文件分段摘要、任務摘要／完整結果、PDF 跨任務快取；一般檔案清單、讀取、匯入與副本工具必须排除 `.lmai`，不把它當文件送出。
- 私有資料 DPAPI 保存，來源版本／閱讀證據／副本狀態由程式核對，摘要只作語意記憶。新對話請求選取摘要及近期要求，完整舊答案可按需讀回；一般聊天不變。
- 單一專案任務卡與最終通知，內部往返只顯示活動；按精確請求 ID 過濾通知，網站須保留 resource_id 關聯。
- 交付 0.8.23 EXE、簽署清單、原始碼及文件至既有 main，使用 `Build.ps1 -EmptyCargoCache`；不製作 NSIS／ZIP。公司加密與模型摘要品質待使用者實測。詳見 docs/DESKTOP_0_8_23_CONTRACT.md。

## 0.8.22 專案筆記與有限續接（最新）

- 使用者已回報 0.8.21 PDF 路由可讀取；本輪改善工具回覆不穩、多 JSON 及空白 done，不變更 PDF／Office 讀取方法。
- 唯一合法操作可附說明；多 JSON／不完整／空白完成只要求修復，不猜選候選命令。未知工具、身分錯誤、取消及結果不明不自動另建任務。
- 三段新閱讀或六次有效非瀏覽操作後，要求下個正常 JSON 同層附 progress_note；不多呼叫一輪筆記。程式保存區間、版本、操作及副本狀態，AI 筆記不是授權或成功證據。
- 無進展最多修復兩次，全任務六次；第一次原地修復，第二次按筆記縮減已摘要歷史，保留原始需求／補充、最近兩段已摘要證據及全部未摘要結果。沿用同個 broker／worker／PDF 快取，不重播成功寫入。
- 上限 60 次工具、80 次模型回覆、連續八次無進展、30 分鐘；文字預算 120 KB 軟門檻／240 KB 硬門檻，不等同模型 token。一般聊天仍維持 20 輪限制。
- 交付 0.8.22 EXE、簽署清單與原始碼至既有 main，Build.ps1 -EmptyCargoCache；不製作 NSIS／ZIP。真實 GLM／Gemma 的改善幅度由使用者測試。詳見 docs/DESKTOP_0_8_22_CONTRACT.md。

## 0.8.21 伺服器 PDF 轉換（最新）

- 使用者已確認 0.8.20 MSG 在公司測試成功，PDF 一般附件伺服器轉檔可正常閱讀；不把此回報當成新路由已驗收。
- 專案 PDF 正式任務改用 POST /api/desktop/documents/pdf-to-markdown，保留部署前綴（公司 /lm_server）；Bearer 桌面 Token、X-Client-Version、單一 multipart file，50 MiB 上限；200 回原始 UTF-8 Markdown，不是 JSON。
- 不传 extract_images；Server 已固定 false。同步等待，不輪詢；WinHTTP 非同步 I/O 支援取消，receive 180 秒／整次 300 秒，不自動重送。原檔鎖、專案邊界與 TXT 成果規則保留。
- 相同任務依來源 SHA256 重用記憶體快取；不寫明文 MD、不把伺服器 filename 當磁碟路徑。全文仍為 200 KB 上限，不靜默截斷。
- 交付 0.8.21 EXE 與簽署清單、原始碼與文件，Build.ps1 -EmptyCargoCache；沿用 main 推送授權，不製作 NSIS／ZIP。真實新路由仍待公司測試。

## 0.8.20 詢問與讀取修正（最新）

- 使用者回報 PDF／MSG 都可手動開啟，但工具失敗；說明＋ask_user 被誤拒絕。统一 Decision 解析與等待補充狀態。
- MSG 獨立暫存副本協調 Outlook 共享需求，不放寬原件寫入保護；保留 COM EXCEPINFO。公司加密仍待實測。
- 使用者確認公司免費 Acrobat Reader 可正常開啟，但 Word 讀 PDF 是亂碼。Word PDF 備援本機測試未正常返回，不納入；PDF 改進原因說明及 ask_user 匯入流程，不宣稱自動解密已修好。
- 沿用 EXE／main 推送授權，交付 0.8.20，使用 Build.ps1 -EmptyCargoCache -TestOffice；不重製 NSIS／ZIP。詳見 docs/DESKTOP_0_8_20_CONTRACT.md。

## 0.8.19 文件相容性與工具說明（最新使用者指示）

- 修正成果定位路徑，完整單一工具 JSON 可附說明並繼續執行；說明直接呈現在對話中，不再要求重送完整合法工具。歧義／未知工具停止，不完整 JSON 最多修正兩次。
- PDF 以內嵌 pdf-extract 在 AppContainer 讀取；MSG 以已開啟的 Classic Outlook OpenSharedItem 讀取。兩者只支援文字閱讀及 TXT 衍生副本，不編輯原始格式、不做 OCR／附件解析。
- Office 支援 DOC/DOCX/DOCM、XLS/XLSX/XLSM/XLSB、PPT/PPTX/PPTM，副本保留原格式；仍只修改既有文字區塊，不新增表格／列欄／版面。
- 使用者已詢問 Word 表格能力並獲告知：既有表格文字可修訂，尚無新增表格工具；本輪不擴充表格結構操作。
- 本輪交付 0.8.19 EXE、清單與原始碼，執行 `Build.ps1 -EmptyCargoCache -TestOffice`，沿用 main 推送授權；不打 NSIS／歷史 ZIP。MSG／公司加密未實测通過不得宣稱驗收。

## 0.8.18 成果與 Office（最新使用者指示）

- 使用者要求成果可定位 Explorer、輸出保留可讀檔名、任務資料夾以時間命名，並試用 Word／Excel／PowerPoint；完成後編譯 EXE 並推送既有 main。
- 進版 0.8.18，執行 `Build.ps1 -EmptyCargoCache -TestOffice`；本輪不製作 NSIS 或歷史 ZIP。公司加密及版面外觀未經公司實測不得宣稱驗收。
- Office 第一版 DOCX 正文、XLSX UsedRange 文字／數字、PPTX 一般文字框；原件唯讀，固定 COM 工具，禁止模型指定任意 COM 方法。公式不改，不從空白建立 Office。相關限制见 docs/DESKTOP_0_8_18_CONTRACT.md。

## 0.8.17 專案歷程與重試（最新使用者指示）

- 使用者已回報公司測試：列專案目錄、檔案清單、編輯 TXT 與儲存副本均成功；不延伸宣稱所有公司加密格式皆已驗收。
- 本輪合併先前 UI 修改與 VNC 更新對話框，通知摘要最多 50 字，專案工具歷程可收合，包裝工具 JSON 最多修正重送兩次，最新提問提供重試。
- 重試保留原對話及成果；結果不明時使用原請求 ID，不另建重複伺服器工作。專案重試建立新執行器／輸出目錄；一般聊天保留原模型、用途及仍有效的附件。
- 最新使用者明確要求完成後編譯 EXE 並放到 Git：進版 0.8.17，執行 `Build.ps1 -EmptyCargoCache`，提交並推送原始碼、EXE、簽署 EXE 更新清單及文件至既有 main。此條取代上一輪僅修改原始碼的限制。
- 本輪先不製作 NSIS 或歷史離線 ZIP。原生選檔、專案移除及真實網站操作的未測部分需據實記錄，不以 DOM 測試冒充完整公司驗收。
- 詳見 `docs/DESKTOP_0_8_17_CONTRACT.md`；先前 `PENDING_PROJECT_UI.md` 與 `PENDING_VNC_SYNC_DIALOG.md` 的 UI 調整一併納入本版。

## 0.8.16 專案文件工具（最新使用者指示）

- 本次實作專案、固定 TXT／MD 工具與 AppContainer 文字執行器；專案位於 VNC 下、最近對話上。一般對話不附帶桌面文件技能，專案對話仍產生獨立標題。
- 對外用途旗標改成 skills 布林值；一般／附件最終整理 true；Outlook 初篩、標題與專案工具往返 false。網站不依標題關鍵字分流。保留本機用途紀錄。
- Outlook 勾選自動補充內文時，初篩與最終整理均為 quality；未勾選沿用使用者模型，不修改一般聊天偏好。
- 原檔唯讀、成果預設 TXT；TXT 不轉 MD。MD 不受公司加密保護，已讀 TXT 的任務禁止發布 MD。無法直接解密時停下，接受使用者明確匯入的記事本文字；不宣稱已整合自動操作記事本。
- 更新箭頭位於設定旁，有更新才顯示；一次原生確認涵蓋指定版本下載與安裝。同版优先 NSIS，較新 EXE 仍優先，EXE 維持手動更換。此條取代兩次更新確認規則。
- 使用者已要求完成後以 `Build.ps1 -EmptyCargoCache` 編譯並推送 EXE、EXE 更新清單、原始碼與文件；先不製作 NSIS 或重製歷史 ZIP，待公司讀寫實测後再打包。本機驗證不能代替公司加密／模型環境測試。

## 修改後的交付與 Git 規則

- **0.8.15 登入與彈窗交付（最新）**：使用者已要求編譯、打包並推送，取代下方本批修改暫不發行的限制。進版 0.8.15，使用 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 完成 v142、Rust、WebView2 與 NSIS 安裝／更新驗證，交付 EXE、NSIS、兩份簽署更新清單、原始碼與文件並推送既有 main。保留現有 VNC 設定與歷史離線 ZIP，不重新製作 ZIP；Git 推送不代表公司內網路由已更新。

- **待下次發行：登入門檻與彈窗關閉（最新）**：登入改為獨立「登入」按鈕，提示「尚未登入時無法使用其他功能」，登入後整區隱藏；未登入時所有其他介面控制及原生命令均禁用，包含 Outlook、VNC 與設定。仍有效的保存授權可沿用。所有 HTML dialog 支援點擊外部關閉；確認視窗以取消處理，關閉更新提示不解除原生版本門檻。使用者明確要求本輪只修改原始碼與文件，不編譯、不進版、不打包、不推送，留待下次一起驗證發行；此條優先於下方舊交付流程。現有 0.8.14 成品與清單不變，詳見 docs/PENDING_LOGIN_AND_DIALOGS.md。

- **0.8.14 NSIS 交付（最新）**：使用者已回報測試成功並要求打包及部署更新。維持 0.8.14，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller`，驗證安裝、更新、重新啟動與解除安裝，交付 EXE／NSIS、兩份已簽署更新清單與驗證文件並推送既有 main。保留 VNC 設定與使用者資料；不重製歷史離線 ZIP。此條取代本版先前等待 NSIS 的限制，Git 推送不代表內網已部署。

- **0.8.14 VNC 預覽與重取（最新）**：匯入預覽依英文／數字自然排序，未分類最後；匯入時排序受影響分類內的機台，既有手動移動功能仍可使用。名稱與 IP 相同標綠並禁止勾選，不同標紅供使用者決定；比對名稱跨分類，重複匹配不任意覆寫。保留網站 Session 到匯入／捨棄，首次清單未分類達 50% 時自動重取一次，上限一次，不循環；手動「再次取得更新清單」只重取一輪 API。取消、停用、正常離開及錯誤均嘗試登出。使用者要求完成後編譯 EXE 並推送 Git，進版 0.8.14，執行 `Build.ps1 -EmptyCargoCache`；NSIS 待實測確認才製作，不重製 ZIP。此條取代先前取得後立即登出與匯入不排序的規則。

- **0.8.13 VNC 手動同步（最新）**：依使用者要求進版 0.8.13，加入可記住帳密的手動網站同步、可調整根目錄／首頁／登入／登出與十個 API、匯入預覽及分類／機台多選管理，仍由既有 vnc_enabled 勾選啟用。網站帳密以 DPAPI 保存，PHPSESSID 只在當次工作使用；每次請求前等待 300ms，取得後嘗試登出。併入前輪更新提示用詞修正。執行 `Build.ps1 -EmptyCargoCache`，只交付 EXE、EXE 更新清單、原始碼及驗證文件並推送 main；使用者實測成功後才製作 NSIS，本輪不更新 NSIS 清單／驗收，不重製歷史 ZIP。下列「待下次發行」的用詞修正已納入本版。

- **待下次發行：更新提示用詞**：使用者要求將下載完成後的按鈕及安裝確認明確寫為重新啟動 LM_AI 應用程式，避免誤認為重啟電腦。本輪只保留原始碼與文件修改，不編譯、不進版、不打包，留待下次一起驗證與套用；現有 0.8.12 成品保持不變。

- **0.8.12 統一完成結果（最新）**：支援使用者指定的 `result.choices[0].message.content` 搭配同層 `sections`、`citations`，背景與串流共用解析。沿用先前打包推送授權，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller`，交付 EXE、NSIS、兩份更新清單及文件至既有 main。沿用 0.8.11 安裝、圖示及歷史 ZIP 界線。

- **0.8.11 完成回覆保留（最新）**：接續使用者回報與原先打包推送授權，修正正文獨存及單行英文五段格式。完成結果非空欄位優先；正文一致才補缺漏，不一致另存可展開原文。執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller`，交付 EXE、NSIS、兩份更新清單與驗證文件並推送既有 main；沿用 0.8.10 安裝、圖示與歷史 ZIP 界線。

- **0.8.10 結構化回覆（最新）**：使用者要求將本輪回覆解析修正打包後推送 Git。進版 0.8.10，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller`，交付 EXE、NSIS、兩份簽署更新清單、原始碼與驗證文件至既有 main。結構化欄位優先、編號舊格式備援，背景與串流完成結果共用解析；正文及重點可見，其餘可展開。沿用三花貓圖示、單純安裝、不附帶／檢查 WebView2、一般安裝不自動啟動；不重製歷史離線 ZIP、不恢復 SFX。Git 推送不代表內網網站已部署。

- **0.8.9 三花貓圖示（最新）**：使用使用者提供的 `icon.png`，原樣複製至 `assets/app.png` 並以 Convert-AppIcon.ps1 產生多尺寸 ICO。使用者授權進版、重新編譯與打包後推送；本輪選用 0.8.9，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller`，交付 EXE、NSIS、兩份更新清單與驗證文件。沿用單純安裝、不檢查 WebView2、不在一般安裝後自動啟動的約定；不重製歷史離線 ZIP，不恢復 SFX。來源圖片不重新繪製或裁切。

- **0.8.8 單純安裝（最新）**：使用者要求移除 SFX 測試包，NSIS 只安裝、不偵測或安裝 WebView2，一般安裝完成不自動啟動主程式。LM_AI.exe 保留原有 WebView2 初始化失敗提示，不新增偵測；既有更新 `/UPDATEPID`／`/RESTART` 交接契約保留。維持 0.8.8，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 後推送 main。使用子程序限定的無效 Runtime 位置驗證「安裝可完成、主程式啟動才失敗並提示」；不移除本機 Runtime、不宣稱已解決趨勢警報。只從最新 Git 移除 SFX 檔案，不改寫歷史，也不重製歷史 ZIP。以下較早的安裝前 Runtime 檢查約定由本條取代。

- **0.8.8 精簡 NSIS（最新）**：使用者同意拆出 WebView2，由公司另外提供。Setup 不嵌入、不下載或自動執行 Runtime；缺少時在替換前提示使用者取得 `MicrosoftEdgeWebView2RuntimeInstallerX64.exe`，以退出碼 2 停止。保留 Git 的獨立 Runtime。維持 0.8.8，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 並推送；測試缺少／零版本、HKCU／HKLM 偵測與既有檔案保留。不重製歷史離線 ZIP。以下隨安裝包提供 Runtime 的舊約定由本條取代。

- **0.8.8 NSIS 交付（最新）**：使用者已實測目前版本大致正常，要求製作 NSIS 並上傳 Git。維持版本 0.8.8，執行 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`，交付 EXE、Setup、兩份更新清單及驗證文件。安裝／更新／解除安裝須保留 EXE 旁既有的 VNC 設定。本輪不重製離線 ZIP；以下較早的「不製作 NSIS」限制由本條取代。

- **0.8.8 附件修正**：使用者回報 0.8.7 選檔後無附件請求；修正已移除刪除按鈕的殘留引用與接收控制項恢復，補上合法檔案完整接收／失敗重試自檢。沿用前輪 `Build.ps1 -EmptyCargoCache`、EXE 與 Git 交付範圍，不重製 NSIS 或離線 ZIP。

- **0.8.7 使用者最新指示**：精簡側欄／對話列操作、沿用黑貓圖示，更新 Outlook 說明及品質模型可用性檢查；先執行 `scripts/Build.ps1 -EmptyCargoCache`，交付並推送 EXE、EXE 更新清單、原始碼與驗證文件。使用者確認介面後才製作 NSIS，本輪不重建 NSIS 或離線 ZIP。使用者已實測 0.8.6 VNC 可正常運作，本輪未修改 VNC 啟動流程。

- **0.8.6 使用者最新指示**：先以本機已安裝 UltraVNC 實測正式呼叫流程，再進版 0.8.6，編譯並提交／推送 EXE、EXE 更新清單、原始碼與驗證文件。使用 `scripts/Build.ps1 -EmptyCargoCache -TestVnc`；本輪先不重建 NSIS 或離線 ZIP，舊包維持歷史版本。這取代前次「僅保存本機變更」及 0.8.5 完整 NSIS 交付要求。後續確認需要 NSIS 時再打包。
- VNC 預設隱藏，只有設定 `vnc_enabled` 啟用後才提供本機頁面。固定沿用 LM_AI.exe 旁 `machines.json`／`user_config.json` 的 Python 格式；機台以原陣列順序顯示，僅手動上移／下移，不自動排序，編輯保留原位置。原生層直接啟動已安裝的 vncviewer.exe，不使用 shell，不把既有密碼傳到前端或 AI。此使用者明確要求的功能為上述「其他自動化尚未實作」的例外；細節見 docs/VNC_QUICK_CONNECT.md。

- **0.8.5 使用者最新指示**：供應商顯示改為 `Largan, Inc.`；重建 NSIS，正式安裝路徑固定 `C:\largan\LM_AI`，逐層建立缺少的資料夾、保留既有目錄及未知檔案。沿用一般使用者權限及單一 LM_AI.exe，避免 Outlook 權限差異。此次交付原始碼、EXE、Setup、兩種更新 JSON 與驗證文件，執行 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`；未要求重新製作離線 ZIP。以下舊版路徑及不製作 Setup 的約定由本條覆蓋。

- **0.8.1 使用者最新指示優先**：本次不製作 NSIS 安裝包或離線 ZIP；以 `scripts/Build.ps1 -EmptyCargoCache` 完成 v142、測試、release 與 EXE 簽章驗證後，提交／推送 LM_AI.exe、清單及對應原始碼與文件。只保留 LM_AI.exe 主程式檔名，CompanyAI.exe 停用。雙格式更新以 docs/UPDATE_0_8_1_CONTRACT.md 為準；既有安裝包與 ZIP 保持歷史版本，不稱為本次交付。未來明確要求完整包時再使用下方完整交付流程。

- 使用者要求專案包含原始碼、已編譯 EXE、Rust 離線編譯包及附屬文件，這些產物必須保持同一版本。
- 完成原始碼、依賴、設定、腳本或交付文件修改後，執行 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Prepare-Delivery.ps1`。它會更新 vendor、執行 Build、製作 ZIP，並在全新解壓目錄使用包內工具鏈、空 Cargo 快取及 `--frozen` 再次驗證。
- 成功後交付 `dist/LM_AI.exe`、相同內容的相容檔名 `dist/CompanyAI.exe`、`dist/LM_AI_Setup.exe`、`dist/MicrosoftEdgeWebView2RuntimeInstallerX64.exe`、`offline/CompanyAI-offline.zip`、ZIP 的 `.sha256`、`offline/manifest.json`、`offline/verification.json`、`offline/environment.txt`，dist/update-manifest.json、offline/installer-verification.json，以及對應原始碼與文件。
- 檢查失敗時修正問題並重新執行；不要把舊 ZIP 配上新原始碼宣稱為完整交付，也不要略過驗證。
- 依賴變更須同步更新 Cargo.lock；先取得所需 registry 套件，再用 `Prepare-Delivery.ps1 -RefreshDependencies` 更新離線包。保持固定 Rust / MSVC 基準。
- Git 儲存庫以本專案資料夾為根，不把外層共用環境或 `.local-ai` 納入。
- `.gitattributes` 已為 `dist/*.exe` 與 `offline/*.zip` 設定 Git LFS。首次加入二進位檔前必須在儲存庫執行 `git lfs install --local`；確認 LFS 可用，避免把大型 ZIP 直接當一般 Git 物件提交。
- 使用者要求提交或上傳時，先完成上述交付流程，再將原始碼、EXE、離線包、校驗與文件一起提交；確認 LFS 物件亦成功上傳。提交前檢查暫存內容，不包含 API Key、登入憑證、個人設定、target、解壓後的 toolchain/vendor、快取或暫存目錄。
- 遠端儲存庫為 `https://github.com/you1230783-sys/CompanyAI.git`，主要分支為 `main`。使用者已授權首次完整交付上傳；後續依使用者的提交／推送指示操作，明確授權可沿用。不自行更換遠端、不強制推送覆蓋歷史。
- 操作細節見 `docs/OFFLINE_AND_GIT.md`。驗證記錄描述本機測試，不能代替公司實機驗收。
