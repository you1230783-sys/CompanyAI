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
