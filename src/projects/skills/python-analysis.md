# 離線 Python 分析（0.8.42 試驗）

適合多檔合併、分組／時間統計、LOG 事件配對、CSV 清理及產生 XLSX。一般閱讀、現成畫圖仍使用原工具。環境固定 CPython 3.13.12、pandas 2.2.3、NumPy 2.2.6、openpyxl 3.1.5；不連網、不安裝套件、不使用 PowerShell、COM 或外部程序。

## 資料與輸出

- 呼叫 run_python(purpose, code, inputs)。每次新程序、120秒計算、1GiB程序記憶體；不保存前次Python變數。來源最多8份、單檔32MiB、序列化來源合計60MiB；不擅自截斷或抽樣。原檔唯讀，資料只走管道。不要 open(path)、read_excel(path)、to_csv(path) 或直接存檔。
- inputs 每項為 name、專案相對path、kind、可選revision。kind=dataset讀原工具生成的追蹤CSV，tables[name]為保留型別的DataFrame；metadata[name]保留原始欄名、Excel格式／日期系統與逐列來源。kind=csv讀一般UTF-8 CSV，欄位先全部為文字，保留前導零及NA字樣。kind=text讀UTF-8 LOG/TXT/JSON/JSONL/OUT/ERR，內容在texts[name]。kind=xlsx只供_AI_Output內程式生成的XLSX，tables['name/工作表名']取得各表。
- 公司原始XLS/XLSX/XLSM/XLSB必須沿用Excel COM：inspect_excel／plan_excel_analysis／export_planned_excel，或已支援的選欄匯出；得到CSV後kind=dataset交给Python。不用openpyxl嘗試解密。來源版本改變先重新檢查。生成XLSX若經Excel另存而無法解析，回到COM匯出，不用改編碼假裝成功。
- pd、np、openpyxl、io已載入。可import re/json/datetime/collections/statistics等標準函式庫。程式只能分析提供的資料，不能讀其他文件、環境機密、網路或啟動其他程序。
- 最後設定result為可JSON序列化的精簡結論（最多12000字元）；print最多保留8000字元。資料未送入模型不代表未計算。錯誤先依回覆修正，使用新operation id，不無限重試。
- emit_table('結果.csv', df)發布來源追蹤CSV，最多100000列／16欄，可接inspect_dataset／chart_dataset；不在result抄完整表。emit_excel('報告.xlsx', {'統計':df,'異常':bad})以openpyxl生成多工作表，最多12表、每表100000列／32欄、8MiB；字串不執行公式。每次合計最多8份成果；工具回覆verified或dataset後才算成功。成果自動加入交付，finish.artifacts仍只填save_copy副本id。
- 衍生列是分析結果，不冒充原始列；保留批號／來源行或列欄，合併／統計後說明來源集合。缺值與錯誤分開，數字轉換用errors='raise'或明確統計失敗，不靜默dropna；批號不要轉數字。metadata中的kinds可核對error／blank。時間格式與1900／1904日期系統先確認，不把Value2時間序號當量測值。

## 常用且已驗證的寫法

分組統計及Excel成果：
```python
df = tables['data'].copy()
df['量測'] = pd.to_numeric(df['量測'], errors='raise')
summary = df.groupby('機台', dropna=False)['量測'].agg(['count','mean','min','max']).reset_index()
emit_table('統計.csv', summary)
emit_excel('分析.xlsx', {'統計':summary})
result = {'輸入筆數':len(df), '機台數':len(summary)}
```
Excel匯出欄仍使用原欄字母（如B/D），用metadata['data']['excel_schema']['headers']核對後可在記憶體明確rename。需要改原始分析欄位時重新規劃，不擅改既有CSV規劃。

多檔合併先pd.concat([tables['a'],tables['b']], ignore_index=True)；依批號關聯用pd.merge(..., on='批號', how='outer', indicator=True, validate='one_to_one')核對未配對與重複，不用inner默默丟資料。一般CSV先用小量head及columns在result確認格式；完整計算在本地。

LOG用texts['log'].splitlines()及enumerate(...,1)保留行號，以re解析已確認的事件／批號，記錄無法解析行數；計算開始／完成配對時保留取消與缺失事件。跨午夜、校時與不同批號不可憑順序猜配對。
