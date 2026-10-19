## 待發行：新圖使用確切CAD版本（2026-10-19）

`cad_draw`的version必須是使用者指定或`cad_inventory`回傳的確切ProgID，不接受`latest`。未指定版本時先查清單；要求最新版時也先取得最新版的實際ProgID再提交。Broker與原生worker均在建圖前拒絕別名，不自動替模型改寫參數。其他連線／唯讀工具的既有版本規則不變。

本機測試曾要求2008卻因模型提交latest而在2022建圖，產物也無法由2008開啟。驗收器現在比對原生報告的實際ProgID；錯版標記WRONG_CAD_VERSION，僅在實際建立它的CAD做獨立唯讀覆核，不當作指定版本通過。修正與新候選實測進度見專案AGENTS及情境驗證紀錄，尚未發行。

## 0.8.64：原生線型與目前圖面純縮放

- 新建線／圓／公式曲線可指定 line_style；既有線／圓／弧可修改原生 Linetype／LinetypeScale。沿用已有定義，缺少時查 CAD 安裝目錄標準 LIN 檔，不修改全圖 LTSCALE。線型外觀與舊版介面仍待公司實測。
- 目前圖面純 ZOOM E／A 不依賴 StartUndoMark；仍保留確認、文件／revision 與指令前後可讀內容核對，不儲存不關閉。圖元修改與混合操作仍要求完整 Undo 群組，不以縮放例外繞過。
- 公司實測 0.8.63 的程序身分備援及目前圖面讀取成功；其 ZOOM 被 Undo 前置檢查阻擋，不能視為縮放已驗收。
- 使用者已授權本版進版、v142 x64／空快取 --frozen 基礎驗證、NSIS 及推送 main；完整驗證與公司 2008／2022 實測待補。保留 icon.png 及歷史離線 ZIP。

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

## 0.8.60：iCAD 2008 尺寸樣式介面

舊版使用 `Document.DimensionStyles.Add` 與 `DimTxt`／`DimAsz`／`DimGap` 等原生屬性；不再呼叫缺少的 `DimStyles` 或 `CopyFrom`。2008 的新樣式設定會立即讀回，並在儲存重開後核對。GstarCAD 使用原相容介面。一般文字僅準備文字樣式，不依賴尺寸樣式集合。

整批所需樣式在幾何繪製／既有圖元刪改之前準備；目前文件的樣式準備保留在 Undo 群組內。前置失敗仍可能留下新建空圖或未使用樣式，不自動關閉、重播或假稱沒有寫入。

