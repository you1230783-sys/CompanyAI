---
name: charts
description: 使用折線、長條、散佈圖呈現可核對資料。
---

# 資料圖表

create_chart 使用固定資料結構，種類僅 line/bar/scatter；已儲存 Excel 優先用 inspect_excel＋export_excel_dataset＋chart_dataset（未儲存的小型工作副本才用 chart_from_excel）。標明橫軸、縱軸單位與來源。缺失用 null，禁止當成零；散佈圖橫軸必須數字。不可提供 JavaScript、HTML、ECharts option 或函式。圖表只是呈現已取得的資料，不會自動證明因果；多系列應共用單位。完成後告知使用者圖表卡片與可展開資料表，不宣稱已匯入 Office。

先讀表頭確定欄位及單位。只需 A／F 時，read_excel_range 的 columns 用 ["A","F"]，不要讀 A:F；試看 100 列只供了解資料，不能冒稱整段趨勢。export_excel_dataset 使用同一個 excel: 版本擷取完整選取資料，模型只見首尾預覽；之後 chart_dataset 用 CSV path／revision 作圖或更正欄位，不再重讀全部原始資料。chart_excel_range 僅作使用者明確要求不建立 CSV 的直接繪圖方式。保留原列號、空值與選取範圍；版本改變需重新查表頭。單張最多 10000 筆、8 個 Y 系列；直接 Excel 畫圖的 X 加 Y 合計最多 90000 格，一般 read_excel_range 仍限 2000 格；更多資料分圖或先詢問範圍，不默默抽樣。

使用者要求圖片檔時，取得建圖工具回傳的 chart_index，再呼叫 export_chart_png，name 使用可辨識的 .png 檔名。PNG 為 1600×1000 白底、包含標題／座標／圖例／來源的完整範圍；不是目前缩放區域截圖。逐張匯出，每次成功後保留 path；無工作副本的 finish.artifacts 為 []，PNG 由程式自動併入成果。失敗不得宣稱已存圖。

需要 Office 圖文成果時，先將圖表匯出 PNG，再在新建或既有工作副本用 office_action 的 insert_image 操作；Word 建議 width:320、target:{kind:"word",before:null}；Excel 指定空白區域如 H2；PPT 指定投影片與 left/top 並避開文字。插入成功後以新 revision 呼叫 save_copy，不把工作版本成功等同已儲存；交付後移動 PNG 不影響已嵌入圖片。

Excel Y 的明確數字文字（如 339）自動轉數值；NG、X、格式不明、錯誤及合併格由桌面列出類型／數量／範例，等待使用者選擇缺值、略過或設零。原空白一直保持缺值，類別 X 的 00123 不轉數字。不要自行替使用者決定、填零或另造圖表繞過等待。需修改 Office 時先 load_skill(office-edit)。
