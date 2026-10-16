## 0.8.68 恢復發行（2026-10-16 最新授權）

- 使用者要求完成編譯、打包與推送，取代下方暫停限制。使用 Build.ps1 -EmptyCargoCache -IncludeInstaller，完成 v142 x64、離線依賴及必要驗證後交付 EXE、NSIS、更新清單與原始碼至既有 main。
- 已重建新 CAD runtime；保留 icon.png、使用者 onboarding-state.json 與歷史離線 ZIP，不提交測試連線或聊天資料。最終結果見 docs/VALIDATION_0_8_68.md，不沿用舊驗證冒充新功能驗收。
- 本次完整建置結束碼 0：實際 v142 14.29 x64、空快取／--frozen、345 Rust、61 CAD 合成、專案／Python 整合及 NSIS 安裝生命週期通過。已更新 EXE／NSIS／兩份簽署清單；新 CAD 的公司實機效率與模型核對仍待驗收。

## 0.8.68 CAD 本機索引與局部預覽（2026-10-16 實作階段）

- 使用者同意實作一次建立快照、按幾何條件查詢、穩定候選編號、局部預覽與修改前核對。維持暫停編譯／打包／推送，這輪只改原始碼與文件。
- 新增 cad_snapshot／cad_query／cad_preview，作用中圖面與專案 DWG 副本共用本機索引。完整快照不送模型、不恢復作用中文件寫入授權；查詢結果才按需交付，details=true 才加入已讀 Handle。
- 品質模型可取得局部 JPG 並另走 analyze_image；快速模型移除圖片工具／方法說明且執行端拒絕。兩者均可顯示使用者候選預覽；指認不等於寫入同意。
- 預覽為 WCS XY 幾何示意，不是真實 CAD 截圖；僅線、圓、可讀直線折線。矩形辨識、距邊計算與未知屬性限制見 docs/CAD_SNAPSHOT_QUERY.md。
- 本批僅格式整理與人工程式檢閱，未新增／執行測試，未編譯或實機驗證。先前 345 項測試與 Luna 實測不涵蓋本批。恢復建置時先 python tools/build_cad_runtime.py 更新鎖定 runtime，再按 v142 指引完整驗證。

## 0.8.68 暫停發行，先討論 CAD 按需查詢（2026-10-16，後續已授權實作）

- 使用者要求停止編譯，先討論一次讀取建立本機索引、依區域／圖元條件查詢，再決定是否併入本版；目前不實作這項新設計，不繼續打包或推送。
- 本次建置已停止；Rust 345 項、release 主程式、CAD 61 項與專案整合通過，Python 全套及 NSIS 尚未完成。dist／offline 產物未更新，未提交／推送。
- 原始碼暫定版本已改為 0.8.68；保留現有修改與 Git 暫存內容，最終驗證待設計決定後重跑。詳見 docs/VALIDATION_0_8_68.md。

## 0.8.68 發行（2026-10-16 先前授權，已由上方暫停要求取代）

- 使用者要求編譯、進版並推送 Git，取代先前暫不發行限制；進版 0.8.68，交付 EXE、NSIS、簽署清單與原始碼至既有 main。
- 使用 Build.ps1 -EmptyCargoCache -IncludeInstaller，以 v142 x64／--frozen 做完整核心與 NSIS 驗證。保留歷史離線 ZIP、icon.png 及使用者 onboarding-state.json；不納入測試連線檔／聊天資料。
- 本版 CAD 分頁實測無重讀，但每頁掃描成本、模型誤填版本及剖面屬性過度確認仍是已知限制。發行結果以 docs/VALIDATION_0_8_68.md 為準，不宣稱公司模型已完整驗收。

## localhost Luna 實測（2026-10-16 最新授權）

- 使用者授權 localhost 轉接到 Codex GPT-6 Luna low 並實際操作應用程式核對 CAD 分頁；允許為測試建立隔離 EXE，仍不進版、封裝或推送。
- 新增 Start-LocalLuna.ps1、原生 dynamicTools 轉接與 --local-ai-test／--local-cad-probe 隔離入口；只綁 127.0.0.1，使用獨立設定與合成資料。
- v142 x64／空快取 --frozen 基礎建置與 26 項 progress 測試通過；target 已更新，dist 未動。645 圖元法蘭圖已存檔重開核對。
- 兩次 GUI 真正模型／CAD 測試均成功讀完 645 圖元、offset 0→100→…→600，無分頁重讀。另補齊四個 CAD 工具漏掉的 progress_note，6 項 schema 測試及第二次完整基礎建置通過。
- 第二次需人工更正模型誤填的年份 version 為 ProgID；最後雖交付核對表，仍把未讀出的 Hatch 屬性稱為符合，答案正確性不通過。僅驗證分頁／筆記流程，詳見 docs/LOCAL_LUNA_TEST.md、docs/VALIDATION_LOCAL_LUNA.md；不宣稱完整 CAD 驗收。

## 待發行：CAD 0／100 分頁循環（2026-10-16）

- 使用者明確觀察 offset 在 0／100 間反覆；補上獨立 CAD 閱讀區間與固定提供模型的 cad_paging，重讀同頁不算新進展。
- 同文件同 revision 重讀首頁保留累積 Handle；文件／內容變更仍重設。詳見 docs/PENDING_CAD_PAGING.md。
- 沿用先改、不編譯不進版安排；新增 Rust 回歸未執行，未封裝推送，實機與模型效果待驗證。每頁全圖掃描效能問題本輪未修改。

## 待發行：只保留本版更新日誌與教學（2026-10-16 最新要求）

- 取代下方跨版本摘要修正：取消更新後自動公告、版本區間追蹤與 NSIS 舊版本轉換紀錄，只保留設定按鈕開啟的本版更新日誌對話框。
- 移除設定下方重複日誌及內嵌 0.8.65／0.8.66 公告；往後 release-notes.json 每次發行替換為本版內容，不累積歷史。完整開發紀錄留在文件。
- 保留全部教學與已讀紀錄，舊 JSON 的版本欄位忽略。先改、不編譯、不進版、不封裝推送；教學 DOM 測試通過，Rust／原生／NSIS 尚未驗證。見 docs/PENDING_ONBOARDING_UPDATE_RANGE.md。

## 待發行：教學公告與更新版本起點（2026-10-16，已由上方簡化方案取代）

- 最新要求先修改、不編譯不進版；本輪不接續打包或推送。
- 檢查 0.8.65～0.8.67 使用者摘要，移除開發用離線測試與內部技能拆分公告；14 張教學卡保留 ID／revision 與已讀紀錄。
- 升級重新設定公告起點，避免舊 pending_from 導致 0.8.65 → 0.8.67 顯示為 0.8.64；同版本已讀不重現。新增 Rust 案例未執行，詳見 docs/PENDING_ONBOARDING_UPDATE_RANGE.md。

## 0.8.67 完整補測進行中（2026-10-16）

- 使用者最新要求：CAD 測完後補跑之前略過的完整驗證，全部通過才打包及推送 main。沿用版本 0.8.67；本輪目前只修改測試，不改產品功能。
- 已修正工具數量、技能拆分、教學保存目錄隔離與四分頁測試；342 Rust、61 CAD 合成、完整專案／Python／前端／本機 VNC 通過。完整核心命令 `Build.ps1 -EmptyCargoCache -ValidateOnly -TestVnc` 結束碼 0；另一次 Python 全套含 Excel COM 通過。
- 尚有使用者的「文件1 - Word」開啟，已透過非同步問題請使用者儲存並關閉 Office，未收到答覆；不代為關閉。Test-Office.ps1 要求 Office 全部關閉，剩下全格式 Office／圖片、SMB 文件、NSIS 生命週期及發行待續。
- 本輪未更新 dist／簽署清單、未提交推送。詳細證據與待辦見 docs/VALIDATION_0_8_67_FULL.md；不要把核心通過宣稱全部驗收完成。保留 icon.png、dist/onboarding-state.json 與既有文件。

## 0.8.67：CAD 離線實機測試與相容修正

- 加入不需公司登入的固定離線測試入口，沿用正式 Broker／COM，測試後清理成功產生的 DWG，保留報告。
- 修正 2022 標準線型檔搜尋路徑；2008 縮放核對容許浮點末位差異，記錄尺寸文字首次可讀，仍拒絕實質變更。
- 缺少 COM 型別庫明示需修復安裝，不誤報模型參數錯誤。本機 2008 登錄經使用者允許後修復，不隨更新自動改登錄。
- 基礎驗證、NSIS及兩版本固定資料流程；外觀、公司AI服務、完整安裝生命週期仍待驗收。

## 0.8.66：解釋與教學目錄

- 一般對話新增「解釋」快速動作，沿用快速動作模型設定。
- 設定新增「教學與更新日誌」，14個短主題獨立記錄已讀，可單張或整組重看；更新日誌可重看。
- 第三輪CAD測試因超時暫停，未視為通過；不處理上游排隊問題。本版採v142 x64基礎驗證與NSIS，完整驗證待補。

使用者已授權本輪進版、編譯、NSIS及推送main；保留icon.png與歷史離線ZIP。