介面參考：[IntelliCAD Document 成員](https://www.progesoft.com/products/progecad-professional/manual?mp=developer-reference/api-reference/objects/document-object/members)、[DimensionStyle 原生屬性](https://www.progesoft.com/products/progecad-professional/manual?mp=developer-reference/api-reference/objects/dimensionstyle-object/members)。參考文件較新，不能取代公司2008實測。[本版驗證](VALIDATION_0_8_60.md)。

## 0.8.59：標註與舊版讀取修正

- 尺寸／引線字高3、箭頭3、ISO文字對正、距尺寸線1，使用專用 simplex.shx 樣式。一般文字保留明確 height 參數，預設3。中文沿用現有 BigFont，或使用 CAD 安裝目錄已存在的 chineset.shx；若沒有中文字型，報告會提示，需在 CAD 設定後確認外觀。
- 2008／2022均保留原生引線、獨立文字與指定樣式。若COM明示有本次新建的空白／同內容文字附屬物件，核對關聯後納入預期集合；未知額外物件仍拒絕。不據此認定原報告的額外圖元一定由引線產生。
- 報告增加每項操作實際產生的 Handle／類型／可讀屬性及額外圖元描述；仍不忽略未知物件、不自動刪除。
- 不再強制要求 Document.HWND 才能閱讀；應用視窗可作備援，搭配程序建立時間、唯一文件名稱／路徑與快照。均不可用時仍可唯讀，editable_session=false 時請改用副本修改。應用視窗備援不具文件視窗同等身分粒度；桌面確認及操作期間勿關閉／切換圖面仍必要。
- 已讀取的尺寸／原生引線只允許 delete，重標時先核對 Handle 再刪除重建。不明額外物件不在自動刪除範圍。
- unsupported_capability 禁止相同版本反覆重試；讀取失敗在介面明示失敗。

ISO 對正參照 [Autodesk 尺寸文字設定](https://help.autodesk.com/cloudhelp/2026/ENU/AutoCAD-Core/files/GUID-38DAEEF0-AB44-4C58-8D0E-955BFCFF71A7.htm)；中文 SHX 搭配參照 [Big Font 說明](https://help.autodesk.com/cloudhelp/2022/ENU/AutoCAD-MAC-Core/files/GUID-CBC3F683-06F3-4B9C-8747-D01F3C262956.htm)。文件僅供介面語意參考，不能取代2008／2022實測。驗證見 [0.8.59驗證紀錄](VALIDATION_0_8_59.md)。

# 0.8.58：目前圖面與 Excel 座標

三種入口共用固定幾何操作：新建圖面、既有 DWG 副本、目前開啟的圖面。副本與目前圖面共用修改／核對引擎。未新增 CAD 指令、LISP、圖塊展開或清理。

## 目前圖面，不自動儲存

範例：「讀取我目前開著的 CAD，找出文字『孔位一』，改成『%%c10』，不要儲存。」

- 先讀取 ModelSpace（含未儲存內容），依實際 Handle、位置與文字選取。寫入前桌面顯示文件名稱與路徑，使用者確認後操作。
- 程序 PID／建立時間、文件 HWND／名稱／路徑與快照共同核對；切換、關閉或手動更動可讀屬性後需重新讀取。舊任務不恢復目前文件寫入授權。
- 每輪修改包成 StartUndoMark/EndUndoMark；不支援時在修改圖元前停止。成功回報 EDITED_UNSAVED，不 Save／SaveAs／Close／重開；使用者自行檢查、Undo 或存檔。
- 每輪要重讀；最多六次修改／任務。操作期間請暫停手動操作 CAD，避免使用者的動作混入 Undo 群組。COM 成功不等於外觀與實機 Undo 已驗收。
- 目前圖面操作也留下獨立 cad-report.json；程序中斷可留下 PENDING／UNKNOWN，不據此宣稱修改完成。修改已開始後遇到錯誤停止，不用副本的重建復原去重播目前圖面。

## 更多基本編輯

共用 edits 支援 text、color、move、radius（圓／弧）、height（文字）、delete（可編輯簡單圖元，value=null）。同 Handle 每次一項修改，鎖定圖層拒絕。新增支援原有線、圓、文字、多行文字、尺寸、引線、剖面與公式取樣曲線。圖塊、代理物件與圖紙空間仍不修改。

## Excel 座標

範例：「讀取座標.xlsx，A欄X、B欄Y、C欄半徑、D欄標籤，資料從第2列到第20列；單位毫米、原點不偏移。先預覽，再畫到目前圖面，不儲存。」

- inspect_excel 核對表頭與來源版本；cad_excel_plan 在本機生成操作與前六項預覽，cad_excel_apply 才執行。模型不抄寫整批數值。
- 支援每列畫圓（固定半徑或半徑欄）、按列順序連線／閉合、可選標籤、ACI顏色、比例與原點。座標=數值×scale+origin，半徑×scale，文字高度以目標圖面單位指定。
- 支援單位英吋/mm/cm/m，與目標 INSUNITS 必須一致；新圖固定mm。未知單位需先在CAD設定，再重新讀取。
- 每次100列、展開後100個操作上限。空白、非數值、負半徑、零長線等整批拒絕，不暗中跳過。初版無分組輪廓、3D、標籤避讓或任意公式；公式儲存格使用既有 Excel 讀取值，不強制刷新外部連結。
- 目標 new/copy 會另存成果，active 不儲存。apply 前重查 Excel revision，座標計畫一次使用。

實機 2008／2022 文件 HWND、Undo 群組、未儲存持續修改與 Excel→CAD 全流程仍需公司環境確認；本機基礎驗證詳見 [0.8.58驗證紀錄](VALIDATION_0_8_58.md)。

---

# 新建圖面使用方式（原 0.8.55 功能）

## 使用方式

安裝本版 NSIS，在專案對話中要求 CAD 繪圖。AI 可按需載入「CAD圖面／副本編輯與Excel座標（測試版）」技能。電腦需要安裝可用的 CAD；Python／pywin32 隨安裝包提供。

範例：

> 使用浩辰 CAD 2022，新建 200×100 mm 的矩形，四角各距邊 15 mm，畫四個直徑 10 mm 的孔位圓。外框白色、孔圈紅色，加入長寬及直徑標註，用綠色帶箭頭引線寫「孔徑 10 mm」，文字放在水平線上方。另在旁邊以藍色畫 y=10*sin(2*pi*x/100)、x=0 到 200 的曲線，分成 100 段。存成 CAD測試.dwg。

預設選明確年份最新的版本，亦可指定 2022／2008，由 inventory 的確切 ProgID 選取。選定版本失敗不改用其他版本。連接或啟動所選 CAD，只新建圖面；成功後存檔、重開核對並關閉自己的文件。成果會列於對話並存入專案 `_AI_Output/<新資料夾>/`。

後續可重建新 DWG，或使用副本／目前圖面編輯；後兩者見上方 0.8.58 說明。組立、3D 與 Solid Edge 尚未支援。

## 操作與界線

| 項目 | 內容 |
| --- | --- |
| 幾何 | XY 平面、mm、直線及圓 |
| 標註 | 對齊尺寸、直徑尺寸 |
| 文字 | 單行／多行中文、指定文字高度 |
| 引線 | 原生帶箭頭直線（Type=2）；水平段上方獨立文字，未關聯 |
| 剖面 | ANSI31、多邊形外框與圓孔，指定圖樣比例 |
| 公式 | x(t)/y(t)，算術及 sin/cos/tan/sqrt/exp/log/abs、pi/e |
| 顏色 | 各操作 ACI 1–255；1紅2黃3綠4青5藍6洋紅7黑白8深灰9淺灰，不接受 RGB |

公式以取樣直線近似，2–1000 段；未計算連續曲線誤差上限，也不產生原生 spline。每份計畫 1–100 個操作、最多 4000 個圖元，每任務十份 DWG。各欄位、數值、公式與點集合先完整驗證，再建立圖面。剖面外框不自交、圓孔位於外框內與 seed 位於填充區仍須依幾何意圖／外觀核對。

存檔前後核對 Handle、圖元數、指定幾何、文字、引線 Type、顏色與毫米單位。顏色為圖元 Color；列印樣式、螢幕背景與 CAD 顯示設定可影響呈現。2008 的 Measurement 回 E_NOTIMPL 時明示未自動核對，不偽造標註驗收。成功狀態為 `DATA_VERIFIED_VISUAL_PENDING`，外觀需人工檢查。

## 0.8.57：連線與既有 DWG 副本

- 對話要求範例：「讀取專案的零件.dwg，列出文字與圖層；確認後，把某個 Handle 的文字改成 %%c10，另存副本。」圖元位置不明確時先確認候選。
- CAD 連線失敗由桌面提供開啟指定版本／已開啟重新連線／取消；保留原計畫，十分鐘上限。途中已寫入而斷線時只能由使用者選擇另建副本，不能從未知位置接續。
- cad_read 只將受鎖定的原檔複製至全新 _AI_Output 目錄，CAD 開啟副本；原檔不交給 COM。第一頁建立完整本機索引，後續以 next_offset／revision 讀取 100 個圖元；每頁仍核對原檔與索引雜湊。
- cad_edit 重新複製原檔，檢查來源 revision 與已讀 Handle，修改後 Save／Close／Open 核對。修改只限未鎖圖層的簡單線／圓／弧／文字顏色、文字內容、線／圓／文字位移；新增線、圓、文字、引線。只讀期間不 Save，修改結果另作成果。
- 圖塊只回傳名稱、位置及可讀屬性，不展開定義或修改；ModelSpace 的數據不等於完整視覺排版。圖紙空間、外部參照、未知代理物件及未實作屬性未驗證，不宣稱完全保持整張工程圖語意。
- 初版來源 512 MB、最多 200000 圖元、64 MB 本機索引、每來源最多授予 10000 個已讀可修改 Handle。超出即提示分割；不默默裁切成完整結果。
- 新圖固定 mm；既有圖依 INSUNITS（units_code），不猜單位。直徑數字前綴使用 %%c，文字讀回容許 %%C／%%c 等價。字型外觀仍需人工查看。
- Count 與枚舉數不同時，只在唯一 Handle 集合與預期完整吻合且屬性一致時接受；報告保留 expected_count／reported_count／enumerated_count／unique_handles、缺少與額外 Handle，不能把未知多餘物件當作內部圖元略過。

## 執行與成果保護

0.8.56：浩辰 GstarCAD 與佑達 iCAD 是分別辨識的支援產品，不要求佑達程式名稱或安裝資料夾含「浩辰」。無副檔名的輸出名稱會補 .dwg；引線必須提供 text 與正數 height。

錯誤依 recovery 分類：correct_and_retry 是尚未建立文件，可補正後重試；revised_new_document 僅用於新文件內明確回報 E_INVALIDARG 的操作，可修改計畫另建新圖面；相同計畫即使換名稱也不重播。每任務最多六次執行。manual_check、程序中斷、取消與逾時仍停止；舊版已保存的未知鎖定不自動解除，升級後請開新任務測試。

intended_path 只是預定輸出位置，file_exists 是工作結束時實際存在檢查，saved 表示 SaveAs 已返回，completed_operations 表示操作呼叫返回數。失敗檔案即使存在也不是驗證通過成果；未儲存的新圖面可能只在 CAD 記憶體中。部分圖面不自動關閉或修改，另建圖面亦不代表修復舊圖面。

舊版點集合使用 Add(x,y,z)，避免對 Item 索引基準的假設。參考 [IntelliCAD Points.Add 官方介面](https://www.progesoft.com/products/progecad-professional/manual?mp=developer-reference/api-reference/objects/points-collection/methods/add-method)；該文件為較新版本，2008 實際相容性仍須公司測試。

- 只接受固定 JSON，不執行模型 Python、Shell、LISP、任意 COM 名稱或外部檔案路徑。
- CAD 專用 runtime 位於 EXE 旁內容指紋命名的 `cad-<指紋>`，與分析用 Python 分離；啟動前逐檔雜湊核對並保留唯讀 handle。
- 以原登錄 CLSID 連接並核對實際應用程式路徑。佑達 ICAD 2008 身份依實機資料精確匹配，不把所有 Icad ProgID 都當成同產品。
- 使用新資料夾與 StageGuard；成功檔案記錄 SHA256，任務恢復及交付前再次核對。既有工作副本／檔案工具不擴大到任意 DWG 編輯。
- 同一應用程式序列化 CAD 工作，240 秒逾時或取消只終止自己的 Python。CAD 可能仍留部分新文件；同任務後續繪圖會被阻擋，不自動重送。請人工檢查後開新任務。
- `cad-report.json` 保存完成或已回傳的錯誤階段；程序逾時／崩潰時可能沒有報告，不能推定未執行。

## 建置與驗證

`python tools/build_cad_runtime.py` 使用 `tools/gstarcad-probe/dependencies.json` 的官方 SHA256 鎖定輸入；本機下載不存在時從既有 `offline/GstarCAD-Probe.zip` 的 rebuild-inputs 取回。重新產生 assets CAD manifest 後審查差異，再執行 `scripts/Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller`。

來源為官方 CPython 3.13.12 x64／pywin32 311 wheel，上游二進位不是本機 v142 編譯；Rust／C++ 仍使用指定 v142。無新增 Cargo 套件，保留 Cargo.lock、vendor 及 .cargo/config.toml。

`tools/test_cad_basic.py` 使用 stage runtime 執行，涵蓋公式、顏色、引線、排序與替身屬性讀回。`--cad-self-check` 驗證固定 runtime、載入及真實匿名管線／唯讀 COM 登錄枚舉，不建立圖面。

完整實機 2008／2022、AI 模型規劃品質、公司加密與色彩呈現，以及 NSIS 安裝／更新／回復／解除安裝留待下一輪。內容指紋目錄讓新版不覆寫舊 CAD runtime；解除安裝僅移除當版清單，將來跨 CAD 指紋更新可能留下舊 runtime，不能遞迴移除未知資料。

## 0.8.70 新圖分段計畫

新cad-new任務先確認必要條件、保存短方案，再用cad_plan_start固定檔名與CAD版本。每次cad_plan_part只保存1–20項操作；每段有穩定part_id及簡短構件／座標範圍說明，程式每輪提供最新plan_revision與區段目錄。精確參數保存在本機checkpoint，不需要在模型筆記重建全部幾何。

cad_plan_inspect可取單段原參數；以相同ID、最新版本整段替換，其他段不變。完全相同重送不重複追加，過期修改拒絕。全部必要幾何、尺寸與註記齊備後cad_plan_commit只送識別碼及版本；程式合併後原生預驗整份計畫，最後一次建立同一張新DWG並存檔重開，不會把多次cad_draw的新圖冒充同圖追加。

最近新增或查回的四段精確參數另以recent_details常駐，合計最多32KB，可同時參考牆線與家具。工作集只保存區段ID並從最新版本投影，替換後不留舊座標；超額明列省略ID，不刪除原計畫。完整常駐的區段不再公告多餘查詢，未列的舊段仍可按需取回。查回資料將預設線型表示為公開Schema使用的null，原生參數及實際繪圖設定不改。

最多40段、400項及500KB；4000圖元與十分鐘原生執行界線仍保留。舊直接cad_draw及其他新增入口仍最多100項。區段暫存不連線CAD，不代表已繪製或已存檔。幾何正確性、字型／標註可讀性及工程設計仍須另外核對，分段不是完整設計驗收。
