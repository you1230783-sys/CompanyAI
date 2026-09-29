# 0.8.21 專案 PDF→Markdown 契約

## 路由與傳输

正式專案任務的 PDF 閱讀、搜尋、建立 TXT 副本統一使用新伺服器路由，不先嘗試本機 PDF 解析。手動匯入快照仍優先。原有原生解析器保留供無登入的本機診斷測試，不在伺服器錯誤後默默備援。

- POST `webBaseUrl/api/desktop/documents/pdf-to-markdown`。以實際 desktop OAuth token endpoint 的同層推導部署前綴；公司最終網址 `http://lp2-en-server/lm_server/api/desktop/documents/pdf-to-markdown`，不遺失子路徑。
- `Authorization: Bearer <desktop_token>`、`X-Client-Version: 0.8.21`、`Accept: text/markdown`。驗證 Session 與目前 Config 綁定及到期；Token 不交模型或子程序。
- multipart/form-data，只有 `file`，原始檔名 .pdf；不送 JSON／extract_images。原生 WinHTTP 沒有 FormData，由固定 builder 以隨機 boundary 同時產生本文與 Content-Type，逐塊傳輸，不把 50 MiB 一次載入記憶體。
- 輸入 1 byte 至 **52,428,800 bytes**；不是 50,000,000。保留 broker 專案邊界、連結拒絕與原檔唯讀鎖，雜湊及上傳期間固定同一個 handle。
- 不自動重新導向，不傳 Cookie／Windows 自動認證，不略過 TLS 驗證。公司仍使用既有內網 HTTP。

## 結果與取消

成功必須是 HTTP 200、`text/markdown`、嚴格 UTF-8（可含 BOM）、非空且不超過 200 KB。錯誤 MIME、亂碼、空內容或過大都明確失敗，不把 HTML 登入頁／JSON 當文件，不靜默截斷。`Content-Disposition` 不作磁碟路徑：此版只需要任務記憶體快取，沒有依伺服器名稱寫入 .md。

非 200 依統一 JSON 保留 error_code／message／request_id，Header X-Request-ID 作錯誤 ID 備援；移除控制字元、限制長度並遮蔽 Token。非 JSON 錯誤仍保留 HTTP 狀態，不展示整頁 HTML。圖片由 Server 固定 extract_images=false；模型不得自行推測圖片或原頁碼。

HTTP wire 使用非同步 WinHTTP，背景函式等待 callback；接收等待 180 秒、上傳等待 120 秒，整次上傳加轉換 300 秒。取消或總時限到達會關閉本次非同步 request，等 HANDLE_CLOSING 才釋放 callback／I/O buffer。沒有 job ID、不輪詢、不自動重送；關閉桌面連線不保證 A／C 端工作已取消。

## 快取與工具

每個 Broker 任務自有 Reader，按原始 PDF SHA256 保存成功的 Markdown 字串；每次操作重新驗證檔案與雜湊。相同內容的 read_file 分段、find_text、create_working_copy 共用結果；內容改變重新轉換。最多 20 份、約 4 MB，超過清除快取；結束任務即釋放。不跨帳號／專案／任務共用，不另存明文 MD；既有訊息與操作記錄仍以 DPAPI 保存。

read_file 仍回傳最多 6000 Unicode 字元與 next_offset／total／revision，全文上限 200 KB。建立副本為 TXT、UTF-8 BOM，仍用既有受限文字編輯與另存流程；不覆寫來源 PDF、不產生修訂後 PDF。

## 驗收界線

本機 loopback 測試驗證契約、真實 WinHTTP、取消與 broker 全流程；不冒充公司 A／C 服務或加密政策測試。使用者已確認 MSG 及一般附件 PDF 轉檔成功；新的原生專案上傳仍需同一份公司 PDF 實測。WebView 選檔與原生檔案讀取是否取得相同解密位元組，取決於公司加密政策；不預先宣稱保證。

版本 0.8.21，v142 x64、空 Cargo 快取 --frozen，發布 EXE 及其簽署清單，不重製 NSIS／歷史 ZIP。