## 0.8.65：技能分組、跨任務精簡與初階教學

- CAD依新建／目前圖面／副本／Excel分開載入；Python、LOG、圖表與Outlook精簡入口、詳細方法按需讀取，共用說明去重。
- 跨任務舊答案改成摘要與查回參照，歷史／記憶共用5000估算token預算，保留近期使用者原文；空白length回覆保存進度並暫停自動重送。
- 登入後一張一般對話教學卡，設定可重看；EXE旁onboarding-state.json分開記錄教學與更新摘要，自本版開始列跨版本更新。
- 程式碼唯讀批次搜尋及Python靜態分析擴充；CAD同worker連線後執行計畫，引線文字錨點放在線上方。
- 使用者第三項CAD測試連續超時三次後手動停止，未視為通過；本版做v142 x64基礎驗證與NSIS，完整安裝生命週期及公司CAD／服務實測待補。

## 待發行：跨任務上下文預算

- 使用者同意將近期三輪的完整助理答案改為摘要與原文查回參照；近期使用者原文及本次要求不截斷。歷史／記憶共用5000估算token預算，使用者原文過大時明示例外，不刪限制。方法筆記去重，詳細結果按需read_task_result。
- 原生回覆length且沒有文字／工具時保存進度並暫停，不自動原樣重送；模型實際token數仍以服務端為準。見docs/PENDING_PROJECT_CONTEXT.md。
- 本輪僅修改與格式整理，未測試、未進版、未編譯、未打包推送；等待第三項CAD實測後再決定發行。

## 待發行：一般對話教學／更新摘要／引線文字

- 使用者要求一張一般對話入門卡、登入後顯示、可重看，EXE旁獨立教學旗標；新增跨版本簡短更新摘要，自0.8.65開始，不補舊公告。教學revision獨立於應用版本，未要求不自動新增教學。
- NSIS成功安裝記錄舊EXE版本，應用更新前保存目前版本；一般對話／設定／專案摘要依記錄分組，沒有可靠舊版不猜測。CAD引線MText固定左下錨點後放在線上方1mm。
- 本輪使用者明確要求先改、不進版編譯；全部未測試／未打包推送，仍等第三項CAD實測。見docs/PENDING_ONBOARDING.md。

## 待發行：CAD 開啟／重連

- 使用者回報任務中開啟／重連無效，需先開CAD再另起任務。已修正探測worker結束後第二個worker重取COM的流程，改同一程序連線並沿用app完成計畫；使用者重連後有界等待就緒，不重播部分繪圖。
- 實際失敗報告待提供，尚未證明全部原因或完成實機驗收。見docs/PENDING_CAD_CONNECTION.md；與程式碼分析修改先保留，未打包推送。

## 待確認：程式庫搜尋限制

- 使用者回報應為0.8.53；程式碼唯讀於0.8.54才加入，舊錯誤與舊白名單一致。本批擴充唯讀程式格式、search_files純程式碼每批100檔、run_python(kind=text)純程式碼32份及研究指南。保留單檔／合計上限、來源保護與不執行來源。見docs/PENDING_CODE_SEARCH.md；本批暫不進版編譯推送，驗證待補。

## 0.8.64：原生線型與目前圖面純縮放

- 新建線／圓／公式曲線可指定 line_style；既有線／圓／弧可修改原生 Linetype／LinetypeScale。沿用已有定義，缺少時查 CAD 安裝目錄標準 LIN 檔，不修改全圖 LTSCALE。線型外觀與舊版介面仍待公司實測。
- 目前圖面純 ZOOM E／A 不依賴 StartUndoMark；仍保留確認、文件／revision 與指令前後可讀內容核對，不儲存不關閉。圖元修改與混合操作仍要求完整 Undo 群組，不以縮放例外繞過。
- 公司實測 0.8.63 的程序身分備援及目前圖面讀取成功；其 ZOOM 被 Undo 前置檢查阻擋，不能視為縮放已驗收。
- 使用者已授權本版進版、v142 x64／空快取 --frozen 基礎驗證、NSIS 及推送 main；完整驗證與公司 2008／2022 實測待補。保留 icon.png 及歷史離線 ZIP。

## 待下次發行：CAD 線型

- 使用者要求先修改線型，同時實測 0.8.63。本批只改原始碼／文件，不進版、編譯、打包或推送，等實測回報後再合併發行。
- 支援新建線／圓／公式曲線的 line_style，以及已讀線／圓／弧的 linetype 修改；保留目前圖面確認、Undo、不儲存契約，不改全圖比例。細節與待驗項目見 docs/PENDING_CAD_LINETYPES.md。
- 0.8.63 公司實測目前圖面身分備援通過，但純 ZOOM 卡在缺 StartUndoMark。併入純縮放不要求圖元 Undo 群組的修正，保留文件／revision、確認及指令前後核對；實際圖元修改與混合計畫不放寬。本批仍暫不打包，實機縮放尚待測試。

## 0.8.63：目前文件程序身分備援與診斷

- 使用者已回報0.8.62在公司iCAD2008新建／儲存正常；目前文件唯讀成功，ZOOM被桌面身分檢查阻擋，尚未執行縮放。不能據此宣稱所有CAD或指令已驗收。
- 缺少COM HWND時，查詢Windows程序完整路徑與建立時間；只接受與CAD回報路徑相符的唯一可查詢程序，且有桌面視窗，再搭配唯一文件名稱／路徑、內容快照與執行中COM文件物件核對。多程序、缺權限、查詢逾時或文件歧義仍唯讀。備援不是文件專屬HWND，使用者確認與操作時勿切換文件仍必要。
- 程序查詢使用PROCESS_QUERY_LIMITED_INFORMATION；不提升CAD權限。Documents列舉每次重設，避免已用盡列舉器誤判文件不唯一。報告加入identity_reason，不把所有阻擋都說成使用者視窗問題。
- 縮放等全圖指令只需目前文件／revision，不需讀完100筆以上的所有圖元或整份工作紀錄。editable_session=false時直接回報原因，不呼叫cad_active_edit、不反覆回讀；使用者要求不儲存時不得自行改成副本存檔。
- 目前圖面仍經確認、Undo群組、不儲存不關閉；程序重啟／文件切換／快照變更須重讀。本版不調整AI推論連線重試，使用者紀錄中的五分鐘連線失敗另待服務診斷。
- 沿用v142 x64、空Cargo快取／--frozen、基礎驗證與NSIS／main交付；完整驗證與公司目前圖面指令實測待補。

## 0.8.62：舊版縮放與指令能力前置檢查

- 佑達 iCAD 2008 的 ZOOM E/A 改用 Document.ActiveViewport；GstarCAD 維持 Application 方法。方法存在不等於實機功能已驗收。
- validate_commands 與 prepare_annotation_styles 分離；能力檢查只查詢方法、不執行指令。報告區分 AVAILABLE／UNSUPPORTED／DEFERRED_NO_DOCUMENT，並明示 executed=false。
- 有現成文件時，在建立新圖前唯讀檢查指令。沒有文件則在新空圖建立後檢查；若不支援，保留空圖並允許移除該指令後另建，不接續修改空圖。既有圖面在 Undo／樣式／圖元修改之前檢查。
- 未執行指令的已知缺介面錯誤不一律 manual_check。新圖計畫確認含不支援指令後不可原樣重送；移除或更換後可重試。RPC／逾時等未知結果仍不放行。
- 沿用 v142 x64、空快取 --frozen、基礎驗證與 NSIS／main 交付；完整驗證與公司 CAD 實機待補。

## 0.8.61：引線／剖面修正與受限 CAD 指令

- iCAD 原生 AddLeader 的第二參數使用註解字串；追蹤本次同步建立、內容及端點相符的唯一文字，設定字高3、樣式與顏色，不另建重複文字。2022 保留相容介面。
- IntelliCAD SelectionSet.Delete 會刪除成員；暫存剖面選取集先 Clear 並核對 Count=0，再 Delete。清空失敗不刪除，保留圖面。
- 每次 COM 列舉重設 IEnumVARIANT；報告記錄逐操作新增／移除及預期 Handle。缺少圖元時帶出對應操作，不猜測其身分、不忽略集合差異。
- operations 可加入 {"kind":"command","command":"ZOOM E"}；支援 ZOOM E/EXTENTS、ZOOM A/ALL、REGEN、PURGE（或 -PURGE A * N）、AUDIT N，可有英文指令底線／點前綴。指令置於本批幾何操作後。
- 指令透過固定同步 COM 方法執行，未開放任意 SendCommand／LISP／Shell／載入程式／儲存／關閉／炸開。缺介面即停止。操作目前文件沿用確認、Undo與不儲存；副本沿用另存重開核對。
- PURGE 清理所有未使用定義（不僅圖層／圖塊）；報告列出圖塊、圖層、線型、文字／尺寸樣式差異，其他表格不在明細範圍。不是炸開圖塊，也不刪除使用中的幾何。
- 維護指令前後核對 ModelSpace／PaperSpace 可讀圖元內容，未驗證完整圖塊定義／不可讀屬性。AUDIT N 只檢查不修復，COM返回不能宣稱圖面沒有錯誤，詳細診斷請看 CAD。
- 請分別實測 2008／2022；合成測試不等於公司實機驗收。

