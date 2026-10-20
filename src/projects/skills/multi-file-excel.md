---
name: multi-file-excel
description: 依共同欄位抽取多份資料並建立可追溯的比較表。
---

# 多文件抽取成 Excel

先確定共同欄位、單位與一列代表什麼，再 search_files / read_file 取得各來源。每列保留來源檔、revision、區段與必要原文。缺值留空或明確寫未提供，不填零、不猜測；不同單位先標記，未有可靠換算規則不混算。create_working_copy(name=新檔.xlsx, source=null) 新建 Excel，用 office_batch / office_action 的既有 Excel 操作批次填表與設定基本格式。文字不得作為公式執行，公式只採明確需求與已支援操作。核對列數、重複項、欄位與數字後 save_copy。需要趨勢圖時優先以 chart_from_excel 從已讀版本的儲存格建立圖表，避免重抄數字。
