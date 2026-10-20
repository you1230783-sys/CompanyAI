# 0.7.0 驗證範圍

## 自動檢查

- 固定 MSVC v142 x64，透過 vcvars64 -vcvars_ver=14.2 初始化；Build.ps1 實際編譯 _MSC_VER / _M_X64 探針。
- Rust fmt、Clippy、workspace tests、release 建置與 WebView2 自我檢查。
- SSE 測試伺服器先送工具事件，必須收到客戶端回授後才傳 delta／done；用以確認桌面不再等全文才解析首筆事件。
- 用途旗標互斥、串流／背景參數、完成事件別名、JSON 混合文字抽取與越權命令拒絕。
- 舊設定預設值、偏好往返、舊歷史相容、手動標題／置頂／草稿在訊息更新後保留。
- WebView2 測試 Enter 設定、IME 不誤送、Shift+Enter 換行、Ctrl+Enter 送出、置頂排序、1.5 秒模型提示與移除同步模式。
- Setup 內嵌 payload 檢查；完整交付流程在全新解壓目錄，以包內 Rust、空 Cargo 快取與 --frozen 重建。

實際完成的建置與離線驗證結果以 `offline/verification.json`、`offline/environment.txt` 為準，未成功執行前不能據此文件宣稱通過。

## 公司實機仍需驗收

- Adobe Acrobat／Reader 的同一份 PDF：快捷鍵、手動貼上、延遲剪貼簿與受限文件。已修正暫時讀不到 Unicode 時過早回傳，但不能以單元測試代表 Adobe 實機通過。
- 浮動 AI 圖示：Word、Edge、Adobe 各自的 UI Automation TextPattern 支援、多螢幕／縮放、選區保留。圖示預設關閉；不支援選區的程式不顯示。
- 真實 Classic Outlook、公司帳號、MSG 上傳轉檔與新版網頁旗標契約。測試不讀使用者真實郵件。
- 公司已簽章版本的背景下載、退出替換、立即重啟、鎖檔失敗回復。缺少簽章與網站 update 欄位時只驗證拒絕／提示行為，不聲稱正式更新可用。
- Setup 在全新 Windows 使用者下安裝、缺少 WebView2 時安裝 Runtime、解除安裝保留／清除資料，以及公司端點真實網路行為。

這些限制須隨交付說明一併提供，不能把本機模擬結果寫成公司驗收結果。
