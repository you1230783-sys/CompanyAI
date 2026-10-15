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

# CAD 二維繪圖測試版

使用者要求 CAD 繪圖時使用本技能。cad_inventory 唯讀列出 COM 版本，由新到舊排序。cad_draw 建立新圖；cad_read 讀取既有 DWG 的工作副本；cad_edit 修改全新副本。磁碟副本模式不開啟或修改原檔；使用者要求編輯目前圖面時改走 cad_active_read/cad_active_edit，原生確認後直接修改記憶體，不儲存。連線失敗由桌面提示使用者開啟／重試／取消，保留原計畫，不要要求使用者另開任務或自行反覆送出工具。

支援產品包含浩辰 GstarCAD 與已辨識的佑達 iCAD 2008（Icad.Application.1）。佑達 iCAD 不必含「浩辰」字樣；不要因品牌名稱不同拒絕使用者已指定的版本，也不要將兩者說成同一產品。以掃描結果及實際身份核對為準，不能擴大宣稱所有 IntelliCAD 皆相容。

- 預設 version="latest"；使用者指定版本時先 cad_inventory，以確切 progid 選取。新版連接失敗不得偷偷改用舊版。
- cad_draw 新圖單位 mm，平面 XY。副本操作沿用 cad_read 的 units_code；0 或未知單位須先問使用者，不猜毫米。每份 1–100 個新增操作、合計最多 4000 個圖元，每任務最多六次繪圖執行。
- 直徑文字使用 %%c10；程式會把數字前的 Ø／Φ／⌀ 轉成 %%c。一般文字內容不任意轉碼。文字回讀容許 %%C／%%c 等價，不代表已驗證字型外觀。
- name 是單一 .dwg 檔名，例如「測試.dwg」；未提供副檔名時桌面補上 .dwg。輸出路徑由桌面決定。每次建立新目錄，不覆寫既有成果。
- 顏色為 ACI 色碼 1–255：1紅、2黃、3綠、4青、5藍、6洋紅、7隨背景顯示白或黑、8深灰、9淺灰。可依使用者要求替每個操作指定不同 color；引線文字及箭頭、剖面邊界及孔圈使用該操作同色。第一版不支援 RGB。
- line：start/end；circle：center/radius；text：position/text/height；mtext：position/text/height/width，換行使用 \\P。
- aligned_dimension：start/end/location；diameter_dimension：start/end 為圓的相對端點，leader_length 大於零。
- leader 必填 kind/color/points/text/height，height 是正數文字高度（本版固定 3，請填 3），不能省略。points 三點分別為箭頭、轉折、水平段末端；第二與第三點 y 相同。自動帶箭頭，文字放在水平段左端上方，2008 使用原生註解，2022 使用獨立文字；關聯持久性仍須實測。依文字長度留足水平段。
- hatch：boundary 是有序不自交外框頂點（不重複首點），holes 為外框內互不相交圓孔，seed 在填充區內且不在孔內。圖樣固定 ANSI31；scale 為正值。請自行核對幾何關係，不能以程式未報錯宣稱外觀合格。
- curve：x/y 為 t 的數學公式，t_start < t_end、segments 2–1000。支援 + - * / **、sin/cos/tan/sqrt/exp/log/abs、pi/e。禁止 Python 程式、屬性、索引。結果是取樣直線近似，不是原生 spline，也沒有自動誤差界線；不能宣稱連續曲線精度已保證。
- 正弦例：x="t", y="10*sin(2*pi*t/100)", t_start=0,t_end=200,segments=100。
- 螺旋例：x="(5+2*t)*cos(t)", y="(5+2*t)*sin(t)", t_start=0,t_end=12.566370614359172,segments=128。

修改先前圖面的顏色／幾何可重新產生新圖，或使用以下副本流程；不聲稱原檔已修改。3D、Solid Edge、圖塊展開、輪廓文字辨識及短線轉曲線不支援。

## 既有 DWG 副本流程

