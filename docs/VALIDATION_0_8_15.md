# 0.8.15 EXE／NSIS 驗證紀錄

驗證日期：2026-09-24。使用者已授權將前輪暫存的登入與彈窗修改編譯、打包並推送；本輪進版 0.8.15，交付 EXE、NSIS 及兩份簽署更新清單，不重製歷史離線 ZIP。

## 本版行為

- 獨立「登入」按鈕與「尚未登入時無法使用其他功能」提示；登入成功後整區隱藏，設定中移除重複登入入口。登入進行中可取消或重開登入頁。
- 未登入時停用其他控制項，包含 Outlook、VNC、設定及動態新增的按鈕；原生層也拒絕非登入功能命令。保存且仍有效的授權可沿用。
- 登出／授權到期會停用選字快捷鍵、浮動圖示及 VNC 同步，清除前端同步欄位；舊擷取結果不會寫入再次登入後的草稿。
- 設定、確認、改名、VNC 機台管理及必要更新提示五種 HTML dialog，均可點外部關閉。從內容拖到外面、右鍵及取消的指標操作不誤關閉；未來新增 HTML dialog 自動套用。
- 外部關閉確認視窗視為取消；改名與機台編輯不自動儲存。關閉更新提示不解除原生最低版本限制，也不會因一般狀態推播反覆彈出；仍可由設定下載更新。
- 未登入且版本過舊時仍能完成登入，避免登入／更新門檻互相卡住；背景版本檢查不取消正在進行的登入。

## 編譯及自動驗證

執行 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`，使用專案 Cargo.lock、70 個 vendor 目錄及 .cargo/config.toml，以新建空 Cargo 快取執行離線 `--frozen` 流程。

- Rust 1.98.1、MSVC v142 14.29.30133、Windows SDK 10.0.19041.0、x64 靜態 CRT。
- 經 `vcvars64.bat -vcvars_ver=14.2` 初始化，實際 C 編譯探針確認 `_MSC_FULL_VER=192930159` 與 x64，Rust linker 指向同一 v142 工具集。
- rustfmt、Clippy（警告視為錯誤）、108 項 Rust workspace 測試、`cargo build --workspace --release --frozen` 均通過。
- 新增原生權限測試：未登入直接送 Outlook、VNC、偏好、複製、下載命令會被拒絕；登入流程可跨過登入前的版本提示，但功能仍受最低版本門檻限制。
- JavaScript 語法及 `node scripts/Test-UiAuth.cjs` 模擬 DOM 回歸通過，涵蓋登入狀態切換、動態控制項、命令過濾、現有五種及未來新增 dialog。
- 實際 EXE 的 WebView2 DOM 自檢通過，包含登入區塊顯示／隱藏、未登入停用所有其他欄位、取消／重開登入、外部關閉五種彈窗、確認取消，以及更新提示關閉後不重開也不解除版本限制。
- 首次受限環境的 WebView2 啟動回傳 0x8000FFFF；在允許桌面程式執行的環境完整重跑上述流程通過，未略過自檢。

## 安裝包及交付

NSIS 自動驗證通過：新目錄安裝、既有使用者檔案與 VNC 設定保留、固定路徑、捷徑與登錄、等待舊程序退出、更新替換及自動重新啟動、檔案鎖定時保留舊程式、解除安裝保留未知檔案。另用真正交付的 Setup 安裝至 `C:\largan\LM_AI`，核對 EXE 雜湊、執行 WebView2 自檢並解除安裝。

安裝器不附帶或檢查 WebView2；模擬 Runtime 不可用時，安裝仍成功，主程式啟動才顯示既有提示。兩份更新清單均經內建公鑰的原生更新驗證器驗證。

| 檔案 | 版本 | 大小（bytes） | SHA256 |
| --- | --- | ---: | --- |
| dist/LM_AI.exe | 0.8.15.0 | 5,196,800 | `3f4ba1190b085e78fa41b6e4c683aacdad1b0b00f9d27e6fdd3ea50bf4360dbb` |
| dist/LM_AI_Setup.exe | 0.8.15.0 | 2,054,509 | `51dc09125cc717764eadb1643ce687b9e06a5d24696a02b92d31da499243c785` |

機器記錄：[EXE 驗證](../offline/exe-verification.json)、[NSIS 驗證](../offline/installer-verification.json)、[編譯環境](../offline/environment.txt)。

本輪未使用真實公司帳密或連線公司機台，未操作真實 Outlook／VNC Viewer，未擷取外觀截圖；自動測試不能代替公司實機驗收。Windows 原生檔案選擇器與系統訊息框不屬於本次 HTML dialog 外部點擊範圍。Git 推送交付檔案，不代表公司內網下載路由已部署。