## 2026-10-15：0.8.60 iCAD 尺寸樣式原生介面

- 使用者回報0.8.59在第9步建立尺寸樣式時缺少DimStyles，已有8個圖元但未儲存。舊版改用DimensionStyles.Add與DimTxt／DimAsz等原生屬性，無CopyFrom；新版相容介面維持。一般文字不依賴尺寸集合。
- 新圖在全部幾何之前準備樣式；目前圖面／副本在任何刪改之前準備，保留Undo與不儲存契約。樣式前置作業仍可能建立新空圖／樣式，失敗不能宣稱完全沒有寫入。
- 原生尺寸、原生引線保留；未知額外圖元不忽略、不重播部分圖面。實機仍需使用者測試。
- 沿用v142 x64、空快取／--frozen、基礎驗證、NSIS與main交付授權。README開頭最多保留三版；保留icon.png與歷史ZIP。

## 2026-10-15：0.8.59 CAD 目前圖面與標註相容修正

- 使用者回報 iCAD 2008 Document.HWND 不支援、額外一個圖元阻擋儲存及標註樣式忽略設定。沿用基礎驗證、v142 x64／空快取／--frozen、NSIS 與既有 main 交付授權。
- 缺文件 HWND 改探測應用視窗、唯一文件名稱／路徑、程序建立時間與快照；完全無可用視窗仍可唯讀，不授權目前圖面寫入。介面不支援錯誤在同任務禁止同版本重試，錯誤不呈現為讀取成功。
- 獨立樣式設定尺寸／引線字高3、箭頭3、ISO文字對正、偏移1、simplex.shx；沿用 BigFont 或尋找已安裝 chineset.shx，不保證中文字形。既有樣式不改寫，暫時尺寸變數還原。
- 2008／2022 保留原生引線並套用獨立樣式；只有 COM 明示關聯、此次新建的空白／同文字註解才納入預期集合。逐操作記錄新增圖元，未知額外圖元仍嚴格拒絕，原本 extra Handle 的實際類型尚待實機報告，不能宣稱已證明成因。
- 可讀尺寸／原生引線只開放刪除後重建；仍需已讀 Handle、未鎖圖層及目前圖面確認／Undo。不得任意刪除未知圖元。
- 保留 icon.png、歷史 ZIP 與共享環境；完整 CAD 實機及安裝生命週期驗證待補。

## 2026-10-15：0.8.58 目前圖面與 Excel 座標（最新授權）

- 使用者要求實作共用 CAD 文件操作與 Excel 座標繪圖兩階段；CAD 指令先不做。完成後進版、編譯並推送既有 main，沿用基礎驗證與 NSIS 交付。
- 目前文件讀取包含未儲存內容；寫入由原生視窗確認文件、固定身分／快照、每輪 Undo 群組。不儲存、不關閉，不恢復舊任務的目前文件授權；成功 EDITED_UNSAVED 不當作磁碟 DWG 成果。
- 共用副本／目前文件修改引擎，增加半徑、文字高度、簡單圖元刪除與既有全部新增操作。Excel 先原檔版本／欄位讀取，本機建立有界座標計畫，預覽後套用new/active/copy，來源與單位再核對。
- 仍需 v142 x64、空 Cargo 快取、--frozen、必要基本檢查與 NSIS。公司2008/2022 HWND、Undo、未儲存逐輪修改、Excel到CAD實機未驗收；不以合成測試代替。保留 icon.png、歷史 ZIP 與共享環境。

## 2026-10-14：0.8.57 CAD 連線與既有 DWG 副本（最新授權）

- 使用者確認 %%c10 在 iCAD 2008 可顯示直徑符號；同意一起實作連線對話框、圖元核對診斷及既有 DWG 限定副本閱讀／修改。
- cad_read 將來源鎖定複製後讀取 ModelSpace，本機索引分頁；cad_edit 僅修改已讀取 Handle、來源 revision 一致的全新副本。支援文字／ACI／位移與新增線圓文字引線；圖塊僅名稱位置屬性摘要，不展開、重構、OCR 或修改巢狀物件，不接管使用者已開文件。
- CAD 連線由原生待答事件處理開啟／重試／取消，期限十分鐘；已部分操作斷線須由使用者明確選擇另建副本，不從未知位置接續。原檔不交給 COM，任務恢復仍核對來源及索引。
- %%c 對數字前的直徑符號正規化；Count 不一致只有完整 Handle 集合／屬性吻合才可接受並記錄，未知額外圖元不忽略。
- 沿用測試版 v142 x64、空快取／--frozen、基礎驗證與 NSIS／既有 main 交付，完整驗證及公司 CAD 實機待補。不得把合成 COM 測試稱為實機驗收。保留 icon.png、歷史離線 ZIP。

## 2026-10-14：0.8.56 CAD 復原與佑達 iCAD 修正

- 使用者實測 2008 已成功連線、矩形／圓／尺寸，第一條引線因 Points.Item 索引失敗；修正為 Points.Add(x,y,z)，不宣稱此修正已完成公司實測。
- 尚未建立圖面可補正重試；已回覆 E_INVALIDARG 的部分操作可修改計畫另建圖面，不修改／關閉部分文件。相同部分執行計畫不能換檔名重播，每任務最多六次執行，未知結果仍鎖定；舊版未知狀態不能自動解除。
- 預定路徑不是磁碟成果。file_exists、saved、completed_operations 與交付驗證分開；更新模型說明避免要求使用者開啟不存在的 DWG。
- 支援產品明示浩辰 GstarCAD／佑達 iCAD，保留已知身份及實際路徑核對，不限制安裝資料夾品牌名。
- 使用者授權跳過完整驗證、v142 x64 基礎編譯與 NSIS 打包後推送既有 GitHub you1230783-sys/CompanyAI main，包含原始碼、EXE／NSIS、驗證文件與 LFS。保留 icon.png、歷史離線 ZIP。

## 2026-10-14：0.8.55 AI CAD 繪圖測試版

- 使用者確認獨立測試的 2022 引線箭頭正常，要求直接整合專案 AI 對話繪圖並加入顏色；進版、v142 x64 編譯與 NSIS 打包，只做基礎驗證，完整驗證延後。
- 新增 cad-drawing 技能、cad_inventory 與 cad_draw。只新建 DWG，修改需求重建另一份成果；不操作使用者目前文件、不做 Solid Edge。年份新版優先，不明年份不猜測，失敗不偷換版本。
- 支援線、圓、尺寸、文字、多行文字、帶箭頭引線、ANSI31 剖面線、受限數學公式取樣線段，以及每項 ACI 1–255 顏色。新增的顏色與應用整合尚待公司實機驗收，不把獨立工具的成功擴大為新版全部功能已通過。
- CAD 工作程序採獨立鎖定 CPython／pywin32，固定 JSON 操作，不執行模型 Python／任意 COM。原分析 Python 與 AppContainer 不變。上游預編譯品不宣稱為本機 v142 編譯。
- 以 Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller 交付；新增 CAD runtime 完整性與唯讀管線自檢。完整 Office／SMB／NSIS 安裝回復驗證延後。保留 icon.png、歷史離線包與既有 Git 遠端；交付後使用者追加授權推送既有 main。
- 詳見 docs/CAD_DRAWING.md 與 docs/VALIDATION_0_8_55.md。
- 2026-10-14 19:48 基礎交付完成：v142 x64／空快取／--frozen、41 項選定 Rust 測試、10 項 CAD 合成契約、正式及 dist CAD runtime／唯讀管線、既有基礎 UI／Python／程式唯讀、NSIS 打包與簽署清單核對通過。公司 CAD 繪圖、AI 實際規劃及完整安裝驗證仍待實測；交付後追加授權提交／推送。

## 2026-10-14：CAD 0.1.1 實機回報修正

- 使用者回報佑達 ICAD 2008 已連接，繪圖在產品身份檢查停止；只更新獨立工具身份規則與 ZIP，沿用 Git 交付及基礎自檢範圍。CompanyAI 0.8.54 EXE／NSIS 不變，不重編 Rust。實際繪圖仍待使用者重測。

## 2026-10-14：0.8.54 程式碼唯讀與獨立 CAD 測試（最新授權）

- 使用者要求開放專案程式碼唯讀，並交付浩辰 CAD 2008／2022 的獨立診斷、新建圖面及尺寸標註測試。預設已確認年份由新到舊（2027 > 2026 > … > 2022 > … > 2008），保留手動選擇；Solid Edge 不做。
- 完成後進版、v142 x64 編譯、NSIS 打包與推送既有 Git main；本輪只做基礎驗證，完整驗證等待使用者實測。使用 scripts/Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller；必須核對實際編譯器、空 Cargo 快取與 --frozen。
- 新程式格式只有閱讀能力；PY 保留既有副本編輯。一般聊天附件上傳契約不變。CAD 工具獨立打包，不接入 AI，不改分析用 Python；上游 Python／pywin32 預編譯品不能宣稱本機 v142 編譯。實機 CAD 成功須以公司報告為準。
- 保留 icon.png、共用環境及歷史離線 ZIP。詳見 docs/VALIDATION_0_8_54.md。
- 2026-10-14 基礎交付通過：v142 x64／空 Cargo 快取／--frozen、25 項選定測試、原生唯讀流程與 NSIS 簽署清單核對；CAD 32／64 位元本機自檢通過，實際 2008／2022 尚未測試。完整驗證暫緩。

