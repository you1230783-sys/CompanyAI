# CompanyAI 專案指引

## 目前工作

- **0.9.19 本機完成：P1～P5／N1、577 項 Rust、18 組前端、完整原生功能回歸、真實 0.9.18 升級與完整安裝矩陣、v142 空快取 frozen 正式建置／封裝及本機封存均通過。來源最後產品修正 655bf12；詳細成品與分段驗證證據見 docs/validation/VALIDATION_0_9_19.md。未推送／公開發布；公司模型另驗。依最新指示到此暫停，0.9.20 未實作。**

- **2026-11-06 最新停止點：使用者改為完成 0.9.19 的完整驗證與封存後暫停，0.9.20 先不做。此指示取代下方接續 0.9.20 的授權；0.9.20 目前僅閱讀手冊，沒有程式實作。**

- **2026-11-06 追加授權：0.9.19 完整驗證與封存完成後，繼續依 CHANGE_HANDBOOK_0_9_20_EXCEL_VALUES_AND_RETRY.md 實作 0.9.20 E1／R1／R2。0.9.20 僅做手冊要求的相關驗證、v142 frozen 正式建置與基本封裝安裝驗證，不必重跑完整安裝生命週期矩陣。兩份手冊原文保留，逐版完成，不混合版本證據。**

- **2026-11-06 最新授權：依 CHANGE_HANDBOOK_0_9_19_PYTHON_AND_NOTES.md 接續實作 0.9.19 P1～P5／N1，執行完整驗證流程。0.9.18 的未實作 B／C 已抄入 FUTURE_IDEAS，不納入本版。手冊原文保留；進度及失敗記於 IMPLEMENTATION_PLAN 與 VALIDATION_0_9_19。取代下方 0.9.18 停止點，後續完成狀態見最上方 0.9.19 紀錄。**

- **0.9.18 已發布：v0.9.18 固定於 531efeeae9d40c3639a1f1c29870b8fade5a850e。X1／X2、M7混合stdout、禁止巢狀新增應用程式鎖，文件分類及新手冊／FUTURE_IDEAS一併交付。48項相關Rust、v142空快取frozen、相關原生Excel／Office／資源、候選NSIS與五項公開下載通過；驗證中發現的COM查詢及自檢HTTP伺服器問題已修正並記錄。僅相關驗證，未跑完整回歸或同來源hosted CI；原生與最終自檢候選的證據界線見 docs/validation/VALIDATION_0_9_18.md。到此停止，等待公司實測，不實作B／C。**

- **2026-11-06 最新授權：依 CHANGE_HANDBOOK_0_9_18_EXCEL_READ.md 實作 X1／X2，納入 M7 混合 stdout 與 Office／CAD 巢狀鎖補強，進版0.9.18。跳過完整回歸，只做相關單元／schema／原生 Excel／Office 與候選安裝包驗證，v142 x64 空快取 frozen 建置；完成後打包、Git與可下載安裝包交付。FUTURE_IDEAS.md 及新手冊一起提交，三份手冊均保留原文。此授權取代前一輪不編譯／不發布及停在0.9.17的界線；本版未做的完整項目必須明列，不宣稱完整驗收。**

- **2026-11-06 發布後補強：依使用者提供的 Claude 建議，修改 M7 的混合 stdout JSON 提示，以及 Office／CAD 鎖的巢狀擴充防護。僅本機原始碼與待驗案例，不編譯、不進版、不打包、不推送／發布；等待公司端實測回饋後再整合。已發布 0.9.17 與其驗證證據維持原樣，本次修改尚未編譯或執行測試；進度見 docs/IMPLEMENTATION_PLAN.md。**

- **0.9.17 已發布：v0.9.17 固定於 3b6c5c1838e8a0c7cdbdd8e5996af51b4b1c8b76，同提交CI 42159015062 通過。M11與Excel筆記／封存兩項修正，v142空快取frozen、492項非CAD Rust、80項前端、原生多任務／Office／Python／panic、實際Setup生命週期與五項公開下載通過。詳見 docs/validation/VALIDATION_0_9_17.md。依使用者指示到此停止，等待公司驗收回饋；不實作0.9.18。**

