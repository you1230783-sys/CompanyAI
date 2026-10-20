# Python LOG 分段分析

LOG／OUT／ERR預設先Big5再UTF-8，UTF-8 BOM優先；metadata.encoding_ambiguous=true先核對樣本。可指定utf8/big5，不用ignore/replace掩蓋錯誤。Unicode資料不再次decode。

LOG用texts['log'].splitlines()及enumerate(...,1)保留行號，以re解析已確認的事件／批號，記錄無法解析行數；計算開始／完成配對時保留取消與缺失事件。跨午夜、校時與不同批號不可憑順序猜配對。

超過32MiB的LOG可在同一個kind=text輸入指定log_range:{start_line:1,line_count:50000}，每次實際最多2MiB完整行，來源上限1GiB。texts[name]只含本段；原始行號使用enumerate(texts[name].splitlines(),metadata[name]['start_line'])。工具sources回傳revision、next_line、eof，後續inputs帶同revision及next_line，直到eof；不能把第一段當全文。每次Python仍是新程序，將跨段待配對事件及累積統計emit_table保存，下一次用kind=dataset讀回；對重疊範圍去重，不重新加總。編碼有歧義先核對中文字樣本，可明確指定encoding=utf8或big5；UTF-16依BOM自動解碼。
每段result保留input、parsed、excluded、unparsed等互斥筆數及unmatched／duplicates／counterexamples。階段性用record_analysis的checks核對，例如 {"label":"解析筆數","operation_id":"實際Python操作ID","kind":"balance","total_pointer":"/summary/input","part_pointers":["/summary/parsed","/summary/excluded","/summary/unparsed"]}。反例檢查可用kind=zero、total_pointer="/summary/counterexamples"、part_pointers=[]；非零時解釋及修正假設。交付前引用原文與計算結果，並按需要把方法存入專案記憶。

多份LOG先核對編碼與格式（參數中的name才是Python索引；path僅用於原生讀取）：
```json
{"purpose":"核對兩天LOG編碼與格式","inputs":[{"name":"day1","path":"LOG/day1.log","kind":"text","revision":null,"encoding":"auto"},{"name":"day2","path":"LOG/day2.log","kind":"text","revision":null,"encoding":"auto"}],"code":"result = {name: {'lines': len(texts[name].splitlines()), 'encoding': metadata[name]['encoding'], 'ambiguous': metadata[name]['encoding_ambiguous'], 'first_line': texts[name].splitlines()[:1]} for name in ('day1', 'day2')}","progress_note":"核對兩份LOG格式後進行完整分析"}
```
不要寫texts['LOG/day1.log']。Schema錯誤會指出如inputs[2].kind（第3項）缺漏；先修正參數，再處理Python回傳的SyntaxError或KeyError。不要抄回錯誤或混入非Python符號，不把executed=false當成已分析。
