# 0.8.10 結構化回覆修正與發行驗收

修正桌面完成回答只取 choices 正文、漏掉結構化重點等欄位的問題。依使用者提供的 JSON 優先讀取 response_payload_json／直接 payload，背景與串流完成共用同一解析；正文及重點直接顯示，來源、信心、限制與引用可展開，複製及加密歷史保留完整內容。無有效 payload 時使用中英文編號章節備援，不將 Sources 後的所有內容一起收合。

版本由 0.8.9 升至 0.8.10，Cargo.lock 僅更新本專案版本，無依賴異動。沿用三花貓圖示、固定安裝位置 `C:\largan\LM_AI`、既有 VNC 設定保留，以及單純安裝／不檢查或附帶 WebView2／一般安裝不自動啟動的流程。

## 完整建置與測試

2026-09-23 執行 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`，本次發行建置完整通過：

- `vcvars64.bat -vcvars_ver=14.2` 初始化 MSVC 14.29.30133；實際 C 探針 `_MSC_FULL_VER=192930159`、x64，linker 為同版 Hostx64/x64。
- Rust 1.98.1、Windows SDK 10.0.19041.0，fmt、Clippy、85 項 Rust 測試全部通過。
- 空 Cargo 快取、Cargo.lock／vendor／.cargo/config.toml 的 `cargo build --workspace --release --frozen`，依賴離線提供。
- WebView2 DOM 自檢包含 Rust 解析使用者範例後經正式訊息橋傳入、背景／串流完成切換、兩項重點可見、信心顯示 `High (高)` 而非外層 `medium`、完整複製、Markdown 安全處理及舊格式 21 個標籤。另以 Rust 測試驗證 DPAPI 歷史保存／重新讀取、部分回答取代與去重。細節見 [回覆章節驗收](VALIDATION_REPLY_SECTIONS.md)。
- EXE／NSIS 兩份更新清單由既有金鑰簽署，再以主程式內建公鑰和正式更新驗證流程檢查版本、平台、大小、SHA256 與簽章。
- NSIS 實際安裝、固定目錄及捷徑／登錄、更新 PID 交接與重啟、檔案鎖定失敗時保留舊 EXE、未知檔案與 VNC 設定保留、解除安裝，以及正式 Setup 安裝後 WebView2 自檢全部通過。
- 只對測試子程序指定不存在的 Runtime 位置：Setup 可完成，主程式啟動才提供既有補裝提示；未移除本機 Runtime。

記錄位於 `offline/exe-verification.json`、`offline/installer-verification.json`、`offline/environment.txt`；完整本機日誌為 `.build/release-0.8.10.log`。

## 產物

| 檔案 | 大小（bytes） | SHA256 |
|---|---:|---|
| dist/LM_AI.exe | 4,844,032 | `01c70ccbf2de86f3fcfcabade9b5516b1a084fd596d25c0d3ff144913e969d2c` |
| dist/LM_AI_Setup.exe | 1,955,866 | `45da8ba3292b9af179319f4b51eead04df4ab21bd4c1bcb741518553a30bd90d` |

EXE／Setup 檔案版本均為 0.8.10.0；`dist/update-manifest-exe.json` 及 `dist/update-manifest.json` 的 version 均為 0.8.10，各清單須與對應二進位檔成對部署。Git LFS 保存 EXE／Setup，原始碼及本次驗證文件一起交付。

網站可公告 latest_version 0.8.10，本次不需提高 minimum_version。同版仍優先 EXE；若要 NSIS 自動安裝，網站所有探索來源應只公告新版 NSIS 清單。Git 推送不等同內網網站的檔案與版本公告已更新。

## 驗收界線

沒有連線公司快速模型、取得實際 SSE 封包或拍攝畫面；本機結果解析與 DOM 測試不代替公司實機驗收。VNC Viewer 連線沿用先前驗收，本次只重測安裝／更新時的設定保留。未重新驗收公司防毒、Outlook 或 Windows 圖示快取。

歷史離線 ZIP 與其 verification.json 未重製，不宣稱是 0.8.10 完整離線交付包。這次在現有專案的 vendor 與空 Cargo 快取完成離線建置驗證。舊歷史只有正文的訊息不自動重新查詢，新的完成結果會保存完整結構。