- **0.9.16 已發布：v0.9.16 固定於 6fd91557427bbe613be7a3085601bbcd33533bc0，同提交 CI 42151446993 通過。v142 空快取 frozen、481項非CAD Rust、65項前端、原生HTTP/DPAPI分支及專案交接、實際Setup生命週期與五項公開下載通過。詳見 docs/validation/VALIDATION_0_9_16.md；公司模型與附件跨對話沿用另驗。接續0.9.17，完成後停止。**

- **0.9.15 已發布：v0.9.15 固定於 33a07acb4a060997883dd9a0cbba202e4df05336，同提交 CI 42145350765 通過。v142 空快取 frozen、478 項非 CAD Rust、M9 前端／原生計畫及恢復／NSIS、五項公開下載通過。詳見 docs/validation/VALIDATION_0_9_15.md；公司模型另驗。接續0.9.16，最終做到0.9.17並停止。**

- **0.9.14 已發布：v0.9.14 固定於 77bf1bb3e0187c457fa86a7b3896301130498507，同提交 CI 42139510276 通過。v142 空快取 frozen、473 項非 CAD Rust、M8 前端／AppContainer、Office／NSIS 相關驗證與五項公開下載核對通過。詳見 docs/validation/VALIDATION_0_9_14.md，並非完整長任務／Python 科學矩陣。依最新授權接續 0.9.15，直到 0.9.17 發布後停止。**

- **2026-11-06 最新恢復授權：使用者要求繼續直到 0.9.17 完成（使用者已更正終點），取代先前暫停。依手冊先逐版完成 0.9.14～0.9.17，每版相關驗證、v142 空快取 frozen、必要封裝、Git／Release 及下載核對完成後才進下一版。不實作 0.9.18。公司驗收不阻止本機進度。手冊不修改，進度及結果記在 IMPLEMENTATION_PLAN.md 與各版 VALIDATION。**

- **0.9.13 已發布：v0.9.13 固定於 5f89ae0da50c4469332df69d3871ad7f06b0b377，同提交 CI 42127495093 通過。v142 空快取 frozen r1、468 項非 CAD Rust、32 組原生 HTTP、5 組前端、M7 真 AppContainer／通知自檢、Office／NSIS 與五項公開下載校驗通過。見 docs/validation/VALIDATION_0_9_13.md；真實模型比較仍留公司端；依使用者最新指示在本版發布後暫停，0.9.14～0.9.17 保留待辦，等待再次指示。**

- **0.9.13 本輪授權：依 docs/CHANGE_HANDBOOK_EIS_FEEDBACK.md 的 M7 與第二節「0.9.12 複核補充」兩項修正，完成後進版、v142 空快取 frozen 驗證／封裝並推送 Git 與 Release。M7、通用技能詞彙、暫停通知各自提交；真實模型比較仍由公司端驗收。本版完整驗證與發布已完成，結果見上項。**

- **0.9.12已發布：v0.9.12固定於42fc74f5a0f725426ce98afdf71da48224c69625，同提交CI 42053066119通過。v142空快取frozen r3、466項非CAD Rust、31組原生HTTP、5組前端、Python／Office／排程及NSIS生命週期完成，五項公開下載SHA256核對通過。先前檔案鎖／Schema與測試參數失敗已如實記錄，見docs/validation/VALIDATION_0_9_12.md。真實模型比較由公司端驗收，M4第二階段偵測另排；不自行進版。**

- **0.9.12 目前授權：依 docs/CHANGE_HANDBOOK_EIS_FEEDBACK.md 實作 M1–M6，每項獨立 commit 並記錄測試／限制，完成後進版、v142 空快取 frozen 編譯、必要封裝驗證、GitHub 與 Release 發布。使用者已明確指定真實模型 EIS 比較留待公司端驗收；本機用合成回歸，不宣稱公司資料已驗收。M4 第二階段數值不一致偵測另行排期，不變更 50K 輸入預算、不重建 checkpoint、不新增 AI 刪檔能力。**