# 專案級開發指引

## 2026-10-14：0.8.53 補做完整驗證（最新授權）

- 使用者回報測試 OK，要求接續完整驗證；取代下方暫緩完整驗證的安排。維持 0.8.53，使用既有 `scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller`，指定 v142 x64 並核對實際編譯器。
- 補跑完整 workspace、專案／Python 隔離與長流程、Office／本機 SMB，以及 NSIS 安裝／更新／回復／解除安裝。依實際結果更新驗證紀錄；使用者的「測試 OK」不擴大解讀為所有公司環境與 PDF 格式均已驗收。
- 沿用既有 Git main 推送授權；保留 icon.png、共享環境、Python runtime 指紋及歷史離線 ZIP。
- 2026-10-14 16:05 完整流程通過：v142 x64／空 Cargo 快取／--frozen、326 項單元測試、原生與長任務／Python／Office／本機 SMB、正式 NSIS 安裝更新回復解除安裝。前端 8 組圖表與 31 項分析／比較重跑通過。沒有修改功能程式碼，EXE／NSIS 雜湊與使用者測試包完全相同，維持 0.8.53；詳見 docs/VALIDATION_0_8_53.md。

## 2026-10-14：0.8.53 基礎驗證測試包（最新授權）

- 使用者提供網站調查：疑似POST413在建任務前拒絕，桌面續查by-request才衍生404；415網站已放寬PDF開頭判斷，實際加密封裝原因仍未確認。
- 已改為授權進版0.8.53、編譯及NSIS打包，僅做基礎驗證，等使用者公司測試後再補完整驗證；取代下方不編譯安排。建置完成後使用者追加授權推送既有Git main，包含原始碼、產物與基礎驗證紀錄。
- 使用 `scripts/Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller`，仍必須指定v142 x64、實際編譯器探針及空Cargo快取／--frozen。執行fmt／Clippy、聊天及附件相關測試、release、WebView2／原生任務停止自檢、Python runtime必要檢查、NSIS打包及簽署清單核對。完整Office／SMB／專案／Python長流程及NSIS安裝更新回復解除安裝待補。
- 明確413等拒絕直接保留原HTTP錯誤並停止；POST等待中不先查任務。傳輸結果未知或408／未明示拒絕的5xx保留原ID查回，再受三次相同錯誤＋models健康條件限制，不自動重送。
- 報告標示PASS_BASIC與完整驗證未完成，不沿用舊版完整驗收聲明。保留icon.png、共享環境、Python runtime指紋及歷史離線ZIP。
- 2026-10-14 14:52 基礎交付完成：v142 14.29.30133 x64／_MSC_FULL_VER=192930159、空Cargo快取／--frozen、fmt／Clippy、21項聊天附件測試、正式EXE及5組原生任務追蹤自檢、Python runtime及NSIS更新清單簽章核對通過。圖表8組與分析／比較31項前端檢查通過；完整驗證與公司端實測待補；後續依追加授權提交並推送Git。產物及雜湊見docs/VALIDATION_0_8_53.md。

## 2026-10-14：任務停止追蹤與介面整理（待合併發行）

- 使用者要求先修改程式碼，等網頁端調查 PDF 415／任務404後再合併處理。本輪不編譯、不進版、不打包、不提交或推送，保留0.8.52既有產物及更新清單。
- 一般對話任務 GET 連續三次相同錯誤，且當輪模型清單回200及有效內容時，自動停止本機追蹤。依實際HTTP狀態計數；無HTTP狀態則比對本機錯誤。成功、改用遠端任務ID或手動重送歸零。停止保存已收文字及DPAPI停止狀態，不另查詢／取消／重送未知工作。正式failed／cancelled仍立即收尾。
- 「分析依據與處理範圍」可摺疊，答案完成預設收起；同對話重繪保留手動展開。圖表移除上方文字清單及編輯文字區塊，工具列保留新增文字，圖內雙擊編輯／刪除。
- 可執行不需編譯的前端互動與Rust格式檢查；新增Rust測試留待後續指定v142 x64建置執行。詳見docs/PENDING_TASK_TRACKING_AND_UI.md，不把瀏覽器檢查當作原生／公司端驗收。

## 2026-10-12：Outlook Table 原生排列修正（目前階段）

- 使用者回報0.8.52仍出現「Outlook Table批次大小不符」，要求先交付可立即執行的小程式，回傳實機排列後再決定修正，避免反覆完整編譯。
- 已收到公司實機報告：Outlook16.0.0.17932、64位元、3677列；原生SAFEARRAY第一維為列、第二維為欄；GetArray(1/100)分別為1/100×5及1/100×9，ConversationID不支援、必要欄數正確。
- 依實測修正原生維度與GetElement索引，維持每批100列；補上真實方向、正方形批次、非零下界與異常尺寸的回歸測試。錯誤列出實際尺寸，不以寬鬆裁切掩蓋欄位錯誤。
- 使用者最新要求跳過剩餘驗證、直接發布NSIS。已完成v142實際編譯器探針、空Cargo快取／--frozen、fmt／Clippy、319項單元測試、release與部分整合檢查；停止未完成的長流程，不再重跑Office／SMB／完整安裝驗證。維持0.8.52，打包NSIS、核對簽署清單並推送既有Git main。
- 此次交付不能宣稱完整驗證通過；先前13:59驗證紀錄對應先前產物。scripts/Test-OutlookTable.ps1保留為唯讀診斷工具。修正版在公司Outlook的完整讀取結果仍待實測，不把尺寸診斷當作整體功能驗收。

## 0.8.52：分析交付、內容比較與方法重用（最新授權）

- 使用者已要求合併 Outlook ConversationID 修正與五項優化，重新進版0.8.52、完整編譯、NSIS打包並推送既有Git main；取代下方等待Python實測的暫停安排。
- 五項為：原生處理完成摘要、可查原文的LOG事件時間表、唯讀左右紅綠內容比較、可選用及調整的方法重用、文件搜尋範圍與原文定位。沿用既有來源版本、DPAPI、副本及權限。實際Office頁面排版預覽、逐項接受／拒絕及通用CLI不在本版。
- 比較固定建立副本時的基準與選定成果，舊檔無基準不補造；格式／結構變更另列。方法重用須重新核對新來源，不直接重播舊程式。
- 使用 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller，指定v142 x64、空Cargo快取／--frozen及完整安裝驗證通過才發布。保留icon.png、共用環境、Python runtime及歷史離線ZIP。

- 2026-10-12 13:59完整驗證通過：MSVC14.29.30133／_MSC_FULL_VER=192930159 x64、空Cargo快取／--frozen、314項單元測試、20項預覽互動與三種版面、實際來源／固定副本比較、Python／Office／SMB與完整NSIS安裝更新回復解除安裝。首次安裝自檢暴露的測試草稿防抖殘留已修正，保留閱讀位置斷言並完整重跑通過。交付雜湊與公司實測界線見docs/VALIDATION_0_8_52.md。

## 2026-10-12：等待 Python 實測的前置階段（已由0.8.52發行授權取代）

- 使用者仍在測試 Python，要求等待測試結果後再一起進版、編譯及推送；目前只保留原始碼、測試與文件修正，不繼續編譯、不打包、不發布。
- Outlook ConversationID 欄位相容性修正已保留；本輪格式、Clippy及307項單元測試通過。停止前 release 編譯已產生 target 內測試用 EXE，後續驗證依使用者指示中止，完整建置尚未完成。
- 保留0.8.51的 dist、更新清單、歷史離線ZIP、icon.png及共用環境。後續收到合併發行指示再依 v142 x64、空 Cargo 快取／--frozen與必要 Office／SMB／NSIS流程驗證。

## 0.8.51：Outlook 日期批次與程式工作資料（最新授權）

- 使用者已確認以完整主旨與寄送時間精確到秒判定隱藏副本，接受兩欄完全相同時排除；不讀寄收件者地址簿。優先日期篩選＋Table每批100列，備援排序批次；可見收件依收件日篩選，隱藏索引依候選寄送日，避免跨月遺漏。
- 專案Outlook輔助程序可被取消／10分鐘期限終止，完成排除才交付模型；錯誤不冒充零封。舊版郵件介面依候選寄送日延後建立索引。公司Outlook實際速度仍待實測。
- 通用Python流程保留真實副本／版本／區段原文與最小測試介面，section_id由程式管理；每段保存相同草稿。不得針對測試下載器特製修改，也不將模型筆記當成完成證據。
- 本版授權編譯進版、完整NSIS與Git main推送；依原有v142 x64、空Cargo快取／--frozen、Office／SMB與完整安裝驗證。保留icon.png、共用環境、Python runtime與歷史離線ZIP。