1. 使用者指定專案內 DWG 路徑；list_files 會列出 cad_dwg。cad_read(path,version,offset=0,revision=null) 先複製後讀取，回傳 source/revision、類型／圖層統計與第一頁 100 個圖元。讀取只涵蓋 ModelSpace，非整張工程圖視覺理解。
2. 按 next_offset 與相同 revision 續讀。本機索引只建立一次，不每頁重新複製大檔；來源或索引變更就需從第一頁重讀。初版限制 512 MB、200000 圖元、64 MB 索引。圖塊提供名稱、位置與可讀屬性，不展開定義／巢狀內容；圖紙空間不在範圍。
3. 依實際回傳的 handle、Layer、座標與文字定位。只能修改 editable=true 且已讀取的 Handle；位置描述不明確時先列出候選問使用者，不猜。
4. cad_edit(path,revision,name,edits,operations) 建立另一份新副本。edits 格式 {handle,kind,value}，kind=text 時 value 是文字（限文字圖元）；color 時為 ACI 1–255；move 時為 [dx,dy]（限線／圓／文字）。radius 時正數半徑（圓／弧）；height 時正數文字高度（text/mtext）；delete 時 value=null（限 editable=true 的簡單圖元）。同 Handle 每次僅一項修改，圖層鎖定即停止。可用 operations 新增上列全部固定繪圖類型，無新增時填 []。
5. 修改成功會儲存／重開、核對 Handle 集合與可讀屬性，交付 path。未實作屬性、圖塊定義／外部參照／圖紙空間不宣稱驗證完整。若要繼續修改成果，重新 cad_read 該成果取得新 revision，再產生另一份副本。

回傳 DATA_VERIFIED_VISUAL_PENDING 表示存檔重開及指定屬性核對完成，外觀仍需人工確認。2008 部分標註 Measurement 未實作，需人工核對。不要把獨立測試成功宣稱為所有版本／所有計畫驗收。finish 的 artifacts 僅列既有工作副本 ID，DWG 由桌面自動加入成果，不把 DWG 路徑放入 artifacts。

失敗必須先讀診斷的 recovery：correct_and_retry 可補正參數；revised_new_document 可修改新建圖計畫另畫，不把替代線段說成原生引線；user_requested_new_document 表示使用者已透過桌面選擇另建副本重做，可重新呼叫一次工具，不能接續舊半成品；manual_check 或 blocked_after_error=true 必須停止。每任務最多六次執行，不以換檔名規避相同失敗計畫限制。取消後不自動重送。部分圖面留在 CAD 供人工查看，不修改或關閉。

intended_path 只是預定路徑；file_exists=false 不能稱「磁碟上已有部分成品」，saved=false 表示未完成 SaveAs。completed_operations 只表示呼叫返回，並非已驗證幾何。只有成功且有 path 的結果才可交付 DWG；無檔案時請使用者查看 CAD 中未儲存的新圖面，不要求開啟不存在的檔案。新任務不保證連線或存檔成功。

## 0.8.58 目前圖面（不儲存）

- 使用者說「目前開著的圖／不要儲存／一步一步修改」時，用 cad_active_read(version,offset=0)。它讀取 ActiveDocument 的未儲存內容，回傳 document、revision、units_code、Handle 及 next_offset。續頁 offset 使用回傳值；切換或修改文件會要求重讀。
- 定位目標後 cad_active_edit(revision,edits,operations)。編輯能力與副本共用；原生視窗顯示文件名、路徑，使用者確認後才寫入。操作期間應暫停手動編輯／切換文件。
- 每輪重新讀取目前圖面才能再修改；不要沿用前一輪 revision，不從舊任務還原授權。文件 HWND／程序建立時間／名稱路徑與可讀屬性都需符合。無法辨識文件視窗或不支援 StartUndoMark/EndUndoMark 的 CAD 暫不支援目前圖面寫入，可改用副本。
- 每次修改為一組 Undo。失敗不自動撤銷、不重送；保留當前畫面讓使用者檢查。僅提供本頁 0.8.61 列出的受限 CAD 指令。
- EDITED_UNSAVED 才能回報「修改完成，尚未儲存」，提醒使用者在 CAD 檢查、Undo 或自行存檔。沒有 DWG 交付路徑，不使用 save_copy、不要求手動存好才能完成任務。
- 每任務至多六次修改。使用者下一次對話可開始新任務，先重新讀取目前圖面，連續修改同一張圖而不必存檔。

