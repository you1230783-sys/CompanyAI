# 0.8.48：圖中文字、AI編輯、本機時間與Python試用

## 本版行為

- 圖表「調整排版」新增最多20則文字，每則1–500字，支援換行、粗體／斜體／底線、固定字型清單、整數字級8–72、#RRGGBB字色及透明／指定底色。可拖曳、方向鍵微調、百分比定位、編輯及刪除；完成保存、取消還原。自動排版重設標題／圖例／參考線位置，保留新增文字。
- 標題、圖例、參考線與新增文字共用比例位置、量測及避讓規則；PNG使用相同ECharts畫法。文字放不下時保留並提示，不默默隱藏。避讓目標是文字框，不保證避開所有數據點，亦非資料座標錨點。
- 「設定 → 一般 → 圖表預設顏色」保存八系列色票；未自訂圖表同步使用，新建AI樣式也取目前預設色。已有自訂樣式的圖表保持自己的色彩，舊PNG不覆寫。舊設定缺欄位時使用預設色與自動排版。
- `chart-edit`技能新增`inspect_chart`、`edit_chart`。索引只帶圖號／標題／筆數，指定圖才回完整受控樣式與來源摘要，不傳點陣。AI讀取同對話歷史的最新手動樣式後可另建本輪版本；原訊息、來源與數據保留。既有轉換／參考線工具依目前呈現圖型處理。完成後可人工再改或重新匯出PNG。進階樣式schema按技能載入，沒有加入每輪常駐工具。
- 使用者及最終回覆保存本機YYYY-MM-DD HH:MM:SS；進度、輪次與工具事件保存完整本機時間、畫面顯示HH:MM:SS。重開不重設，舊字串活動及缺時間的歷史仍可讀取且不補造時間。日期時間不混入模型訊息正文或一般聊天API上下文。
- `python-edit`為試用技能，開放.py文字讀取、搜尋、新建及副本修改；每檔200 KB，原檔唯讀。`check_python`以固定檢查器對即將輸出的bytes偵測編碼、核對文字、AST解析及compile，未執行來源或匯入其依賴。當前副本revision通過才允許`save_copy`，再次修改需重查。
- Python結果明示`syntax_valid`、`source_executed=false`及`functional_tests_run=false`，附版本、首個錯誤行列、最多5則警告、20個函式／類別索引及總數。語法通過不代表功能、套件或其他Python版本相容。未新增pytest／pip，既有Python worker／套件／runtime指紋保持不變。

使用方式見 [使用指南](USER_GUIDE.md)，Python實作界線見 [執行環境](PYTHON_RUNTIME.md)。[前置排版設計](PENDING_CHART_LAYOUT.md)中的只改來源限制已由本版授權取代。

## 驗證狀態

2026-10-09 08:35（Asia/Taipei）完整發行流程通過，使用以下命令：

```powershell
./scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller
```


- Rust 1.98.1、MSVC v142 14.29.30133及Windows SDK 10.0.19041.0；實際C編譯器探針為`_MSC_FULL_VER=192930159 x64`。從空Cargo快取使用專案Cargo.lock、vendor及`.cargo/config.toml`，Cargo建置均採`--frozen`。
- fmt、Clippy、288項單元測試及release EXE通過。涵蓋舊設定相容、色票、文字樣式界線、歷史圖的人工設定／AI新版本、換圖型後參考線、舊時間紀錄及API不帶本機時間。
- 正式EXE的WebView2與原生輸入介面自檢通過，含圖中文字及排版的DPAPI保存／讀回與原始數列保留。26個原生工具、9個上游恢復、12個圖片案例與兩組68輪長任務通過；包含中斷後續接、舊協定、加密記憶、PDF及30份約10 MiB LOG。
- 真正透過Broker與Python AppContainer通過.py讀取、修訂副本、AST／compile範圍錯誤、語法修正、UTF-8／BOM／Big5、編碼宣告不符、未檢查／失效版本拒絕儲存、成果讀回及最終交付核對。以頂層raise與不存在的import驗證沒有執行來源程式。
- Python分析、超過77 MiB LOG、實際Excel COM → CSV → pandas、檔案／網路／子程序隔離、取消、真正120秒逾時及runtime損壞拒絕通過。
- Word／Excel／PowerPoint十種既有格式修改與重開、新建三種格式、三份Excel乘五時段的15張圖表、10000點資料、PNG嵌入與原PNG移除後重開通過。
- localhost SMB的UNC及暫時映射磁碟通過：獨立資料夾建立、文字／Office讀寫、加密筆記重載、原件保留及路徑逃逸拒絕。沒有把它當作公司共享磁碟驗收。
- 完整NSIS實際安裝、自檢、更新、PID交接啟動、失敗回復、保留使用者檔案與解除安裝通過。Python原地沿用，以及缺檔、同大小損壞、清單不符三種完整修復均通過。

