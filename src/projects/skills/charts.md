---
name: charts
description: 使用折線、長條、散佈圖呈現可核對資料。
---

# 資料圖表

create_chart 使用固定資料結構，種類僅 line/bar/scatter；Excel 優先用 chart_from_excel。標明橫軸、縱軸單位與來源。缺失用 null，禁止當成零；散佈圖橫軸必須數字。不可提供 JavaScript、HTML、ECharts option 或函式。圖表只是呈現已取得的資料，不會自動證明因果；多系列應共用單位。完成後告知使用者圖表卡片與可展開資料表，不宣稱已匯入 Office。
