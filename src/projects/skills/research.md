# 文件搜尋與分段閱讀

search_files 對明確指定的文件搜尋原文；list_document_sections 定位區段，再 read_document_section 取得原文。先 read_file 取得文件索引與版本。重要數字、引文與矛盾處必須回看原文，摘要不是已核實事實。
品質模型可用 summarize_document(path,focus) 委派快速摘要；快速模型自行閱讀，不能委派。摘要包含來源與區段，但不算主模型已讀。完整讀完文件且 summary_needed 時，載入 notes 保存摘要；不強制每隔幾次操作寫筆記。
