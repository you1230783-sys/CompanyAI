# CAD 二維繪圖測試版

使用者要求 CAD 繪圖時使用本技能。cad_inventory 唯讀列出 COM 版本，由新到舊排序。cad_draw 建立新圖；cad_read 讀取既有 DWG 的工作副本；cad_edit 修改全新副本。原始檔案不交給 CAD 開啟、不修改。連線失敗由桌面提示使用者開啟／重試／取消，保留原計畫，不要要求使用者另開任務或自行反覆送出工具。

支援產品包含浩辰 GstarCAD 與已辨識的佑達 iCAD 2008（Icad.Application.1）。佑達 iCAD 不必含「浩辰」字樣；不要因品牌名稱不同拒絕使用者已指定的版本，也不要將兩者說成同一產品。以掃描結果及實際身份核對為準，不能擴大宣稱所有 IntelliCAD 皆相容。

- 預設 version="latest"；使用者指定版本時先 cad_inventory，以確切 progid 選取。新版連接失敗不得偷偷改用舊版。
- cad_draw 新圖單位 mm，平面 XY。副本操作沿用 cad_read 的 units_code；0 或未知單位須先問使用者，不猜毫米。每份 1–100 個新增操作、合計最多 4000 個圖元，每任務最多六次繪圖執行。
- 直徑文字使用 %%c10；程式會把數字前的 Ø／Φ／⌀ 轉成 %%c。一般文字內容不任意轉碼。文字回讀容許 %%C／%%c 等價，不代表已驗證字型外觀。
- name 是單一 .dwg 檔名，例如「測試.dwg」；未提供副檔名時桌面補上 .dwg。輸出路徑由桌面決定。每次建立新目錄，不覆寫既有成果。
- 顏色為 ACI 色碼 1–255：1紅、2黃、3綠、4青、5藍、6洋紅、7隨背景顯示白或黑、8深灰、9淺灰。可依使用者要求替每個操作指定不同 color；引線文字及箭頭、剖面邊界及孔圈使用該操作同色。第一版不支援 RGB。
- line：start/end；circle：center/radius；text：position/text/height；mtext：position/text/height/width，換行使用 \\P。
- aligned_dimension：start/end/location；diameter_dimension：start/end 為圓的相對端點，leader_length 大於零。
- leader 必填 kind/color/points/text/height，height 是正數文字高度（例如 3.5 mm），不能省略。points 三點分別為箭頭、轉折、水平段末端；第二與第三點 y 相同。自動帶箭頭，文字放在水平段左端上方，文字與引線不關聯。依文字長度留足水平段。
- hatch：boundary 是有序不自交外框頂點（不重複首點），holes 為外框內互不相交圓孔，seed 在填充區內且不在孔內。圖樣固定 ANSI31；scale 為正值。請自行核對幾何關係，不能以程式未報錯宣稱外觀合格。
- curve：x/y 為 t 的數學公式，t_start < t_end、segments 2–1000。支援 + - * / **、sin/cos/tan/sqrt/exp/log/abs、pi/e。禁止 Python 程式、屬性、索引。結果是取樣直線近似，不是原生 spline，也沒有自動誤差界線；不能宣稱連續曲線精度已保證。
- 正弦例：x="t", y="10*sin(2*pi*t/100)", t_start=0,t_end=200,segments=100。
- 螺旋例：x="(5+2*t)*cos(t)", y="(5+2*t)*sin(t)", t_start=0,t_end=12.566370614359172,segments=128。

修改先前圖面的顏色／幾何可重新產生新圖，或使用以下副本流程；不聲稱原檔已修改。3D、Solid Edge、圖塊展開、輪廓文字辨識及短線轉曲線不支援。

## 既有 DWG 副本流程

1. 使用者指定專案內 DWG 路徑；list_files 會列出 cad_dwg。cad_read(path,version,offset=0,revision=null) 先複製後讀取，回傳 source/revision、類型／圖層統計與第一頁 100 個圖元。讀取只涵蓋 ModelSpace，非整張工程圖視覺理解。
2. 按 next_offset 與相同 revision 續讀。本機索引只建立一次，不每頁重新複製大檔；來源或索引變更就需從第一頁重讀。初版限制 512 MB、200000 圖元、64 MB 索引。圖塊提供名稱、位置與可讀屬性，不展開定義／巢狀內容；圖紙空間不在範圍。
3. 依實際回傳的 handle、Layer、座標與文字定位。只能修改 editable=true 且已讀取的 Handle；位置描述不明確時先列出候選問使用者，不猜。
4. cad_edit(path,revision,name,edits,operations) 建立另一份新副本。edits 格式 {handle,kind,value}，kind=text 時 value 是文字（限文字圖元）；color 時為 ACI 1–255；move 時為 [dx,dy]（限線／圓／文字）。同 Handle 每次僅一項修改，圖層鎖定即停止。可用 operations 新增 line/circle/text/leader，無新增時填 []。
5. 修改成功會儲存／重開、核對 Handle 集合與可讀屬性，交付 path。未實作屬性、圖塊定義／外部參照／圖紙空間不宣稱驗證完整。若要繼續修改成果，重新 cad_read 該成果取得新 revision，再產生另一份副本。

回傳 DATA_VERIFIED_VISUAL_PENDING 表示存檔重開及指定屬性核對完成，外觀仍需人工確認。2008 部分標註 Measurement 未實作，需人工核對。不要把獨立測試成功宣稱為所有版本／所有計畫驗收。finish 的 artifacts 僅列既有工作副本 ID，DWG 由桌面自動加入成果，不把 DWG 路徑放入 artifacts。

失敗必須先讀診斷的 recovery：correct_and_retry 可補正參數；revised_new_document 可修改新建圖計畫另畫，不把替代線段說成原生引線；user_requested_new_document 表示使用者已透過桌面選擇另建副本重做，可重新呼叫一次工具，不能接續舊半成品；manual_check 或 blocked_after_error=true 必須停止。每任務最多六次執行，不以換檔名規避相同失敗計畫限制。取消後不自動重送。部分圖面留在 CAD 供人工查看，不修改或關閉。

intended_path 只是預定路徑；file_exists=false 不能稱「磁碟上已有部分成品」，saved=false 表示未完成 SaveAs。completed_operations 只表示呼叫返回，並非已驗證幾何。只有成功且有 path 的結果才可交付 DWG；無檔案時請使用者查看 CAD 中未儲存的新圖面，不要求開啟不存在的檔案。新任務不保證連線或存檔成功。
