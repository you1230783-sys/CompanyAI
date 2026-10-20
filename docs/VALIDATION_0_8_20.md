# 0.8.20 驗證紀錄

2026-09-29，本機使用 `scripts/Build.ps1 -EmptyCargoCache -TestOffice`；不重製 NSIS 或歷史 ZIP。完整建置成功，EXE 已通過原生簽署清單驗證。

- 指定 MSVC v142 14.29.30133，實際 `_MSC_FULL_VER=192930159` x64；Rust 1.98.1、SDK 10.0.19041.0。Cargo.lock、vendor、.cargo/config.toml 齊備，未新增依賴。
- 空 Cargo 快取、fmt、Clippy `-D warnings`、122 項單元測試與 `cargo build --release --frozen` 通過。
- 真實 WebView2 自檢、AppContainer 外部檔案／網路拒絕、原件保護、副本編輯／成果讀回與路徑邊界測試通過。
- 四組本機 HTTP 往返通過：純 JSON、說明＋工具不重送、不完整 JSON 有限停止、新增說明＋ask_user 正常詢問且不重試。詢問案例保存說明、只呼叫兩輪、不建立成果目錄。
- 包裝 finish 解析、換行詢問、額外物件拒絕單元測試通過；既有成果驗證維持。
- MSG 暫存測試以真實 Windows 檔案 handle 驗證：原檔不可寫、副本不可被重新命名、副本可讀寫、成功／錯誤清理，以及閱讀器暫留 handle 時回傳結果與提醒。此測試不是實際 Outlook 郵件解析。
- COM EXCEPINFO 延後填入、應用程式說明、內外錯誤碼與資源釋放流程測試通過。
- 一般 PDF 經 AppContainer 擷取、頁碼、空白／損壞拒絕、2 MB 管線傳輸、TXT 另存與原檔不變通過。

## 尚未驗收與已知限制

- **公司加密 PDF 自動讀取未解決。** 使用者確認免費 Acrobat Reader 可正常開啟、Word 開啟是亂碼。本機嘗試 Word PDF 唯讀轉讀在有／無來源鎖兩種條件均未返回，已停止專屬測試程序並移除此備援，沒有放進 EXE。現在提供正確的錯誤說明與 ask_user 匯入流程。
- MSG 真實讀取仍需公司重測；本機 Outlook 尚未完成首次設定，不修改使用者帳號或安全設定。獨立副本改善開檔共享衝突，但公司加密系統是否允許需實測。失敗時請保留新版回傳的完整應用程式說明及錯誤碼。
- COM 仍可能等原生視窗，取消需待呼叫返回。巨集格式測試不含真實 VBA/XLM；未驗證公司加密、第三方加入項與複雜排版，不宣稱逐頁視覺驗收。
- 內網網站未由 Git 推送自動更新。NSIS 與其清單仍為 0.8.15。

## 完整建置與交付

- Office 16.0.20326.20158 的 DOC/DOCX/DOCM、XLS/XLSX/XLSM/XLSB、PPT/PPTX/PPTM 十種格式全部通過實際讀取、編輯、副本另存、重新開啟、原件 bytes 不變與重送冪等檢查。Word／Excel 粗體及未修改段落／公式／文字框核對通過。
- Word 輸出的繁體中文 PDF 經正式 AppContainer 讀取通過；不涉及 Word 轉讀 PDF。
- EXE：8,179,200 bytes，SHA256 `ede597678c08a4cfa6c7172649ba12c75c5615cc2421517c51984cd986d20984`；與 EXE 更新清單及原生驗證一致。
- 本版機器紀錄為 `offline/exe-verification.json`、`offline/office-verification.json`；`offline/document-verification.json` 保留 0.8.19 的歷史專項驗證，不當成本版新增實測。
