> 2026-10-15：本批已納入0.8.65發行；下文保留設計時紀錄。最新建置與驗證狀態以[0.8.65驗證](VALIDATION_0_8_65.md)為準。

# 待確認：程式庫搜尋限制

使用者提供 search_files 拒絕 CPP 的原始結果，並補充應為 0.8.53。Git 紀錄確認程式碼唯讀於 0.8.54（facddc0）加入，因此此舊版本錯誤與舊白名單一致；目前 find_text／search_files 經 content → readable_extension → read_cancel 共用唯讀白名單，CPP/H 已支援。仍不將未編譯的新功能稱為已驗證。

本批補工具描述、研究與 Python 分析指南及資料夾尾斜線／萬用字元提示。搜尋單檔4MiB，全部程式碼每批最多100檔、混合文件仍20檔，最多60處結果，保持逐檔coverage；加入application_version以利診斷。大量命中時定期檢查取消。擴充C++模組／模板／CUDA／資源檔及Ruby/PHP/Swift/VB/Lua/R/Julia等純文字來源。

run_python(kind=text) 接上同一程式碼白名單及排除規則，來源只送入texts[name]，不開放執行來源或任意檔案存取。純程式碼每批最多32份，其他最多8份；保留單檔32MiB、序列化60MiB與AppContainer限制。預設UTF-8，Big5須明確指定，UTF-16仍報不支援。Python使用raw bytes revision，不與程式分段文字revision混用。inspect_excel仍只接受表格；程式碼使用read_code_section，不走一般文件索引。

未增加遞迴搜尋；本批未執行測試、未進版、未編譯打包或推送，0.8.64 成品不變。下次發行前補各語言唯讀、批次界線、Python快照／編碼／版本、排除路徑及來源未改寫驗證。
