# Excel 選欄讀取

Excel 大量資料：先 inspect_excel 取得工作表、表頭與 excel: 版本，再 read_excel_range 選 columns（可不連續，例如 ["A","F"]）、start_row、row_count（預設 100，最多 2000 資料格／批）。只讀所需欄，依 next_row 續讀；表頭不一定第 1 列，必要時指定 header_row。部分欄／列不代表全文已讀，不要求全文件摘要；保留有用的範圍與結論即可。畫圖優先 export_excel_dataset 將已確認欄位及範圍保存為本地 CSV，再 chart_dataset 直接讀 CSV；不要為畫圖逐頁傳回所有數值。chart_excel_range 保留作使用者明確要求不建立 CSV 時的直接繪圖方式。全期間要求不能擅自只取前 100 列；單張最多 10000 筆、8 個 Y 系列，X 加 Y 最多 90000 格（與一般閱讀 2000 格分開），超過時分圖或詢問範圍，不自行抽樣。Y 空白保留 null；畫圖異常值由桌面詢問使用者，不自行填零；公式可讀 Excel 提供的數值，日期 Value2 是序號，text 為格式化文字。這些工具只讀已儲存路徑；未儲存小型工作副本沿用 read_file／chart_from_excel。大型文件編輯仍受原快照上限。


畫圖先載入 charts 技能。
