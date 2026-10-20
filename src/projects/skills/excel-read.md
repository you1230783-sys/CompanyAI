# Excel 選欄與分析規劃

「X／Y重新編號或平移」是圖表衍生座標，不是更换來源欄位。沿用原規劃與chart_dataset取得chart_index後，使用charts技能的transform_chart完成；不得以不存在的CSV欄位、create_chart或重新抄數列繞過核對。

1. 分析或畫圖先 inspect_excel 核對每個檔案自己的工作表、表頭、excel: 版本與 used_range。需要樣本時 read_excel_range，只讀少量必要欄；最多2000資料格／批。不要把檔案A的欄字母直接套到檔案B。
2. 由你理解使用者用詞，再呼叫 plan_excel_analysis：purpose 保留使用者需求原文；reason 簡述用詞和表頭／樣本的對應。分開指定 x、y、time（篩選欄）；每欄帶原始大寫欄字母與真實表頭。例：「透光值」可能是 D／圖樣Mean值；B／紀錄時間應供時間篩選或X，不能因 Value2 為數字就選為量測Y。別名不是固定字典；若多欄都合理，先 ask_user。
3. y_kind=measurement 是量測值；只有使用者確實要求時間作Y才用time，不為通過檢查而改用途。time_mode=time_of_day 表示一天中的時間，elapsed 表示經過時間；HH:MM 永遠表示時:分。含日期的序號不能默默去日期；資料意義不清楚先詢問。
4. 沿用 plan_id 呼叫 export_planned_excel，指定表頭後 start_row、scan_rows 與 window。時間由本機篩選，起點包含、終點不含，例如12:00–13:00；time_of_day的終點可24:00，終點小於起點表示跨午夜。無篩選填window=null。最多掃描250000列／120秒，符合最多10000列／9欄／90000格，超過需縮小範圍或分批，不自行抽樣。依used_range涵蓋要求；scan_complete=false時沿next_source_row完成剩餘範圍，不把部分結果當作完整時段。
5. chart_dataset 直接讀CSV；x_column/y_columns 必須沿用規劃。只選B/D仍叫B/D，不重新編成A/B。CSV保存表頭、格式、來源版本、規劃與時間條件；精簡上下文後用inspect_dataset取回，不重抄所有數值。更正需求需重新規劃並匯出；不要改走create_chart／舊工具繞過鎖定。
6. 多檔案多時段：每檔核對並規劃一次，每個時段分別匯出及作圖。標題／檔名包含來源與時段，保留完成清單；同一plan只可用於同檔同版本。3檔×5時段應產生15張，沒有資料或失敗的組合明確列出，不能當作成功。

畫圖先載入charts。Excel Value2是原值，text是顯示文字，number_format是格式；時間的小數是儲存表示，不是量測值。Y空白保留null，文字／錯誤沿用桌面異常值處理，不猜成0。公式只讀當次結果，不刷新外部連結。一般閱讀依next_row續讀，不將選欄視為全文。未儲存小型工作副本沿用read_file／chart_from_excel；已儲存分析走上述規劃流程。