- **0.9.11 已發布：Release／標籤指向來源提交 01bbda6cf36031a113e1b63fc1c01206e93d4611，同提交 CI 42003106319 通過。相同 EXE 再次以 v142 空快取 frozen 核對後完成 NSIS 安裝／更新／回復／解除安裝及 Python／CAD 安裝後自檢，五項公開下載 SHA256 通過。證據見 offline/release-verification-0.9.11.json、offline/release-download-verification-0.9.11.json；公司 EIS／DRT 仍待實測，不自行進版。後續只有文件與驗證收據提交，不移動已發布標籤。**

- **2026-11-05 後續授權：使用者明確要求發布 0.9.11 安裝包到 Release。接續已通過的來源驗證，封裝前以 v142／空快取 frozen 核對相同 EXE，補做 NSIS 生命週期、同來源提交 GitHub CI 與公開下載 SHA256；先前「不含安裝包／Release」僅描述來源交付階段，不再是禁止發布。**

- **來源交付階段紀錄：0.9.11已完成本機來源整合驗證：v142／空快取frozen、非CAD456項／核心472項Rust、14組前端、Python科學套件與版本串接、Office Markdown格式讀回、排程Excel占用安全略過。詳見docs/validation/VALIDATION_0_9_11.md，列有分段接續、兩個EXE雜湊與PDF彩色emoji文字抽取限制。當時交付Git來源與證據，尚未封裝；後續發布已完成，見上項。**

- **2026-11-05追加授權：使用者已回報一般功能大致正常，要求完成0.9.11全部待辦與Python科學計算串接修正，完成驗證後提交推送Git；不再等待公司端測試才開始實作。持續保留v142與離線驗證規則。**

- **先前階段授權（已由上項接續）：先實作0.9.11桌寵狀態／點擊與右鍵捷徑、對話／專案工具精簡、Markdown匯出與開啟資料夾、範本教學及排程安全略過／通知修正。完成必要本機檢查後等待使用者公司實測回饋；本輪先不封裝、推送或發布Release，不沿用0.9.10的發布授權。版本號與正式公告留待整合發行，工作清單見docs/history/IMPLEMENTATION_0_9_11.md。**

- **最新指示（2026-11-05，節奏調整）：0.9.8繼續完整驗證／封裝並發布Git與Release。0.9.9完成F08／F09／C01後只提交推送程式碼，明示未編譯／未驗證，跳過編譯、封裝及驗證、不發布Release；提交加[skip ci]避免自動CI。接著完成0.9.10 D03，再對包含0.9.9改動的整體版本完整驗證／封裝／推送Git與Release。每個commit詳列改動與證據狀態，供Claude二次驗證。**
- 0.9.8本機完整v142／空快取frozen、499完整／455核心Rust、Office／NSIS與九組前端已通過；28份證據見docs/validation/VALIDATION_0_9_8.md。同提交GitHub CI 41959105102及五項Release公開下載SHA256也已通過，v0.9.8標籤2b6ece3；0.9.9 F08／F09／C01已寫入來源與待驗案例，依指示未編譯／未驗證／不發布Release；來源提交b3bbfe9已推送。0.9.10 D03與0.9.9合併範圍已通過本機v142空快取完整511／核心467項Rust、十組前端、Office與NSIS；同提交GitHub CI 41976149119、v0.9.10 Release及五項公開下載SHA256也已通過，見docs/validation/VALIDATION_0_9_10.md。本輪已完成，等待Claude二次驗證與公司實測；不自行進版。
- 0.9.7已發布，tag為`5fb110822ca461d92df867da48e57cd4c93a9751`；本機v142空快取、499完整／455核心Rust、Office／NSIS、最終GitHub CI及五項公開資產SHA256均通過。
- 本輪順序：0.9.8 D02b → 0.9.9 F08／F09／C01 → 0.9.10 D03（僅app開啟／托盤）。C01為50,000 tokens軟目標、超標整理至約40,000，放寬原文移出，技能與筆記維持。M02擱置。先前停止、僅記錄及每版均完整驗證發行的節奏已由最新授權取代；細節見docs/IMPLEMENTATION_PLAN.md。