可追溯報告：[環境](../offline/environment.txt)、[EXE](../offline/exe-verification.json)、[原生介面](../offline/composer-verification.json)、[Python](../offline/python-verification.json)、[Office](../offline/office-verification.json)、[SMB](../offline/network-verification.json)、[NSIS](../offline/installer-verification.json)。完整本機日誌為`.build/release-0.8.48-complete.log`，不納入交付。

已完成的來源介面驗證：

- `Test-Charts.js`：10000點／八系列、範圍、缺值政策、參考線及PNG回歸。
- `Test-ChartLayout.cjs`：實際headless Edge拖曳／鍵盤、標題／圖例／參考線、六種圖型、十條長標籤／窄圖框提示／零尺寸恢復、配色、保存／取消／JSON重載。新增文字的格式、位置、字級拒絕、刪除取消、自動排版保留、AI樣式及PNG皆通過；已目視核對文字PNG。
- 同一瀏覽器測試涵蓋跨午夜訊息日期、HH:MM:SS活動、舊資料留空、進度筆記辨識及更新時保留活動清單節點；已目視核對時間畫面。
- `Test-Settings.cjs`：一般／操作／進階頁籤、鍵盤、其他偏好、歷史診斷與晚到回覆、亮暗色及窄視窗；`Test-ReplyTables.cjs`一般對話表格回歸通過。

瀏覽器報告`.build/chart-layout-review.json`、`.build/settings-review.json`及截圖留本機，不包含公司資料，不納入安裝包。原生保存、Python、Office、SMB與安裝結果由上述完整發行流程核對。

## 交付產物

| 檔案 | 版本 | Bytes | SHA256 |
| --- | --- | ---: | --- |
| `dist/LM_AI.exe` | 0.8.48.0 | 16,501,760 | `e1025163aced465a9ec2d477b11bf92493a365659753feac740d1b96e64e61fc` |
| `dist/LM_AI_Setup.exe` | 0.8.48.0 | 28,415,277 | `4477c14de19f8ac499b7c6676cd8d32674d123d554a9913f466320be096fac2b` |

兩份`update-manifest*.json`完全相同，均為0.8.48、`kind=nsis`；大小、SHA256及簽章對應上述Setup，已通過更新清單驗證。Python worker、runtime清單／指紋、依賴包與歷史Rust離線ZIP未改動。請使用完整安裝包。


## 實測界線

維持0.48估算係數及8K–12K軟目標，不加token硬上限；新增圖表編輯schema只有切換技能時提供，既有歷史點陣不因查索引送模型。沒有使用公司真實API、郵件、共享磁碟或Python專案做本輪語意／效能驗收，不能以本機合成測試宣稱模型每次都會正確選圖、編輯程式或達成固定速度。

文字位置是圖框比例，畫面與1600×1000 PNG可能因字型及框寬重新換行、避讓。原生程式與本專案測試使用v142；既有上游CPython及wheel為預編譯品，其來源編譯器另記於Python文件，不冒稱由本機v142編譯。本版不重製歷史Rust離線ZIP，完整更新請使用NSIS。