- 2026-10-09 17:42完整發行驗證通過：v142 14.29.30133／_MSC_FULL_VER=192930159 x64、空Cargo快取／--frozen、305項單元測試、Outlook批次／取消逾時、實際Python副本與Office／SMB、完整NSIS安裝更新回復解除安裝。產物與公司實測界線見docs/VALIDATION_0_8_51.md。

## 0.8.50：程式需求驗收與 Outlook 四欄位副本比對（最新授權）

- 使用者要求改善通用程式修改流程，不針對範例下載器特製修補。先保存需求，分段修改同一草稿，最後核對真正副本與需求；Python 可用標準 unittest 在既有 AppContainer 執行實際副本。語法、模型核對、功能測試、模擬依賴與未驗證項目分開記錄，證據綁定 revision。
- Outlook 移除 Message-ID 讀取，使用主旨、寄送時間、寄件地址、To/CC 地址集合的本機 SHA256 指紋。使用者接受四欄位恰巧相同時視為副本；不讀隱藏正文。部分欄位缺失時只隔離不能排除關聯的候選，明確無關的郵件仍可讀；完全不可辨識或索引未掃完須明示查詢失敗、數量未知。
- 使用者已授權編譯進版、完整 NSIS、原始碼及產物推送既有 Git main。使用 v142 x64、空 Cargo 快取、--frozen、Office／SMB 與完整安裝驗證；不改 icon.png、共享環境、Python runtime 指紋或歷史離線 ZIP。
- 其他語言先不加入，待本版 Python 的公司實測回饋；資料結構可重用，不預先聲稱模型品質或公司 Outlook 已通過。
- 2026-10-09 15:18 完整發行驗證通過：v142 14.29.30133／_MSC_FULL_VER=192930159 x64、空 Cargo 快取／--frozen、299 項單元測試、實際副本功能驗收、Python／Office／SMB 與完整 NSIS 安裝更新回復解除安裝。產物與公司實測界線見 docs/VALIDATION_0_8_50.md。

## 0.8.49：七項實測修正與有界並行（最新授權）

- 使用者要求隱藏 Outlook 資料夾中的副本也從其他資料夾排除、圖中文字雙擊編輯與對話框刪除、讀信後保存可寫入草稿、圖片本機轉 JPEG／100 張／雙圖合計 8 MB、VNC 單分類排序、修正活動紀錄與手動停止的 Error Code 99、Python 分段修改即保存同一草稿。
- 使用者追加獨立工具／兩個 Python 分析可並行，修改同一副本依序提交；不能以模型筆記假稱批次執行。草稿與語法通過的完成成果須清楚區分，保留原件及來源版本。
- 本版完成後直接進版、完整 NSIS 與既有 Git main 推送；仍使用 v142 x64、空 Cargo 快取／--frozen、Office／SMB 與完整安裝驗證。不改 icon.png、共用環境及歷史離線 ZIP。
- 2026-10-09 11:54完整發行驗證通過：v142 14.29.30133／_MSC_FULL_VER=192930159 x64、297項單元測試、26個原生／9個恢復／13個圖片案例、兩組68輪長任務、兩個真實Python程序重疊約8秒、持續草稿／隔離／逾時、Office／SMB與完整NSIS安裝更新回復解除安裝。產物與界線見docs/VALIDATION_0_8_49.md。

## 0.8.48：圖中文字、AI圖表編輯與本機時間（最新授權）

- 使用者要求連同下方待發行配色／排版完成編譯、進版、完整NSIS與既有Git main部署，取代只修改原始碼的限制。
- 自訂排版加入可拖曳文字，支援粗體／斜體／底線、字型、大小、字色與底色；畫面、加密歷史及PNG共用資料。AI使用相同受控圖表設定，不接受腳本或修改原始點陣。
- 使用者訊息及最終回覆顯示本機日期時間；輪次／工具活動顯示HH:MM:SS，時間於事件發生時保存，舊資料不捏造時間。
- 使用者追加Python原始碼編輯試用：開放.py文字副本，以內建Python固定檢查器AST／compile但不執行原文；目前版本語法通過後才發布副本。區分語法與功能測試，不需另增依賴，Python worker／runtime可維持原指紋。
- 完整發行仍以v142 x64、空Cargo快取／--frozen、Office／SMB與NSIS驗證；不改icon.png、Python runtime或歷史離線ZIP。
- 2026-10-09 08:35完整驗證通過：v142 14.29.30133／_MSC_FULL_VER=192930159 x64、288項單元測試、26個原生／9個恢復／12個圖片案例、兩組68輪長任務、Python編輯／隔離／逾時、Office／SMB及完整NSIS安裝更新回復解除安裝。正式產物與雜湊見docs/VALIDATION_0_8_48.md。

## 圖表文字排版與預設配色（併入0.8.48）

- 前置階段當時只修改原始碼與文件；現已依上方0.8.48授權一併編譯、驗證及發行。
- 參考線標籤自動避讓，提供明確的排版模式拖曳標題、圖例及參考線文字；不移動數據點或改變參考線數值。保留完成、取消與單獨重設排版。
- 一般設定加入八個系列預設色票；已自訂的圖表顏色優先。排版比例位置須隨歷史保存，縮放及PNG共用規則，舊設定缺欄位時相容。
- 執行不需編譯的JS、瀏覽器互動與格式檢查；新增Rust檢查待下次v142建置，不宣稱原生驗證完成。不改icon.png與Python runtime。

## 0.8.47：郵件成果筆記、階段與座標範圍（最新發行授權）

- 使用者已要求將本輪與上一批待發行修改一起實作、編譯進版及部署既有 Git main，取代下方不編譯／不發布安排。需完整 NSIS。
- 郵件閱讀後下一輪同時保存來源摘要／條目草稿，準備寫入時載入選定條目；階段控制技能並允許有理由的定向回讀。沿用 DPAPI／.lmai、來源版本與閱讀範圍，不把模型摘要當已驗證事實、不以改寫筆記假造進展。
- 量測軸以2倍標準差留白，限制在跨度5%～40%，依跨度選1/2/5刻度向外取整；保留全部可見有效點、手動範圍、常數備援及長條零基準。
- 使用 scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot "\\localhost\Y$\Rust\Project\CompanyAI\.build" -IncludeInstaller，確認v142 x64與實際cl版本、空Cargo快取／--frozen、完整驗證後才發布。
- 不改icon.png、Python worker／runtime指紋或歷史Rust離線ZIP。
- 2026-10-08 完整發行驗證通過：MSVC14.29.30133／_MSC_FULL_VER=192930159 x64、空Cargo快取／--frozen、284項單元測試、26個原生／9個恢復／12個圖片案例、兩組68輪長任務、Python／Office／SMB及正式NSIS安裝更新回復解除安裝。交付雜湊、設定／圖表檢查及公司實測界線見 docs/VALIDATION_0_8_47.md。


## 前置階段：參考線、設定、錯誤診斷與日期（已併入上方0.8.47發行）

- 此前置階段當時僅修改原始碼及文件，暫不編譯、不進版、不製作 NSIS 或發布；保留 0.8.46 產物與清單，後續已依上方授權一起驗證發行。
- AI 可設定圖表參考線；多時段未指定排版時使用非阻塞偏好，提供同圖／分圖及其他輸入，明確預設並保留原資料。
- 設定分一般／操作／進階，VNC 與診斷放進階。診斷須能選歷史回合、匯出錯誤，與含正文 JSON／純用量保持分離；錯誤提供固定代碼與文件。
- 查核今天錯到下週的問題，使用 Windows 本機日期作明確依據，區分目前日期、任務起始日期與使用者指定分析日期，不改寫已提交請求。
- 本輪僅執行不需編譯的格式、JS、來源頁面檢查；新增 Rust 測試留待下一次 v142 完整建置，不能宣稱原生驗收通過。
- 保留估算係數 0.48 與 8K–12K 軟目標，先觀察公司長任務；不修改 icon.png。

## 0.8.46：工作筆記、動態技能與非阻塞偏好（最新指示）

- 使用者已要求完成本輪所有討論功能、完整NSIS並部署既有Git main。
- 每輪累積筆記＋最近1–3組工具結果，原始要求／授權／來源與精確資料可查回；完整操作留DPAPI／.lmai。8K–12K是軟目標，不能以token預算直接拒絕任務。技能只保留目前階段的核心說明／工具，完整教學按頁讀取。
- Outlook分開新增資料與有限的舊代號恢復，空字串游標視為首頁，舊ID用outlook_index查回；不可用隨機快照或筆記假造進展。read_work_log索引按筆數固定分頁，原文按字元分頁。
- DEBUG位於進階設定，純用量報表不含正文、檔名或工具參數，與JSON有各自複製按鈕。初始估算係數0.48來自使用者四筆實際API樣本，需明示誤差與服務端usage缺漏，不能當真正tokenizer。
- 可選偏好不阻塞工作，完成前依事先公告的預設。Y空白／異常預設保留缺值，原值明細與索引保留供圖表編輯切換；無效X不默默排除整列。授權與關鍵資料定義仍需明確回答。
- 一般聊天與重試無20輪上限，桌面只送本次問題，網站負責歷史；不可因此截斷Outlook助理的專用請求。回答重點中的分列／可核對單行Markdown表格只修復顯示，不改原payload。
- 發行仍用 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller，固定v142 x64、空快取、--frozen；不改icon.png與歷史Rust離線ZIP。實際模型的速度、token與長任務品質需公司回饋，不以合成測試宣稱達標。
- 2026-10-08 完整發行驗證通過：MSVC14.29.30133／_MSC_FULL_VER=192930159 x64、275項單元測試、26個原生／9個恢復／12個圖片案例、兩組68輪長任務、Python／Office／SMB與正式NSIS安裝更新解除安裝。最終產物、雜湊、DEBUG用法及實測界線見 docs/VALIDATION_0_8_46.md。