### 以下為歷史進度，不能視為目前的接續授權

- 0.9.7首次GitHub CI（41936574844）失敗：11項記憶原子替換共享衝突、2項ANSI語系假設。記憶目錄改READ_ATTRIBUTES並保留不分享DELETE／獨占anchor；補明確碼頁與目錄保護測試。300專案定向測試、499完整／455核心Rust及完整本機r2 Office／NSIS通過；遠端CI須核對同提交成功，發布r2新成品。

- 0.9.7修正後本機完整v142空快取、499項完整／455項核心Rust、Office與NSIS已通過，28份公開證據見docs/validation/VALIDATION_0_9_7.md。CI腳本與12項漏洞政策案例已驗；推送後須核對GitHub hosted CI才發布。ttf-parser停止維護公告保留有期限精確例外，不宣稱零警告。發布及公開下載核對後接續0.9.8 LTO。

- 2026-11-05追加0.9.9 F09：額外功能預設關閉，側欄以Outlook助理直達入口呈現；開啟後恢復進階功能入口。VNC獨立開關／外部入口保留；塔羅與桌寵教學受額外功能開關控制，VNC教學受VNC開關控制。與Word內容對齊同版實作，不提前混入0.9.7。

- 2026-11-05斷線後使用者確認繼續逐版完成／推送；無人值守先限定程式開啟／縮到托盤時執行，使用評價與統計明確擱置，不列為本輪必做或等待後端的阻塞項。

- 0.9.5已發布並核對公開下載。0.9.6已完成Cargo features、16種組合、核心453項／完整497項Rust、Office與NSIS驗證；正式安裝包預設全功能，自訂features只供BuildOnly且輸出獨立目錄。完成本版驗證／封裝／推送後再進行0.9.7 CI與漏洞檢查。

- 0.9.5已完成本機v142空快取、495項Rust、Office與NSIS驗證，COM生命週期統一、Windows綁定分工保留；證據見docs/validation/VALIDATION_0_9_5.md及docs/development/WINDOWS_BINDINGS.md。發布後接續0.9.6，後續仍逐版驗證／封裝／推送。

- 最新授權：依待辦順序逐版修改、完整驗證、封裝、推送main及Releases，再接續下一版，直到可完成項目結束。先前「本輪不提前開始」只屬0.9.4歷史範圍；本輪可繼續0.9.5之後版本。缺少公司後端／實機的項目如實記錄並詢問必要資訊，不宣稱完成，不動NAS備份。

- 0.9.4本機修改、純搬移核對與完整v142空快取／Office／NSIS驗證已完成（494項Rust）；證據見docs/validation/VALIDATION_0_9_4.md及docs/history/REFACTOR_0_9_4.md。後續Windows綁定、features、CI／漏洞、LTO依計畫分版，本輪不提前開始；Word內容對齊固定排在LTO之後的下一版。

