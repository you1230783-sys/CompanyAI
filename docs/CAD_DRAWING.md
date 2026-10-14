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