## 0.8.45：本機預設專案、上游恢復與實際進度（最新指示）

- 使用者已授權把本批與下方待發行分析功能一起進版、完整 NSIS 編譯驗證並部署既有 Git main，取代待發行段落的不發布安排。
- 預設改為 Windows 目前使用者個人資料夾的 LM_AI_Projects/LM_AI專案資料夾_YYYYMMDD，同日加序號；移除桌面／下載選擇，保留自選資料夾及 UNC／映射磁碟。確認預設為本機固定磁碟，不拼帳號路徑。
- 專案主推論及子請求新增10／30／60／180／300秒重試，耗盡保存並暫停，可手動繼續。結果未知只查原ID；僅已核對的 failed、result=null、限定暫時上游錯誤可另建一次推論。使用者回報的 AI_BACKEND_ERROR 納入有限重試，即使舊網站標 retryable=false；登入／權限／額度／格式／身分／結果未知錯誤不因此放行。這是新增的明確桌面政策，須同步更新契約；不重播本機工具。
- 無進展依實際新資料／閱讀範圍，Outlook 保存查詢與下一頁；四次提醒，八次暫停。筆記、操作ID或游標隨機碼不能假造進度；網路重試與工具5／10次分開。
- 使用者追加移除專案「圖片辨識」快捷按鈕、選圖視窗及獨立 image-read 技能；analyze_image 併入基本檔案閱讀，AI 按需直接使用。保留模型能力／大小／來源／快取／未知提交保護；Outlook 助理及一般聊天附件不擴大。舊 checkpoint 的 image-read 僅作空相容別名，不再公告。
- 發行使用 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller，固定v142 x64、空Cargo快取、--frozen；不改icon.png，不重製歷史Rust離線ZIP。
- 2026-10-07 本版完整發行驗證通過：MSVC14.29.30133／_MSC_FULL_VER=192930159 x64、263項單元測試、25個原生／9個恢復／12個圖片案例、Python／Office／SMB及正式NSIS安裝更新解除安裝。最終產物及雜湊見 docs/VALIDATION_0_8_45.md；公司F槽及真實模型效果仍待公司實測。

## 已納入0.8.45：分析依據、方法記憶與映射磁碟

- 使用者同意五項分析改善：原生範圍紀錄、可展開工具證據、工具結果筆數核對、project 範圍方法記憶、目標／待確認／失效結論。沿用既有工具、DPAPI 記憶與 checkpoint；不把 AI 宣稱當程式驗證。
- 使用者回報 F 槽子資料夾出現「無法核對文件的實際位置」，UNC 可用；映射磁碟以目前工作階段 Windows API 解析 UNC，正規化 handle 路徑提供 NT 名稱備援，不放寬 .lmai／連結／路徑界線。相同錯誤每次重試均重新顯示。
- 前置階段先修改與完整驗證，當時保留 0.8.44 dist／更新清單；後續依上方授權合併0.8.45發行。前置驗證使用 Build.ps1 -ValidateOnly -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build'，仍需 v142 x64、空快取與 --frozen。本機映射測試不得冒稱公司 F 槽已驗收。
- 2026-10-07 前置完整驗證通過：MSVC 14.29.30133／_MSC_FULL_VER=192930159 x64、259 項單元測試、原生／舊版對話與續接、Python／Office／UNC／映射磁碟。新增案例實跑 Big5 分段、超過77 MiB LOG、證據與數量核對、方法去重及跨對話查回；WebView2 重複通知與分析介面也通過。紀錄在 docs/DEVELOPMENT.md 與 .build/analysis-validation-final.log；當時尚未進版，正式0.8.45另依上方流程重新驗證及打包。

## 0.8.44：圖表編輯與失敗預算（最新使用者指示）

- 使用者授權完成七項修改後進版、完整 NSIS 並部署既有 Git main。所有對話框按鈕加底色；圖表編輯分基本設定／座標轉換／顏色與參考線三頁，加入就地說明。
- X／Y 各有自動範圍；使用者已確認依數值尺度向外取整，例如12～23為10～25、0.12～0.23為0.10～0.25。類別軸取完整位置；空白參考線文字不補數字；滑鼠提示同時顯示實體X／Y。
- Python 與其他工具的失敗獨立計數，上限10／5；同類成功才歸零，已辨識原生工具的非嚴格參數錯誤也納入。失敗不重複扣8次無進展額度；未知提交、身分、權限及無法辨識的回覆保護不放寬。
- 使用完整 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller，遵守 v142 x64、空快取、--frozen。Python worker／清單不變，沿用既有安裝最佳化；不改 icon.png，不重製歷史 Rust 離線 ZIP。

## 0.8.43：座標轉換、圖片支援與 Python 沿用（最新使用者指示）

- 使用者已要求本批完成後進版並部署既有 Git main，取代下方待發行段落的暫不發布限制。納入三張異常值示意、X／Y平移及重新編號、全系列無值位置移除、Python安裝沿用及專案圖片一般支援。
- AI 增加 transform_chart，沿用原 CSV／Excel 欄位核對後衍生座標；圖表編輯器也可直接調整。Python 技能增加主動選用時機，固定閱讀／作圖仍走專用工具，不強迫每次使用 Python。
- Python LOG／OUT／ERR 無 BOM 先嚴格 Big5（CP950）、失敗再 UTF-8；BOM 優先，回報編碼／歧義並提供 encoding 明確覆寫。修正原生 UTF-8 前置阻擋；worker／套件指紋維持不變。工具參數拒絕加入欄位路徑診斷，不猜參數執行。使用者後續討論待辦系統，本版保留現有累積筆記，不另加待辦流程。
- 正式驗證用 scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller；v142 x64、空快取及--frozen不變。發布 EXE、完整 NSIS、兩份NSIS簽署清單、驗證文件及原始碼，不重製歷史離線ZIP、不改icon.png。

## 待下次發行：Python 更新沿用與圖片附件（最新使用者指示）

- 使用者目前仍在公司測試 LOG／Excel 繪圖；本輪先修改及驗證，不進版、不發布或推送，保留 0.8.42 的 dist、簽署清單與發行驗證紀錄。
- NSIS 使用新版待安裝 EXE 的 Python 完整自檢，通過就原地沿用現有環境；不符時完整替換及失敗回復，不只比對版本字串、不逐檔混補套件。Python worker 暫仍納入同一環境指紋。
- 使用者已澄清圖片支援指「專案內的圖片檔」；移除試驗定位，與其他專案資料一起使用，AI 按需求決定是否讀取。Outlook 助理及一般聊天不改動，不新增郵件附件讀取。
- 圖片單次仍一張／5,000,000 bytes；每任務改為最多20次不同辨識要求，相同來源與焦點完成結果重用。只送當次子請求，後續保留文字重點，fast 不支援；保留既有待查請求與快取識別碼供續接。
- 異常 Y 值選擇視窗加入三張固定五點折線示意，並列保留缺值／略過此點／設為0；標明假設資料、共用座標及原 X 位置，不改變既有選擇或原始數據。只有無效 X 的視窗不顯示這組 Y 值示意。
- 使用者後續要求 X／Y 均支援平移、重新編號及移除無值位置；AI 與完成圖表編輯器共用受控轉換。保留 CSV／Excel 來源欄位核對，另以 transform_chart 衍生座標，不手抄原數列。先移除全系列無值列、再用共同列序號編號，0 保留；實體 X／Y 隨水平長條正確對應。原始點陣、明細、來源保留；PNG 依圖表轉換版本去重，舊 PNG 不覆寫。工具目錄增為49項。
- 測試建置只留 target／.build；Rust／C++ 沿用 v142 x64，完整原始碼驗證使用 Build.ps1 -ValidateOnly -EmptyCargoCache，安裝回歸可用 Test-Installer.ps1 -ValidateOnly。待 LOG／Excel 測試問題收齊後再一起進版。

## 0.8.42：獨立 Python 與完整 NSIS（最新使用者指示）