## 0.8.58 Excel 座標繪圖

1. 載入 excel-read，先 inspect_excel 取得 Excel revision／表頭／樣本，確認 X/Y、半徑或標籤欄、列範圍、來源單位及目標單位。不能由列號或欄名猜測實際數值。
2. cad_excel_plan(path,revision,settings) 在本機讀取原資料並產生計畫，模型不手抄數列。settings 全部欄位如下：sheet/header_row/start_row/row_count、x_column/y_column、radius_column（無則 null）、label_column（無則 null）、kind="circles" 或 "lines"、radius（無半徑欄時預設半徑）、scale、origin=[x,y]、color、height、closed、units_code。
3. 座標 = Excel 數值 × scale + origin；半徑同乘 scale，height 已為目標圖面單位。units_code 1英吋/4mm/5cm/6m；新圖固定4。初版不接受未知單位0，請使用者先在 CAD 設定並重讀。circles 不可 closed；lines 不用 radius_column，依列順序連線，可 closed=true 首尾閉合。
4. 每次至多100列，展開後至多100個操作（圓+標籤為每列2項）；無效、空白、重複相鄰點、負半徑或超額時整批拒絕，不補0或暗中略過。標籤位於座標點，需依需求另行排版；目前不提供自動避讓。
5. 先向使用者說明欄位與轉換預覽，需求已明確時可直接套用。cad_excel_apply(plan_id,target,path,revision,name,version)：target=new 時 path/revision=null；target=active 時先 cad_active_read，revision 為圖面版本、path=null；target=copy 時先 cad_read，提供該 DWG path/revision。name 為副本單一.dwg檔名，active 不使用名稱可填空字串；version=latest 或確切 ProgID。
6. apply 再核對 Excel 原檔版本與目標圖面單位；一次性消耗計畫，失敗不可直接重播。new/copy 會保存新檔；active 僅修改目前圖面、不儲存。圖面與來源版本變更需重新讀取／預覽。單位、欄位與原點由使用者意圖確認，工具不猜測。

## 0.8.59 相容修正

- 尺寸字高與引線字高固定3、箭頭3、ISO對正（DIMTIH=0/DIMTOH=1）、文字偏移1，使用獨立simplex.shx樣式；一般文字預設填height=3，明確指定其他高度時尊重指定值。保留現有BigFont；沒有中文字型時回報，不把文字內容讀回成功當作中文字形驗收。
- 2008／2022保留原生引線；COM明示關聯且本次新建的空白／同內容文字才可納入預期集合。顯示文字仍獨立建立，不宣稱其與引線關聯。報告列出每項操作產生的圖元及額外圖元的可讀屬性；不得逕自刪除未知物件。
- 讀取缺少Document.HWND時嘗試應用視窗＋唯一文件名稱／路徑＋程序建立時間與內容快照；皆無可用視窗時仍能唯讀，editable_session=false不能修改目前文件，改用副本。
- unsupported_capability是程式／CAD介面限制，相同呼叫不可重試。讀取錯誤不代表已成功讀到圖元；不要反覆讀同一診斷／重送相同工具。
- 已讀取的尺寸／原生引線allowed_edits=[delete]，僅可依使用者要求刪除後重建；不能改它們的文字／位置或顏色。不要猜測Handle。

## 0.8.60 舊版尺寸樣式

- iCAD2008改用DimensionStyles原生介面；一般文字不依賴尺寸集合。prepare_annotation_styles失敗可能已有新空圖／樣式，但尚未進行本批幾何繪製或既有圖元刪改，依report交代，不宣稱完全沒寫入。
- 核對需求中的距邊距離與實際座標：200×100矩形左下角(0,0)，孔心各距相鄰邊15時，X應為15／185、Y為15／85；25／175代表距左右邊25。不能只抄先前筆記就聲稱符合需求。
