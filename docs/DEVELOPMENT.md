# 開發與維護說明

[專案首頁](../README.md) · [使用指南](USER_GUIDE.md) · [版本更新](../CHANGELOG.md) · [技術文件索引](README.md)

本文件說明目前專案的環境、編譯與維護方式。開始修改前請閱讀 [AGENTS.md](../AGENTS.md)。

## 專案與環境

CompanyAI 是獨立 Cargo workspace，開發環境中的位置為 `Y:\Rust\Project\CompanyAI`；`Y:\Rust` 是多個專案的共用根目錄，不是本專案 workspace。

| 項目 | 基準 |
| --- | --- |
| 執行平台 | Windows 11 x64 |
| Rust／Cargo | 1.98.1，由 rust-toolchain.toml 固定 |
| 目標 | x86_64-pc-windows-msvc，靜態 CRT |
| C/C++ 編譯器與 linker | MSVC v142，已驗證 14.29.30133／_MSC_VER 1929 |
| Windows SDK | 10.0.19041.0 |
| 前端 | WebView2、內嵌 HTML／CSS／JavaScript 與離線資源，不需 npm 或 CDN |

Visual Studio 可用作工具集安裝宿主，但不得直接換成其預設的新工具集。`Enter-DevShell.ps1` 透過 `vcvars64.bat -vcvars_ver=14.2` 選用 v142；Build 另外用實際編譯器核對版本與 x64。公司完整 MSVC 修補版本及 SDK 仍需現場核對。

## 編譯與檢查

以下指令均在 CompanyAI 專案根目錄執行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Build.ps1 -EmptyCargoCache
```

腳本依序執行格式、Clippy、工作區測試、release 編譯、WebView2 自檢及真正 AppContainer／文件工具整合，再更新 `dist/LM_AI.exe`。機器驗證紀錄在 `offline/exe-verification.json` 與 `offline/environment.txt`。

依賴由 `Cargo.lock`、`vendor` 與 `.cargo/config.toml` 固定，使用空 Cargo 快取及 `--frozen` 驗證。環境腳本優先使用專案內的 `toolchain`，否則使用共用 `.tools`，並設定本專案的 target 目錄。

`toolchain/` 與 `vendor/` 不直接提交 Git。**現存離線 ZIP 是歷史交付包，不代表目前 0.8.25 原始碼。** 全新環境需先準備指定工具鏈與符合 Cargo.lock 的 vendor；不要將舊 ZIP 整包覆蓋到新版原始碼。完整離線包操作見 [離線交付與 Git](OFFLINE_AND_GIT.md)。

| Build 選項 | 用途 |
| --- | --- |
| `-EmptyCargoCache` | 建立新的空 Cargo 快取，核對離線依賴是否齊全。 |
| `-ValidateOnly` | 執行驗證，保留 target 產物，不更新 dist 與發行清單。 |
| `-TestOffice` | 加跑已安裝桌面 Office 的整合測試。 |
| `-TestVnc` | 加跑已安裝 UltraVNC Viewer 的整合測試。 |
| `-IncludeInstaller` | 明確要求時才建立並驗證 NSIS 安裝包。 |

本機測試通過不能代替公司模型、加密系統、網路及實際文件的驗收。每次發行的已測／未測範圍見對應的 `VALIDATION_*.md`。

## VS Code

以「開啟資料夾」開啟 CompanyAI，安裝 `rust-lang.rust-analyzer`。第一次使用或搬移專案後執行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Configure-VSCode.ps1
```

在命令面板執行 `Developer: Reload Window`，並重開終端機。腳本為 rust-analyzer 與終端機設定 Rust／MSVC 環境；編輯器的檢查及跳轉需要 Cargo、rustc 與 rust-src，已編譯 EXE 則不需要。

`.vscode/settings.json` 含本機路徑，不提交 Git 或納入 ZIP。設定檔若含 PowerShell 5.1 不支援的 JSON 註解，腳本會停止並保留原檔，需先備份再合併。

## 本機示範與測試入口

```powershell
.\dist\LM_AI.exe --demo
```

在示範介面按「登入」，於本機頁面允許授權。可測附件狀態、串流、背景任務與通知；使用模擬轉檔和虛構郵件，狀態只存記憶體，不讀真實信箱或連接公司服務。

開發時先載入環境，再執行 Cargo：

```powershell
. .\scripts\Enter-DevShell.ps1
cargo run --frozen -- --demo
```

需要人工瀏覽器登入整合時：

```powershell
cargo run --example browser_smoke --frozen
```