- 使用者明確授權加入離線 Python 分析、製作完整 NSIS 並部署既有 Git main；取代先前不製作 NSIS 的限制。不改 icon.png、不開放 PowerShell、不移除 Outlook 助理。
- 本版固定 CPython 3.13.12 x64、pandas 2.2.3、NumPy 2.2.6、openpyxl 3.1.5 及鎖定的必要依賴。安裝在 EXE 旁 python 子目錄，不嵌入 EXE、不使用系統 Python／pip／網路下載。
- Python 在單次 AppContainer／Job 執行，原檔由 Rust 唯讀建立快照；公司原始 Excel 仍以既有 COM 匯出資料集交給 pandas，不引入 pywin32。生成 XLSX 可用 openpyxl。
- 原生程式與 C++ 測試必須 v142 x64；上游預編譯 Python／wheel 的來源與編譯器需另據實記錄，不冒稱由 v142 編譯。
- 完整驗證用 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot -IncludeInstaller。交付 EXE、NSIS、兩個指向 NSIS 的簽署更新清單、Python 離線依賴包及驗證文件。歷史 CompanyAI-offline.zip 不重製、不冒稱新版。
- 修改 python_worker.py 後以 Prepare-Python.ps1 -Destination <新目錄> -RefreshManifest 重建，核對 assets/python-runtime-manifest.json 與 python-runtime-sha256.txt，再編譯。Python 原始碼必須 LF，避免不同 Git checkout 改變固定雜湊。

## 0.8.41：圖片關聯、單張試驗與紀錄捲動（最新指示）

- 修正圖片及原生快速摘要子請求：直接使用已核對父請求中的遠端 conversation_id，同 owner／project／run／conversation；client_snapshot 不允許網站自行補回歷史。
- 使用者把100張說明為舉例，本版不做批次功能／技能。每次任務只允許1次不同圖片辨識要求；同來源與焦點的完成結果可重用。最大5 MB明確採5,000,000 bytes，拒絕超額，不自動壓縮；圖片JSON最多10,000,000 bytes並遵守網站公告。
- 圖片只送當次無工具子請求，後續主對話只保留依需求整理的文字重點與來源。待查子請求仍以本機DPAPI保存以防未知提交重播；完成或明確未受理後清除圖片快照。不擅改網站資料庫。
- 快速模型 fast 不公告image-read／analyze_image，不載入圖片技能說明；原生入口、工具及請求建立均阻擋並提示此模型不支援圖片傳入。
- 工具紀錄保留清單節點、底部追加；120筆輪替保留可見列。中途閱讀不強制回頂端，在底部才跟隨；保留進度筆記及已完成紀錄的內部捲動。
- 沿用v142、EXE／main交付授權；完整Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot後推送。不改icon.png，不製作NSIS／ZIP。舊版已保存但未受理的422任務需重新送出新任務，不改寫未知提交ID。

## 0.8.40：Excel 欄位規劃與使用者圖表編輯（最新指示）

- 使用者授權實作 Excel 欄位規劃及完成圖表的本機編輯、恢復原樣與儲存圖片；沿用 EXE／既有 main 交付範圍，不改圖示、不製作 NSIS／ZIP、不開放 PS。
- AI 依要求、實際表頭及少量樣本規劃 time/X/Y；原生重查來源版本、表頭與時間格式，規劃隨 CSV／DPAPI 續接保存。新增 plan_excel_analysis、export_planned_excel，目錄共47項；不重新編號不連續欄。
- 本地時間掃描最多250000列／120秒，符合最多10000列／9欄；HH:MM是時:分，區間起點含、終點不含。日期序號不取餘數；錯誤／空時間不默默略過；不靜默抽樣。
- 圖表使用者設定獨立保存在加密歷史、不傳模型、不改原值；完成訊息可雙擊／編輯。支援標題、實體X/Y軸範圍、系列名稱／色彩、圖例位置及最多10條參考線。PNG只在原生確認的專案_AI_Output新時間資料夾保存並讀回驗證；來源版本碼不顯示，內部保留。
- 圖型為line/bar/scatter/step/area/horizontal_bar，標題置頂置中、圖例預設右側；不提供自由拖曳排版及需額外統計的箱形／直方圖。
- 發行需 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot；驗證三份Excel（含欄序調換）× 五時段共15圖。模型語意判斷與公司真實資料仍待使用者實測。
- 使用者已明確把圖片5 MiB上限與自動壓縮延後至下一版，待公司圖片API實測；本版維持原圖1 MiB與既有圖片請求格式。

## 0.8.39：Outlook 專案入口與圖片試驗（最新使用者指示）

- 使用者授權本版 Outlook 整合＋圖片嘗試、進版編譯 EXE 並部署既有 Git main；沿用 v142、空 Cargo 快取及 Build.ps1 全流程，不改 icon.png／NSIS／ZIP，不開放受控 PS。
- 專案 Outlook 按鈕共用原有 runner、技能、勾選權限及 1000／50 封額度；獨立預覽／MSG 入口先保留，不把入口整合宣稱為所有 UI 已合併。
- 專案 JPG／JPEG／PNG 圖片試驗：analyze_image(path,focus)，image-read 按需載入，工具目錄共 45 項。單張 1 MiB、8192×8192／1600 萬像素、每次任務最多 20 次不同要求。只允許專案相對路徑及既有檔案保護。
- 使用者已實測模型伺服器的 image_url；公司 API 負責轉發及身分驗證。本版明確允許圖片試驗，即使舊 capabilities 只公告 text；仍遵守授權、模型與整份請求大小上限。若轉發端拒絕，不另接模型直連端點、不略過 TLS 驗證。此條僅涵蓋無工具圖片子請求，取代舊文字專用限制。
- 真正 user.content 陣列包含 text／image_url data URL。使用目前模型、既有 agent/turns 與 parent_request_id，tools=[]；主歷史只保留文字及 SHA256。DPAPI 保存待查子請求，未知提交只查原 ID；完成移除圖片快照，相同來源與要求快取。
- 不讀 Outlook 圖片附件／內嵌圖，不解碼掃描 PDF。這些留待後續整合。公司代理／實際模型辨識品質待使用者驗收。詳見 docs/VALIDATION_0_8_39.md。

## 0.8.38：週報精靈、網路專案及回覆恢復（最新使用者指示）

- 使用者已確認實作、編譯 EXE 並推送既有 Git main；沿用 EXE 發行，不重建 NSIS／ZIP，不改 icon.png，受控 PowerShell 延後。
- 專案提供資料傳送說明及目前 Windows 下載／桌面的預設資料夾建立功能；使用 Known Folder 重新導向位置。明確開放映射網路磁碟與 UNC 專案，取代舊版只允許本機路徑的限制；仍檢查路徑、重解析點、硬連結及專案邊界。
- 專案快速操作只留生成週報：說明、建立素材目錄／日期／選填補充、送出前確認。按否回輸入；確認後才啟動任務；取消不刪除素材。日期預設本週一至今天，提示帶本機日期及 ISO 週年／週別，使用者明確日期優先。
- Exchange／OST 線上收件匣及子資料夾加入既有 Outlook 工具，仍需勾選授权；不新增 Graph／新 Outlook 連線能力，不宣稱快取等於即時伺服器資料。
- 無效模型回覆改為兩次一般修復、封存原始工具結果後兩次精簡上下文恢復；每段累計最多 30 次。保留使用者原文、權限、額度、來源及副本狀態；未知提交不可重送，無進展及工具錯誤保護不變。
- 完整驗證使用 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot，固定 v142 x64、空 Cargo 快取與 --frozen；本機 SMB／映射測試不得冒稱公司 F 槽及真實 Outlook 驗收。見 docs/VALIDATION_0_8_38.md。

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

## 0.8.37：Outlook 隱私選擇與本機比對（最新使用者指示）

- 使用者要求實作、進版、編譯 EXE 並部署 Git main；供公司實測。包含未提交的0.8.36變更，不重建NSIS／ZIP、不改圖示。受控PowerShell明確延後。
- 先在本機列資料夾供勾選，首次預設全選、之後保留选择；未勾選分支不可列給AI、讀取或匯出。DPAPI設定共用於專案及Outlook助理，實際資料夾祖先鏈再次核對。變更範圍拒絕舊快照續接，已送出的資料不可宣稱已撤回。
- 本機前文比對最多1000封不同版本、64MiB／120秒；AI內文最多50封，分開計數。工具只回比較摘要，不回未選讀內文；全文正規化精確涵蓋才略過，不以字數／相似度判定完整。
- Outlook任務優先同串最新信，再按缺口補讀；週報一開始載入方法／讀舊結構。保留來源與短結論，足夠即交付。限制與測試見docs/VALIDATION_0_8_37.md。
- 發布必須使用Build.ps1 -EmptyCargoCache -TestOffice驗證v142 x64、空Cargo快取／--frozen、原生UI與Office回歸。公司Outlook／模型品質待使用者實測。

## 0.8.36：本地 CSV 與主動上下文整理（最新使用者指示）

