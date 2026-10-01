---
name: charts
description: 使用折線、長條、散佈圖呈現可核對資料。
---

# 資料圖表

create_chart 使用固定資料結構，種類僅 line/bar/scatter；已儲存 Excel 優先用 inspect_excel＋chart_excel_range（未儲存的小型工作副本才用 chart_from_excel）。標明橫軸、縱軸單位與來源。缺失用 null，禁止當成零；散佈圖橫軸必須數字。不可提供 JavaScript、HTML、ECharts option 或函式。圖表只是呈現已取得的資料，不會自動證明因果；多系列應共用單位。完成後告知使用者圖表卡片與可展開資料表，不宣稱已匯入 Office。

先讀表頭確定欄位及單位。只需 A／F 時，read_excel_range 的 columns 用 ["A","F"]，不要讀 A:F；試看 100 列只供了解資料，不能冒稱整段趨勢。chart_excel_range 直接以同一個 excel: 版本取得所選資料，不需要模型搬運數字。保留原列號、空值與選取範圍；版本改變需重新查表頭。單張最多 1000 筆，X 加 Y 合計每次最多 2000 格；更多資料分圖或先詢問範圍，不默默抽樣。
