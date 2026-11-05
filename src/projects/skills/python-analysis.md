# Python 分析入口

run_python在本機無網路AppContainer分析；原件唯讀，不可open、連網、啟動程序或執行來源程式。不需先讀所有參考頁。
讀原文、確認格式用read_file，再record_reading_notes；run_python用於計算，kind=text的衍生輸出不是閱讀來源，不拿Python操作ID記閱讀筆記。
inputs填name/path/kind；revision用read_file.source_revision或成功Python的sources.revision。read_file.revision是解碼文字雜湊，BOM等差異不能混用；版本不符先重新核對，不改null繞過。dataset沿用inspect_dataset版本。
csv/dataset在tables[name]；text在texts[name]；生成xlsx在tables['name/工作表']；來源資訊在metadata[name]。CSV初始欄為文字，明確轉數字；原始Office先用excel-read經COM匯出CSV。
已附np/pd/openpyxl/io/scipy/plt及標準re/json等，不需探測或pip安裝。from scipy.optimize import nnls可用；核對方法假設、收斂與殘差，不臨時自寫未驗證求解器。
每次新程序，120秒／1GiB；text最多32份，其他8份；單檔32MiB，序列化合計60MiB。result可放巢狀JSON（12000字元），print最多8000字元。
成果例：emit_table('統計.csv',df)、emit_excel('分析.xlsx',{'資料':df})、emit_figure('圖.png',fig,dpi=150)。缺副檔名自動補齊；錯副檔名改檔名，不改kind或result結構。一次計算可交付8份成果，勿為拆表重算或經print再轉抄。成果自動交付，finish.artifacts不填路徑。
CSV上限10萬列／16欄；XLSX12表、每表10萬列／32欄、8MiB；PNG4096×4096、8MiB、50–300 DPI。plt.subplots()後emit_figure，不用savefig寫檔；預設繁體中文字型。亦可CSV→dataset-charts→export_chart_png。
資料驗證與最終計算都把報告會引用的筆數、欄數、頻率上下限、取樣密度、τ網格／步距等放result具名數值，以register_report_values的/summary/...及單位登記；每輪保留並原生附表。Word/Excel用apply_report_values，不憑記憶補值。數字一致不代表方法正確。保留來源列號與輸入／成功／排除／失敗／未配對筆數；缺值不補零、批號不轉數字，不默默抽樣截斷。
按需read_skill_guide：python-tables（合併、表格與科學圖範例）、python-logs（編碼與LOG分段）、python-source（程式碼唯讀分析）。未知副檔名可嘗試文字閱讀／kind=text；二進位或編碼不明須回報。修改走一般文字副本（200KB），原件不動；list_files依next_offset續頁。
