# Python 分析入口

報告數值：統計值放result的具名數字欄位，成功後register_report_values以/summary/...引用，指定小數位與單位；缺失值先明確處理。Word／Excel切office-edit，建立表格後read_file取得block_id，再apply_report_values直接填欄，不能轉抄或猜額外min/max/std。圖表用export_report_values的CSV作圖，量尺不同單位分開。交付會核對綁定欄位與來源版本；數字一致不代表公式符合需求。
用 run_python 完成跨檔統計、合併、文字分析及產出表格；不必先讀所有參考頁。
inputs.revision可沿用read_file回傳的同檔SHA，或既有sha256:/log:表示；只有同一原始位元組雜湊才相容，檔案變更仍拒絕，不拿其他區段或資料集版本代用。
inputs 必填 name/path/kind；dataset/csv→tables[name]，text→texts[name]，生成的xlsx→tables['name/工作表名']，來源資訊在metadata[name]。CSV欄位起初是文字；資料集保留型別與原欄字母。
原始Office先切excel-read經COM匯出CSV，再回本技能。不能open、連網、啟動程序或執行來源程式。pd/np/openpyxl/io已提供，可import標準re/json等。
每次新程序，120秒／1GiB；純文字text最多32份，其他8份；單檔32MiB、序列化合計60MiB。不要默默抽樣或截斷。
result放精簡JSON（12000字元）；print最多8000字元。emit_table(name,df)輸出追蹤CSV（10萬列／16欄）；emit_excel(name,{sheet:df})輸出XLSX（12表、每表10萬列／32欄、8MiB）；每次最多8份成果。成功成果自動交付，finish.artifacts不填檔案路徑。
保留來源列號及輸入／成功／排除／失敗／未配對筆數；缺值不補零，批號不轉數字。日期系統與數值意義先核對，轉換錯誤不可靜默忽略。
只按需要 read_skill_guide：python-tables（合併與表格範例）、python-logs（編碼、超大LOG分段與跨段狀態）、python-source（程式碼唯讀分析）。圖表切dataset-charts；不為查方法而讀完全部參考頁。

未知副檔名也可嘗試一般文字閱讀或run_python(kind=text)，依實際編碼驗證；不要因副檔名未列在範例就拒絕。二進位／無法確認編碼須如實回報，專用文件／CAD／圖片走原工具。修改使用一般文字工作副本（200KB），原件唯讀、不自動執行來源。list_files按next_offset續頁，null代表結束。