- 2026-11-04本輪授權0.9.4：先修正批次panic保護一致性與Word位置比較提示，再純搬移拆分files.rs／ui.rs／runner.rs；每個拆分commit列出函式原位置→新位置及必要可見性／路徑調整。完整v142驗證、封裝後推送main及Releases。Windows綁定、Cargo features、CI／漏洞、LTO分開後續版本；Word內容對齊排在LTO之後的下一版。NAS備份由使用者自行處理。
- 0.9.3的F01～F07已完成本機實作、v142空快取完整建置、Office與NSIS驗證；證據見docs/validation/VALIDATION_0_9_3.md。公司模型翻譯品質與公司實機仍待驗。後續0.9.4依實作計畫處理維護工作，不混入已移除的Authenticode待辦。
- 2026-11-04 使用者調整並授權0.9.3一次完成F01～F07：匯出、Word翻譯、文件比較、流程定時提醒、預設工作區、Word原樣式副本翻譯及新建Markdown；逐功能提交，完整驗證封裝後推送main與Releases。0.9.2已發行，證據見docs/validation/VALIDATION_0_9_2.md。0.9.4才做純搬移拆分、Windows綁定／Cargo features、CI、LTO與漏洞檢查；以docs/IMPLEMENTATION_PLAN.md最新順序為準。
- 0.9.0本機驗證包已用v142空快取完成全Rust／原生整合與NSIS生命週期驗證；VNC入口、桌寵字級／描邊已納入。本輪已獲授權推送GitHub main；公司環境與桌寵人工DPI缺項見docs/validation/VALIDATION_0_9_0.md。
- 完整建置與必要封裝、相關驗證成功才可稱完成；公司實機缺項如實記錄。不能把舊發行授權當作本輪測試通過證據。
- 安裝路徑 C:\largan\LM_AI、公司HTTP、machines.json明文格式與匯入匯出、公司更新同源與簽章檢查均不變。使用者於0.9.1授權GitHub Releases與歷史清理：安裝包及更新JSON移至Releases，Git不再使用LFS；CI、LTO與漏洞檢查仍延後。
- 大檔案純搬移延後至獨立commit；不混入功能變更。
- main歷史已以0.8.75的`9f8febc`重建，後續原始碼保留至0.9.1；對照見offline/history-cleanup-0.9.1.json。不要將本機備份、舊clone或改寫前提交merge／force-push回main。
- 歷史授權與驗證已移至 docs/history/AGENTS_BEFORE_0_9_0.md；逐段對照見 docs/history/AGENTS_MIGRATION.md。

## 專案界線與程式碼風格

- 文件入口為 `docs/README.md`；開發方法放 `docs/development/`、功能放 `docs/features/`、串接放 `docs/contracts/`、CAD 放 `docs/cad/`、版本驗證放 `docs/validation/`、歷史實作放 `docs/history/`。當前進度仍記 `docs/IMPLEMENTATION_PLAN.md`；三份修改手冊留在 docs 根目錄且不改原文。新增文件同步更新分類索引。

- `Y:\Rust` 是共用根目錄，本專案是 `Project/CompanyAI` 內的獨立 Cargo workspace。原始碼、Cargo.lock、vendor、.cargo、target、dist 與 scripts 留在專案內。
- 不移動或修改其他專案、`.local-ai`、共用 `.tools`、安裝下載與共用環境腳本；不在共用根目錄建立 Cargo workspace 或 .cargo/config.toml。
- 優先使用明確命名、簡單控制流程與單一責任函式，避免不必要的抽象、泛型及巨集。以繁體中文註解說明用途、限制與設計原因；公開 Rust 介面使用 `///`。
- 錯誤訊息應說明原因與可採取的動作。修改邏輯同步更新註解與文件；不以無理由的 unwrap/expect 取代錯誤處理。

## 建置與驗證

- Windows 11 x64、MSVC **v142 / 14.2x**；已驗證 14.29.30133、`_MSC_FULL_VER=192930159`、SDK 10.0.19041.0。Rust 由 rust-toolchain.toml 固定。
- 使用 scripts/Enter-DevShell.ps1（vcvars64.bat -vcvars_ver=14.2）及 scripts/Build.ps1；確認實際 cl/link 與 x64，不使用新版 VS 預設工具集或 target-cpu=native。
- 離線依賴保留 Cargo.lock、vendor、.cargo/config.toml，以空 Cargo 快取及 --frozen 驗證。不要把歷史離線 ZIP 當成目前依賴。
- BuildOnly 僅編譯；ValidateOnly 不更新發行產物；BasicValidation 不代表完整驗證。Office、VNC、SMB、CAD 與 NSIS 依變更範圍及當次授權實測，缺少環境如實回報。
- 真模型、固定參數原生工具、幾何／內容及畫面核對分開記錄。保留原始參數與失敗，不由測試程式替模型補正；流程見 docs/development/LOCAL_MODEL_VALIDATION.md。
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