- 使用者要求兩項實作後編譯 EXE 並部署 Git；CSV 建立後不得反覆把完整原始資料帶入對話。沿用 main／EXE 發行授權，不製作 NSIS／ZIP，不改 icon.png。
- Excel／LOG 先確認樣本與欄位，再由原生工具匯出來源追蹤 CSV；模型只見路徑／版本／欄位／筆數、首尾各 10 筆及統計。chart_dataset 直接讀 CSV，回覆只有圖號／處理摘要；型別、顯示文字、原列號與异常值明細保留。
- LOG 每批最多 30 檔、250000 筆／16 欄／64 MiB；固定分隔或起訖標記，不執行腳本。Excel 沿用本地圖表讀取限額 10000 列／9 欄／90000 格。圖表仍最多 10000 筆，不靜默抽樣。
- 匯出成功先封存完整工具結果，再精簡舊原文；compact_context 保存交接筆記／失效結論／下一步。補充於安全界線先縮短舊結果，標記舊笔記待核對，不改寫已提交或未知的模型請求。保留使用者原文、去重、來源／CSV 版本、24 小時與無進展限制。
- scripts/Build.ps1 -EmptyCargoCache -TestOffice 驗證 v142、空 Cargo 快取／--frozen、原生 HTTP CSV／更正／查回、30 份 LOG 及真實 Excel CSV 往返後才發布。公司加密 CSV 行為、實際模型選欄與速度待使用者驗收。

## 0.8.35 發行（最新使用者指示）

- 使用者已要求編譯後推送 Git，取代下方前兩輪只修改原始碼的安排。合併進度筆記顯示、LOG 近似時間定位及主輸入框三種傳送模式，進版 0.8.35。
- 使用 scripts/Build.ps1 -EmptyCargoCache -TestOffice 完成 MSVC v142 x64、空 Cargo 快取／--frozen、原生與 Office 驗證後，交付 EXE、簽署清單、原始碼與文件至既有 GitHub you1230783-sys/CompanyAI 的 main。不製作 NSIS 或離線 ZIP，不改其他專案與 icon.png。
- 排程與停止須驗證原任務先結束、原對話／帳號／模型綁定、重啟不自動送出及加密歷史；不得以 DOM 檢查代替原生執行。公司模型與實際 LOG 品質仍待使用者驗收。

## 0.8.35 納入：LOG 定位與主輸入框傳送方式（原待發行項目）

- 使用者的約略時間先前後各 5 分鐘定位事件錨點，再回讀確認開始；無結果有界擴大至前後 15／30 分鐘，不反覆找整點字串。只有時間的 LOG 可查，日期來源明示；支援時間後逗號分隔訊息。
- 專案執行中使用主對話框選擇下一輪提示（預設）、停止後啟動新任務、完成後啟動新任務；移除補充專用小視窗。每個對話先支援一則可取消的待送新任務，與歷史一起 DPAPI 保存，綁定原對話／專案／帳號／模型。
- 舊 worker 結束後才啟動新任務；暫停、失敗、等待補充、手動停止或重啟保留待送文字供手動傳送。草稿只在確認接受且仍為原文字時清除。
- 前輪只改原始碼的安排已由上方 0.8.35 發行指示取代；原始設計見 docs/PENDING_LOG_AND_COMPOSER.md，發行驗證見 docs/VALIDATION_0_8_35.md。

## 0.8.35 納入：顯示進度筆記（原待發行項目）

- 執行中顯示模型提供的最新 progress_note；屬於工作進度摘要，不是內部思考過程。沿用工具活動紀錄保存，完成／暫停／失敗後連同中途說明收進預設收合區，不混入最終正文或複製內容。
- 前輪延後編譯的安排已由上方 0.8.35 發行指示取代；設計說明保留於 docs/PENDING_PROGRESS_NOTE_UI.md。

## 0.8.34：大型 LOG、Outlook 與長任務

- 本輪授權包含大型 LOG、執行中補充、專案 Outlook 唯讀、占用檔案等待重試、圖表預設略過異常文字及異常值明細、跨對話上下文整理、最長 24 小時任務與 checkpoint。程式原始檔、CMD／PowerShell／虛擬環境另輪處理。
- LOG 檔名 YYYYMMDD_分類_站別.log（A01-01、Z01-CY），內文 `2026/06/23, 15:25:48.084`。內文日期優先；單檔 32 MiB／每次 30 檔，逐行分頁／時間關鍵字搜尋／行號／來源版本。未讀完及未知時間明確標示。
- 補充指示逐次核對 run／conversation，DPAPI 保存原文；下一輪作 user 訊息，保留原始目標。未開始的舊候選工具回傳 executed=false 再規劃，已開始的先完成。
- Outlook 工具第一次執行前必須原生 UI 同意，暫停或重啟續接重新詢問；本地 PST 收信及規則資料夾、Exchange／OST 線上信箱寄件備份分層選擇，先標題後重要內文，不寄信或讀附件。不能把離線快取宣稱為即時伺服器內容。
- 正式原生任務每段最多 24 小時，60 工具／80 回覆自動 checkpoint 換批；無進展及錯誤上限保留，不自動重設。手動繼續開始新一段時間。原請求未知只 GET 查回；本機修改開始先使舊 checkpoint 失效，完成核對後才恢復，不重播未知寫入。
- 原始要求／補充、工作筆記、真實狀態與舊工具索引分開；65% 工具文字預算或 30 對工具歷史自動整理，完整結果 DPAPI 分檔按需回讀。近期三輪用戶要求與 AI 最終答案、工具計數供下輪使用，過大最終答案有查回指標。摘要不取代授權及證據。
- 使用者已授權完成後 EXE 上 Git main；以 Build.ps1 -EmptyCargoCache -TestOffice 完成 v142／空快取與 Office 驗證；不製作 NSIS／ZIP。不得宣稱模擬時鐘等於已連跑 24 小時；實際公司郵箱／加密／模型品質待使用者實測。詳見 docs/VALIDATION_0_8_34.md。

## 0.8.33：圖表資料決策與按需工具

- 使用者授權完成後進版、編譯 EXE 並上傳 main；以 Build.ps1 -EmptyCargoCache -TestOffice 驗證，不製作 NSIS／ZIP。
- 明確數字文字只在數值軸轉換；類別 X 保留前導零。異常值按欄／類型由桌面 UI 選擇 gap／skip／zero，原空白不變；無效 X 僅可明確排除整列。模型不提供使用者決策參數。
- 等待／稍後決定不算工具失敗；同一已完成模型請求暫存續接，重新預檢並核對來源版本。PNG 與聊天室共用座標及缺值規則。
- 原生專案初始七個基本工具；九項技能簡介，load_skill 啟用對應工具與唯一的 system 說明，依賴組合及續接持久化。模型／副本／圖表條件仍適用，不更改網站路由或資料庫。

## 0.8.32：Office PNG、進階診斷與工具可用條件（最新使用者指示）

- 使用者已改為要求完成後編譯並上傳：進版 0.8.32，以 Build.ps1 -EmptyCargoCache -TestOffice 驗證後發布 EXE、簽署清單、原始碼與文件至既有 main；不製作 NSIS／ZIP。
- office_action／office_batch 新增 insert_image：專案相對 PNG，經路徑鎖／CRC／尺寸／SHA256 核對，Word 段落前／文末、Excel 儲存格錨點、PPT 座標，保持比例、嵌入而不外連，只改副本。圖片來源在發布前須保持不變。
- 執行紀錄入口移至設定內預設收合的「進階功能」，可選目前對話的不同任務；聊天／進度區不再顯示診斷按鈕。
- fast 不公告 summarize_document；沒有副本／圖表時不公告相關編輯／儲存／匯出工具。空 copy_id／revision 進有界修復，不執行空白修改。
- 使用者已要求網頁端放行已知欄位正確的回覆；桌面以白名單讀取結果／訊息／工具封裝，忽略額外欄位。必要欄位、身分配對、finish_reason 與工具 arguments Schema 仍驗證；詳見 docs/VALIDATION_0_8_32.md。

## 0.8.31：圖表、PNG 及本機診斷

- 使用者已改為要求完成後進版、編譯並上傳 Git，取代本批先前「不編譯、不進版」指示。發布 0.8.31 EXE、簽署清單、原始碼與文件至 main，不製作 NSIS／ZIP。
- 單圖最多 10000 筆、8 系列；Excel 直接畫圖獨立最多 90000 格，一般 read_excel_range 仍限 2000 格，不靜默抽樣。
- export_chart_png 由內嵌 ECharts 繪製完整範圍，受控寫入 _AI_Output，重名加序號、讀回核對、任務去重及交付前再驗證。
- 工具失敗顯示原因，另提供本機加密日誌的查看／複製；不將本機顯示視為已修復網站資料保存。
- 執行 Build.ps1 -EmptyCargoCache -TestOffice；涵蓋真實 Excel 一萬筆取值、PNG WebView2 callback 與目視檢查。實際公司加密及網站聯測仍需使用者確認。紀錄見 docs/VALIDATION_0_8_31.md。

## 0.8.30：原生工具 Schema 相容性修正

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
- 本輪交付 0.8.19 EXE、清單與原始碼，執行 `Build.ps1 -EmptyCargoCache -TestOffice`，沿用 main 推送授權；不打 NSIS／歷史 ZIP。MSG／公司加密未實測通過不得宣稱驗收。

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
- 使用者已要求完成後以 `Build.ps1 -EmptyCargoCache` 編譯並推送 EXE、EXE 更新清單、原始碼與文件；先不製作 NSIS 或重製歷史 ZIP，待公司讀寫實測後再打包。本機驗證不能代替公司加密／模型環境測試。

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
