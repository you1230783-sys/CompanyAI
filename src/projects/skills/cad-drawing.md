# CAD 二維繪圖測試版

使用者要求 CAD 繪圖時使用本技能。cad_inventory 唯讀列出 COM 版本，由新到舊排序。cad_draw 接受一整份固定結構的繪圖計畫，連接或啟動指定 CAD，只建立新 DWG，存檔重開核對，關閉這份新文件。使用者現有圖面不作為輸入，也不修改。

支援產品包含浩辰 GstarCAD 與已辨識的佑達 iCAD 2008（Icad.Application.1）。佑達 iCAD 不必含「浩辰」字樣；不要因品牌名稱不同拒絕使用者已指定的版本，也不要將兩者說成同一產品。以掃描結果及實際身份核對為準，不能擴大宣稱所有 IntelliCAD 皆相容。

- 預設 version="latest"；使用者指定版本時先 cad_inventory，以確切 progid 選取。新版連接失敗不得偷偷改用舊版。
- 單位固定 mm，平面 XY。每份 1–100 個操作、合計最多 4000 個圖元，每任務最多十份 DWG。
- name 是單一 .dwg 檔名，例如「測試.dwg」；未提供副檔名時桌面補上 .dwg。輸出路徑由桌面決定。每次建立新目錄，不覆寫既有成果。
- 顏色為 ACI 色碼 1–255：1紅、2黃、3綠、4青、5藍、6洋紅、7隨背景顯示白或黑、8深灰、9淺灰。可依使用者要求替每個操作指定不同 color；引線文字及箭頭、剖面邊界及孔圈使用該操作同色。第一版不支援 RGB。
- line：start/end；circle：center/radius；text：position/text/height；mtext：position/text/height/width，換行使用 \\P。
- aligned_dimension：start/end/location；diameter_dimension：start/end 為圓的相對端點，leader_length 大於零。
- leader 必填 kind/color/points/text/height，height 是正數文字高度（例如 3.5 mm），不能省略。points 三點分別為箭頭、轉折、水平段末端；第二與第三點 y 相同。自動帶箭頭，文字放在水平段左端上方，文字與引線不關聯。依文字長度留足水平段。
- hatch：boundary 是有序不自交外框頂點（不重複首點），holes 為外框內互不相交圓孔，seed 在填充區內且不在孔內。圖樣固定 ANSI31；scale 為正值。請自行核對幾何關係，不能以程式未報錯宣稱外觀合格。
- curve：x/y 為 t 的數學公式，t_start < t_end、segments 2–1000。支援 + - * / **、sin/cos/tan/sqrt/exp/log/abs、pi/e。禁止 Python 程式、屬性、索引。結果是取樣直線近似，不是原生 spline，也沒有自動誤差界線；不能宣稱連續曲線精度已保證。
- 正弦例：x="t", y="10*sin(2*pi*t/100)", t_start=0,t_end=200,segments=100。
- 螺旋例：x="(5+2*t)*cos(t)", y="(5+2*t)*sin(t)", t_start=0,t_end=12.566370614359172,segments=128。

修改先前圖面的顏色／幾何：查回原 cad_draw 計畫，在新的 cad_draw 計畫調整並輸出另一份 DWG；不聲稱原檔已修改。既有任意 DWG 讀取／編輯、3D、Solid Edge 不支援。

回傳 DATA_VERIFIED_VISUAL_PENDING 表示存檔重開及指定屬性核對完成，外觀仍需人工確認。2008 部分標註 Measurement 未實作，需人工核對。不要把獨立測試成功宣稱為所有版本／所有計畫驗收。finish 的 artifacts 僅列既有工作副本 ID，DWG 由桌面自動加入成果，不把 DWG 路徑放入 artifacts。

失敗必須先讀診斷的 recovery：correct_and_retry 可補正參數，連線問題須先確認使用者已開啟正確 CAD，不連續盲試；revised_new_document 可依已知失敗原因修正計畫，另建新圖面，不能原樣重播或把替代線段說成原生引線；manual_check 或 blocked_after_error=true 必須停止。每任務最多六次執行，不以換檔名規避相同失敗計畫限制。部分圖面留在 CAD 供人工查看，不修改或關閉。

intended_path 只是預定路徑；file_exists=false 不能稱「磁碟上已有部分成品」，saved=false 表示未完成 SaveAs。completed_operations 只表示呼叫返回，並非已驗證幾何。只有成功且有 path 的結果才可交付 DWG；無檔案時請使用者查看 CAD 中未儲存的新圖面，不要求開啟不存在的檔案。新任務不保證連線或存檔成功。
