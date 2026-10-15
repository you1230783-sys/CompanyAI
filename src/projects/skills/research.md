# 文件搜尋與分段閱讀

## 程式庫唯讀搜尋

C/C++、H、Python、Rust、JS/TS、HTML/CSS等程式碼可直接find_text或search_files，不要用inspect_excel。先list_files逐層取得明確檔名；find_text單檔最多100處命中，search_files全為程式碼時每批最多100檔、混合一般文件最多20檔，結果最多60處命中。搜尋為區分大小寫的字面文字，不接受資料夾、glob或正規表示式。coverage/errors/truncated都要檢查；不能把未掃描或失敗當零命中。

命中後read_code_section按行讀取（最多200行／6000字），或read_file按字元分頁；這些程式檔不建立一般文件區段索引，不用list_document_sections／read_document_section或全文摘要工具反覆嘗試。一般閱讀／搜尋程式單檔4MiB。

需要正規表示式、跨檔比較、符號出現次數或較大程式檔時，載入python-analysis，用run_python的kind=text把來源交到texts[name]做靜態文字分析；全程式碼最多32來源、單檔32MiB、序列化合計60MiB，其他混合來源仍最多8份。不執行或import來源程式、不自行開檔。先列樣本核對編碼，輸出精簡結論及來源／行號，不回傳整個專案原文。超過批次／大小限制要分批或明示未掃描，不原樣重試。

search_files 對明確指定的文件搜尋原文；list_document_sections 定位區段，再 read_document_section 取得原文。先 read_file 取得文件索引與版本。重要數字、引文與矛盾處必須回看原文，摘要不是已核實事實。
品質模型可用 summarize_document(path,focus) 委派快速摘要；快速模型自行閱讀，不能委派。摘要包含來源與區段，但不算主模型已讀。完整讀完文件且 summary_needed 時，載入 notes 保存摘要；不強制每隔幾次操作寫筆記。

專案圖片亦可作為來源。清單的 kind=image 表示 JPG／JPEG／PNG，依任務需要再以 analyze_image 讀取相關圖片；不必載入額外圖片技能。文字搜尋與文件摘要工具不讀圖，不把圖片檔名當內容，也不自動辨識不相關圖片。PDF／Office／MSG 內嵌圖片不在此範圍。
