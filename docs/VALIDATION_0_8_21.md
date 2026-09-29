# 0.8.21 驗證紀錄

2026-09-29 執行 `scripts/Build.ps1 -EmptyCargoCache` 成功。只交付 EXE 與簽署清單，不重製 NSIS／ZIP。

- MSVC v142 14.29.30133，實際 `_MSC_FULL_VER=192930159` x64；Rust 1.98.1、SDK 10.0.19041.0。
- Cargo.lock、vendor、.cargo/config.toml 齊備；未增加依賴。以全新空 Cargo 快取執行 fmt、Clippy `-D warnings`、128 項測試、`cargo build --release --frozen` 全部通過。
- 原生 WebView2 自檢與實際 AppContainer 外部檔案／網路拒絕、副本修改／版本核對／成果讀回通過。
- 真正 WinHTTP loopback 核對 POST 路徑含部署子目錄、Bearer、目前 X-Client-Version、單一 multipart file、中文及空白檔名、相符的 boundary、原始 Markdown bytes 與 X-Request-ID。
- 真正串流上傳完整 **52,428,800 bytes**，伺服器逐 byte 核對內容通過；超過上限在連線前拒絕。回覆超過 Markdown 200 KB 上限時停止，沒有截斷後冒稱全文。
- 等待 Header 的轉換可取消；縮短測試總時限可正常中止。兩者均驗證連線關閉且沒有第二次重送；正式設定為 receive 180 秒、整次 300 秒，不以縮短測試冒稱等待過實際 120 秒 Server C。
- HTTP 400／401／403／413／422／502／503／504 的原始錯誤回應保留；302 不跟隨 Location。JSON 錯誤代碼、request_id 與 Token 遮蔽測試通過。
- UTF-8 BOM 與繁體中文成功；非 UTF-8、空 Markdown、HTML、成功狀態 JSON 的拒絕測試通過。
- 正式 broker 接到 loopback 轉換服務：使用不具 PDF header 的測試輸入，確認不先交本機解析器；分段 read_file、find_text、create_working_copy 只上傳一次，改變來源 bytes 後重新上傳。TXT 經 AppContainer 修訂、儲存、讀回通過；原件未被改寫，未產生明文 MD 快取。
- 既有四組模型 HTTP 工具往返，以及原生 PDF 診斷／2 MB 管線傳輸回歸通過。
- EXE：8,236,544 bytes；SHA256 `e2c7919f61f1b56f8166ea25696b9f52e64b6886149bd6bcae70f40733f89113`。更新清單與原生公鑰驗證一致。

機器建置紀錄：`offline/exe-verification.json`、`offline/environment.txt`。Office／MSG 實作未變更，本輪未重跑真實 Office；`offline/office-verification.json` 保留 0.8.20 的歷史驗證，`offline/document-verification.json` 保留 0.8.19。

## 公司實測與限制

使用者已回報 0.8.20 MSG 正常，且一般對話的伺服器 PDF 轉檔可正常閱讀。**本輪尚未連到公司新路由，不宣稱 A／C 服務與原生上傳的加密相容性已驗收。** 請以原先失敗 PDF 測試專案 read_file／摘要／TXT 副本，確認 0.8.21 原生程序的上傳與一般 WebView 附件取得相同可解析內容。

Markdown 上限沿用 200 KB、每次讀取 6000 字，頁碼與版面不可推測；圖片由伺服器停用。取消桌面請求不代表 A／C 端轉換已撤銷。未測新路由真實超過 120 秒等待、公司反向代理或重新登入失效情境。

本輪未做新 UI 外觀截圖，不宣稱視覺驗收；Git 推送不等於公司內網下載路由已更新。NSIS 維持 0.8.15。
