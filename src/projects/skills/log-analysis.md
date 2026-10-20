# LOG 搜尋與閱讀
唯讀LOG/OUT/ERR/JSONL/TXT，每檔最多1GiB。list_logs按檔名日期／分類／站別篩選，每頁100檔；檔名日期不等於事件日期。
read_log回傳revision與一基行號，以next_line/next_column及相同revision續讀。search_logs最多30個明確檔案，terms是任一字面命中，[]不限制文字；時間區間與date填專用欄位，不把約略時間又塞進terms。
有next_cursor必須沿用完全相同query續頁。空matches不表示查完；coverage/errors/unclassified_time_lines與complete決定範圍。excerpt_truncated先回讀全文；before/after只是前後文。
約略時間先前後5分鐘找事件錨點，再回讀20–50行；查完無候選才擴15／30分鐘。有歧義再詢問，不只搜尋ERROR就判定無問題。時間繼承、跨午夜、無日期等需要時讀log-time。
重要事件以record_log_events引用真實operation_id及/lines/N或/matches/N，不自編原文／時間；需要事件配對時讀log-events。大量統計切python-analysis並按需讀python-logs。
回覆附來源、行號、時間及未掃描／失敗範圍；事件先後不是因果證明。不把整份LOG塞入模型。