開啟輸出的 loopback 網址並允許授權；範例驗證 Token、DPAPI、中文回覆與登入碼不可重複使用。`examples/project_smoke.rs` 的文件／HTTP／AppContainer 驗證已由 Build 腳本呼叫。

## 發行檔案與版本

| 檔案 | 目前狀態 |
| --- | --- |
| `dist/LM_AI.exe`、`dist/update-manifest-exe.json` | 0.8.25，EXE 與簽署清單須成對發布。 |
| `dist/LM_AI_Setup.exe`、`dist/update-manifest.json` | 0.8.15，尚未重製為最新版本。 |
| `offline/CompanyAI-offline.zip` 及其校驗／清單 | 歷史交付包，版本與目前原始碼分開確認。 |

一般 EXE 發行使用 Build；完整離線包另用 `scripts/Prepare-Delivery.ps1` 整理 vendor、封裝並解壓驗證。只有要求整包交付時才更新 ZIP，不能用舊包驗收新版。需要更新依賴時使用其 `-RefreshDependencies` 流程，詳見 [離線交付與 Git](OFFLINE_AND_GIT.md)。

EXE／ZIP 由 Git LFS 管理。更新簽署私鑰位於 `.private/`，不提交或交付；版本清單由內建公鑰驗證。僅編譯 EXE 時不修改 NSIS 版本與清單。

版本更新請同步維護 Cargo.toml／Cargo.lock、[首頁](../README.md)、[更新紀錄](../CHANGELOG.md)、目前使用指南，以及對應契約與驗證紀錄。純文件整理不改應用程式版本或重製二進位檔。

## 程式碼導覽

| 位置 | 責任 |
| --- | --- |
| `src/main.rs`、`src/lib.rs` | 程式入口、模組與示範／自檢模式。 |
| `src/ui.rs`、`src/ui/` | 原生狀態、UI 命令與背景工作協調。 |
| `src/webview.rs`、`ui/`、`build.rs` | WebView2 與嵌入介面資源。 |
| `src/projects.rs`、`src/projects/` | 專案、工具契約、文件副本、記憶、PDF 與受限執行器。 |
| `src/projects/skill.md`、`src/projects/tools.json` | 專案系統提示詞與工具參數定義。 |
| `src/jobs.rs`、`src/attachments.rs` | 持久任務、輪詢與附件流程。 |
| `src/outlook.rs`、`src/outlook/` | Classic Outlook 預覽、查詢與受控 MSG 匯出。 |
| `src/vnc.rs`、`src/vnc/` | 機台設定、Viewer 啟動與網站清單同步。 |
| `src/auth.rs`、`src/config.rs`、`src/service.rs` | 登入、固定路由、偏好、版本與模型。 |
| `src/protocol.rs`、`src/protocol/`、`src/transport.rs` | API 訊息、回答解析與 WinHTTP。 |
| `src/selection.rs`、`src/selection_popup.rs`、`src/hotkey.rs` | 選字、浮動入口與快捷鍵。 |
| `src/storage.rs`、`src/history.rs` | 本機保存、DPAPI 與對話歷史。 |
| `src/demo.rs`、`examples/` | 本機模擬服務與整合驗證。 |

圖示原圖、多尺寸 ICO 與重建方式見 [assets 說明](../assets/README.md)。

## 資料、服務與診斷

一般本機資料位於 `%LOCALAPPDATA%\CompanyAI`。偏好為 settings.json；Token、對話與通知等私有紀錄以目前 Windows 使用者的 DPAPI 保存。專案記憶位於專案根目錄 `.lmai`，逐輪請求紀錄位於應用資料的 `project-runs`。VNC 相容設定則放在 EXE 同一資料夾，詳見 [VNC 說明](VNC_QUICK_CONNECT.md)。

匯入文字是加密快照，不隨原件自動更新；匯入文字衍生的 TXT 副本使用 UTF-8 BOM。未送出的郵件預覽只保留在記憶體，登出不會刪除伺服器資料或等同撤銷伺服器 Token。

公司服務主機與路由固定於程式，不提供 UI 修改；目前使用內網 HTTP，DPAPI 的本機加密不等於傳輸 TLS。版本、模型與登入檢查由共用服務流程處理；已確認的最低版本門檻跨重啟保存，不因斷線解除。

一般聊天與專案皆使用現有後端契約；0.8.25 工具定義只放在 system 文字內，並非原生 API tools 欄位。串接、提示詞與驗證入口見 [技術文件索引](README.md)。
