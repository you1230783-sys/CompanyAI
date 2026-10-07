# 本地資料集與畫圖

- 來源欄位核對不禁止衍生座標。要求從1重新編號、X／Y平移或移除無值時，仍沿用真實欄位完成chart_dataset，再以回傳chart_index呼叫transform_chart；規則見charts技能。無需重新匯出CSV，也不要用不存在的Index欄繞過檢查。原CSV與來源Index保持不變。

- 用戶要求 Excel／LOG 畫圖時，優先「少量確認 → 本地批次匯出 CSV → chart_dataset」。不要逐頁把所有數值送給模型再手工拼 create_chart。
- 先核對欄位意義、單位與 X/Y；LOG 約略時間沿用前後 5／15／30 分鐘尋找錨點，再以確認範圍匯出。未知格式先 read_log 看樣本；格式複雜或需要跨行配對時載入python-analysis，用受控Python處理已確認的結構。欄位或事件意義不明才ask_user，不猜欄位、不執行外部腳本。
- export_log_dataset 的 delimited 是固定字串切割（不解讀引號）；between 使用第一個 start 之後到第一個 end，找不到則保留空格；timestamp_seconds 是當日秒數，跨日不得冒充連續時間。時間／日期篩選規則和 search_logs 相同；context_lines 必須 0。terms 為任一關鍵字符合（OR）。
- Excel 先 inspect_excel，再 plan_excel_analysis 分開鎖定篩選時間、X、Y，由 export_planned_excel 本地篩選及匯出。多檔各自規劃，每時段各自匯出。每份最多 10000 列／9 欄／90000 格，超過請用不同 CSV 分批。保留 Value2、原型別、顯示文字與來源列；公式只保存當次計算值，不執行新公式或刷新連結。
- CSV 保存到 _AI_Output；回覆只有 path、revision、筆數、欄位、首尾各 10 筆與統計（預覽每格最多 80 字）。中間資料未送出，不等於遺失；畫圖會由本地讀取完整指定範圍。
- 請記住 CSV 路徑、revision、欄位意義、單位、篩選條件、總筆數。後續對話只帶這份索引／簡介，必要時 inspect_dataset（跨任務只知道路徑時可省略 revision，先取得目前版本再畫圖）；不要重讀全部 LOG／Excel／CSV，也不要把完整座標或原始資料貼到筆記或最終答案。
- chart_dataset 的 start_row 是 CSV 的資料列（1 起算、不含表頭），最多 10000 點／8 系列，不自動抽樣；大資料集請分圖或先明確縮小範圍。工具回覆不包含 x／series 陣列。
- LOG 明確有限數字由本地辨識為 number，原字串仍保存；文字與缺值不猜成 0。CSV 前六個 __ 欄保存來源、型別、原顯示文字，不作一般 X/Y 欄。異常值沿用桌面選擇，查看異常值會顯示原檔及行／列號。已帶規劃的CSV禁止更換X/Y；更正需求必須重新規劃與匯出。未帶規劃的舊CSV才可重用其他已含欄位。舊圖表不再作為正確結論。
- 匯出成功後程式會先封存工具結果，再移出前面原文。需要進一步整理或否定舊結論，用 compact_context 保存精簡交接，不重新抄入資料。
- CSV 自動納入成果，不要把 CSV 路徑當作 finish.artifacts 的 copy_id。原檔不修改。這是本工具生成的來源追蹤 CSV，尚非任意 CSV 匯入器。

Excel CSV 的 excel_schema 保存真實表頭、格式、規劃及篩選條件；前六個追蹤欄中的 __display 第一筆可為version=2物件，其餘仍為顯示文字陣列。以inspect_dataset／chart_dataset讀取，不自行解析並重新編號。時間區間為起點含、終點不含，需向使用者說明實際使用範圍。
