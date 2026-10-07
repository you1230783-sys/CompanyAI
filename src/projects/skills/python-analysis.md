# 離線 Python 分析

適合多檔合併、分組／時間統計、LOG 事件配對、CSV 清理及產生 XLSX。一般閱讀、現成畫圖仍使用原工具。環境固定 CPython 3.13.12、pandas 2.2.3、NumPy 2.2.6、openpyxl 3.1.5；不連網、不安裝套件、不使用 PowerShell、COM 或外部程序。

符合上述分析需求時主動選用，不必等使用者指定Python。先用現有工具核對少量樣本／來源欄位，再在Python完整計算；不為了展示工具而重做已完成的分析。單純座標平移、重新編號及去除全系列無值位置使用transform_chart即可。

## 資料與輸出

- 呼叫 run_python(purpose, code, inputs)。每次新程序、120秒計算、1GiB程序記憶體；不保存前次Python變數。來源最多8份、單檔32MiB、序列化來源合計60MiB；不擅自截斷或抽樣。原檔唯讀，資料只走管道。不要 open(path)、read_excel(path)、to_csv(path) 或直接存檔。
- inputs 每項必填 name、專案相對path、kind，可選revision及encoding；不要漏kind或產生其他欄名。kind=dataset讀原工具生成的追蹤CSV，tables[name]為保留型別的DataFrame；metadata[name]保留原始欄名、Excel格式／日期系統與逐列來源。kind=csv讀一般UTF-8 CSV，欄位先全部為文字，保留前導零及NA字樣。kind=text讀LOG/TXT/JSON/JSONL/OUT/ERR，內容在texts[name]。kind=xlsx只供_AI_Output內程式生成的XLSX，tables['name/工作表名']取得各表。
- LOG／OUT／ERR 使用encoding=auto（預設）：程式先嚴格嘗試Big5（Windows CP950），失敗再嘗試UTF-8；UTF-8 BOM優先遵循，損壞不退回Big5。TXT／JSON／JSONL預設UTF-8，可明確指定big5。資料交到Python時已是Unicode，不要再次decode，不要因LOG不是UTF-8就放棄Python。metadata[name]['encoding']提供實際編碼；encoding_ambiguous=true表示Big5與UTF-8都能解碼但結果不同，務必先核對少量中文字／已閱讀样本，必要時以encoding='utf8'重讀。兩種都無法可靠解碼才核對來源或匯入文字，不用errors='ignore'／'replace'。
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

多份LOG先核對編碼與格式（參數中的name才是Python索引；path僅用於原生讀取）：
```json
{"purpose":"核對兩天LOG編碼與格式","inputs":[{"name":"day1","path":"LOG/day1.log","kind":"text","revision":null,"encoding":"auto"},{"name":"day2","path":"LOG/day2.log","kind":"text","revision":null,"encoding":"auto"}],"code":"result = {name: {'lines': len(texts[name].splitlines()), 'encoding': metadata[name]['encoding'], 'ambiguous': metadata[name]['encoding_ambiguous'], 'first_line': texts[name].splitlines()[:1]} for name in ('day1', 'day2')}","progress_note":"核對兩份LOG格式後進行完整分析"}
```
不要寫texts['LOG/day1.log']。Schema錯誤會指出如inputs[2].kind（第3項）缺漏；先修正參數，再處理Python回傳的SyntaxError或KeyError。不要抄回錯誤或混入非Python符號，不把executed=false當成已分析。
