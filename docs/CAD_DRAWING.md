# CAD 對話繪圖測試版（0.8.55）

## 使用方式

安裝本版 NSIS，在專案對話中要求 CAD 繪圖。AI 可按需載入「CAD新建圖面、標註、公式曲線與顏色（測試版）」技能。電腦需要安裝可用的 CAD；Python／pywin32 隨安裝包提供。

範例：

> 使用浩辰 CAD 2022，新建 200×100 mm 的矩形，四角各距邊 15 mm，畫四個直徑 10 mm 的孔位圓。外框白色、孔圈紅色，加入長寬及直徑標註，用綠色帶箭頭引線寫「孔徑 10 mm」，文字放在水平線上方。另在旁邊以藍色畫 y=10*sin(2*pi*x/100)、x=0 到 200 的曲線，分成 100 段。存成 CAD測試.dwg。

預設選明確年份最新的版本，亦可指定 2022／2008，由 inventory 的確切 ProgID 選取。選定版本失敗不改用其他版本。連接或啟動所選 CAD，只新建圖面；成功後存檔、重開核對並關閉自己的文件。成果會列於對話並存入專案 `_AI_Output/<新資料夾>/`。

後續要求改色／尺寸時，AI 查回原計畫並產生另一份 DWG，原圖保留。第一版不支援任意既有 DWG 讀取／編輯、組立、3D 或 Solid Edge。

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

## 執行與成果保護

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
