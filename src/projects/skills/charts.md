---
name: charts
description: 使用折線、長條、散佈圖呈現可核對資料。
---

# 資料圖表

使用者要求 X／Y 平移、重設 Index 或移除無值位置時，先依來源建立圖表，再呼叫 transform_chart(chart_index,transform)，不要重建假 CSV 欄位或手抄 create_chart。X／Y 指圖上的實體軸；horizontal_bar 的類別在 Y。兩軸都可選 original、offset（原值加 offset）、index（依保留列順序 start＋step×序號）；一般從1起算即 start:1、step:1。每次設定都以原始資料計算，重複呼叫不累加位移。原值模式其餘欄填 offset:0、start:1、step:1。

drop_empty:true 只移除所有系列都缺值的整列，0 是有效值；先篩列，再共用保留列序號轉換兩軸，單一系列缺值仍保留缺值，不自行補點。重新編號量測軸會改變資料意義，只在使用者要求時使用，說明衍生座標。文字類別不能直接平移，但可明確重新編號。例：要求 X 從1開始且刪除無值，用 x:{mode:"index",offset:0,start:1,step:1}，y:{mode:"original",offset:0,start:1,step:1}，drop_empty:true。若只要求從1開始但保留間距，數值軸可用offset:1−原首值。轉換成功後才export_chart_png；舊PNG不覆寫，新圖會另存。

create_chart 使用固定資料結構，種類僅 line/bar/scatter/step/area/horizontal_bar；已儲存 Excel 優先用 inspect_excel＋export_excel_dataset＋chart_dataset（未儲存的小型工作副本才用 chart_from_excel）。標明橫軸、縱軸單位與來源。缺失用 null，禁止當成零；散佈圖橫軸必須數字。不可提供 JavaScript、HTML、ECharts option 或函式。圖表只是呈現已取得的資料，不會自動證明因果；多系列應共用單位。完成後告知使用者圖表卡片與可展開資料表，不宣稱已匯入 Office。

先讀表頭確定欄位及單位。只需 A／F 時，read_excel_range 的 columns 用 ["A","F"]，不要讀 A:F；試看 100 列只供了解資料，不能冒稱整段趨勢。export_excel_dataset 使用同一個 excel: 版本擷取完整選取資料，模型只見首尾預覽；之後 chart_dataset 用 CSV path／revision 作圖或更正欄位，不再重讀全部原始資料。chart_excel_range 僅作使用者明確要求不建立 CSV 的直接繪圖方式。保留原列號、空值與選取範圍；版本改變需重新查表頭。單張最多 10000 筆、8 個 Y 系列；直接 Excel 畫圖的 X 加 Y 合計最多 90000 格，一般 read_excel_range 仍限 2000 格；更多資料分圖或先詢問範圍，不默默抽樣。

使用者要求圖片檔時，取得建圖工具回傳的 chart_index，再呼叫 export_chart_png，name 使用可辨識的 .png 檔名。PNG 為 1600×1000 白底、包含標題／座標／圖例／來源的完整範圍；不是目前缩放區域截圖。逐張匯出，每次成功後保留 path；無工作副本的 finish.artifacts 為 []，PNG 由程式自動併入成果。失敗不得宣稱已存圖。

需要 Office 圖文成果時，先將圖表匯出 PNG，再在新建或既有工作副本用 office_action 的 insert_image 操作；Word 建議 width:320、target:{kind:"word",before:null}；Excel 指定空白區域如 H2；PPT 指定投影片與 left/top 並避開文字。插入成功後以新 revision 呼叫 save_copy，不把工作版本成功等同已儲存；交付後移動 PNG 不影響已嵌入圖片。

Excel Y 的明確數字文字自動轉數值；原空白、NG、錯誤及合併格先保留缺值，桌面以非阻塞卡片詢問偏好。未回答繼續其他工作，完成時公告預設；不得自行填零影響統計。set_chart_policy 可依明確回答切換 blank／invalid 的 gap、skip、zero；完成後的編輯器也能切換。原值與異常明細保留，改政策後需要PNG時重新匯出。無效 X 預設暫不建立圖，先完成其他工作，只有使用者明確選擇才排除整列；deferred 不等於已產圖。類別 X 的 00123 不轉數字。需修改 Office 時切換 office-edit。

Excel分析必須依excel-read技能先plan_excel_analysis，再export_planned_excel與chart_dataset；量測Y不可誤用時間的Value2小數。不連續欄保留原始欄字母，規劃隨CSV與任務續接保存。

未指定種類時依目的選擇：趨勢用line，離散設定變化用step，表達隨時間量的大小可用area（不可暗示累積量），比較類別用bar／horizontal_bar，兩個數值變數關係用scatter。類別時間軸不可猜成數值散佈X。沒有箱形圖、直方圖或堆疊統計能力。完成後使用者可自行編輯顯示設定、參考線及儲存PNG；這些設定不提供模型修改。

參考線：建立圖表後用 set_chart_reference_lines(chart_index,lines) 整組設定（最多10條；[]清除）。每條 axis=x/y、value=有限數值、name=最多100字、color=#RRGGBB。使用目前圖上的實體座標；類別軸為0起算位置，scatter為真實數值。先完成座標轉換，再標線。參數1在[0,200)、參數2在[200,500)、其餘為3時，在X=200與500畫線；若界線來自資料欄，先核對新參數第一筆的原座標。不能默默省略切換點或重編時間。已指定同圖就不詢問，未指定的多時段先以ask_preference詢問同圖加參考線／分圖；UI另有其他文字輸入。先做獨立資料工作，需要圖時採已公告預設並於交付說明。參考線隨圖表保存，編輯器可修改或清除；先前PNG不覆寫，要新版PNG需再次export_chart_png。
