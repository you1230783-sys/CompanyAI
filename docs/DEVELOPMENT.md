# 開發與維護說明

## 0.8.43 維護位置

- 圖表轉換：`src/projects/charts/transform.rs` 與 `ui/chart-transform.js` 共用契約，先篩除全系列缺值列、再依共同序號轉換。`ui/chart-editor.js` 提供兩軸設定，`ui/charts.js` 共用畫面／PNG呈現。
- 異常值示意：`ui/chart-review-examples.js`；固定假設五點，不使用真實來源數據。
- Python編碼：`src/projects/python/encoding.rs`；只在原生快照階段處理，worker與runtime清單不變。Schema錯誤定位在 `src/projects/agent/schema.rs`，只診斷、不猜參數。
- 安裝沿用：`installer/LM_AI.nsi`；`scripts/Test-Installer.ps1` 以唯讀檔案鎖、建立時間及三種損壞情境驗證。正式發行仍跑下方完整NSIS流程，詳見 [本版驗證](VALIDATION_0_8_43.md)。

## 0.8.42 Python 環境

Python 輸入包、固定依賴、AppContainer 資料橋接及離線重建見 [PYTHON_RUNTIME.md](PYTHON_RUNTIME.md)。本版必須完整 NSIS 發行：`Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot <測試SMB根目錄> -IncludeInstaller`。不再以獨立 EXE 更新清單發布本版。

## 0.8.39 維護位置

- 專案快速入口：`ui/project-quick.js`、`src/ui/projects/quick.rs`；原生層核對對話、專案、帳號、模型及歷史，準備視窗不啟動任務。
- 圖片輸入：`src/projects/vision/input.rs`，既有路徑保護、有限讀取、PNG CRC／JPEG 容器尺寸、Base64 與 SHA256；不新增影像解碼套件。
- 協調與恢復：`src/projects/vision.rs`，獨立模型請求、20 次額度、DPAPI 待查／快取；`agent.rs::attach_image` 只轉換圖片子請求，不更動一般 Message 型別。
- 測試：`examples/project_smoke/vision.rs` 的五種真實本機 HTTP 案例；可用 project_smoke 的 `--vision-only` 快速驗證。正式 Build.ps1 仍包含全部測試、原生 UI、Office 及網路專案回歸；詳見 [驗證紀錄](VALIDATION_0_8_39.md)。


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

`toolchain/` 與 `vendor/` 不直接提交 Git。**現存離線 ZIP 是歷史交付包，不代表目前 0.8.43 原始碼。** 全新環境需先準備指定工具鏈與符合 Cargo.lock 的 vendor；不要將舊 ZIP 整包覆蓋到新版原始碼。完整離線包操作見 [離線交付與 Git](OFFLINE_AND_GIT.md)。

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
| `dist/LM_AI.exe`、`dist/update-manifest-exe.json` | 0.8.27，EXE 與簽署清單須成對發布。 |
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

一般聊天與專案皆使用現有後端契約；0.8.27 工具定義只放在 system 文字內，並非原生 API tools 欄位。串接、提示詞與驗證入口見 [技術文件索引](README.md)。

### 新增專案工作技能

內建技能目錄位於 `src/projects/skills.rs` 與 `src/projects/skills/*.md`，只包含工作方法；工具參數集中在 `tools.json`，操作實作仍由 Rust 決定授權。圖表在 `charts.rs`／`ui/charts.js`，快速模型委派在 `delegation.rs`。修改 Office 或批次邏輯須使用 `-TestOffice`；摘要 HTTP／快取／續接測試在 `examples/project_smoke/skills.rs`。

## CSV 與上下文整理的維護位置

- `src/projects/datasets.rs`：固定來源追蹤 CSV 方言、可逆公式文字保護、有界預覽、型別保留與圖表來源註記；不執行腳本，未加入新依賴。
- `src/projects/logs.rs::dataset`：沿用 LOG 的串流解碼／檔案锁／時間判斷，本地一次掃描所選檔案，不走模型逐頁迴圈。
- `src/projects/files.rs`：受控成果目錄、CSV 讀回／版本驗證、checkpoint 資料集索引、Excel COM 擷取與本地繪圖。
- `src/projects/progress.rs`／`runner.rs`：交接與更正的上下文投影，封存先於精簡，不更動已提交請求；模型摘要不是執行證據。
- 原生 HTTP 新增 case 21，檢查 CSV 後請求縮短、圖表回覆不含座標、用戶更正覆蓋舊候選、按需查回原文。真實 Excel 的 CSV 往返位於 `examples/office_smoke/excel_read.rs`。


## 0.8.37 Outlook 授權與比對

`src/outlook/privacy.rs`管理本機資料夾選擇／DPAPI／祖先鏈檢查；`src/projects/mail/comparison.rs`管理不向AI回傳原文的前文比對；UI由原生run/conversation/request配對接受勾選代號。技能目錄與tools.json同步加入outlook-coverage／outlook_compare，網站通用契約不變。資料夾選擇不是追溯清除聊天或引用文字；限制與驗收見[本版紀錄](VALIDATION_0_8_37.md)。

## 0.8.38 週報及網路專案

`src/projects/setup.rs` 負責 Known Folder、唯一目錄、本機日期／ISO 週次及週報提示；`ui/weekly.js` 管理說明、輸入、確認視窗；`src/ui/projects/weekly.rs` 核對帳號／專案／對話／準備代號後才啟動。使用 `SHGetKnownFolderPath(KF_FLAG_DEFAULT)` 取目前重新導向位置，不拼接使用者 C 槽路徑。網路 IO 沿用 Windows 連線與權限，不能保證斷線時立即回應。

`src/projects/files.rs` 接受磁碟及 UNC 絕對子目錄，保留固定目錄 handle、重解析點、硬連結、`.lmai` 與成果發布檢查。網路伺服器若不支援所需查詢則拒絕操作，不降級略過安全檢查。`examples/network_smoke.rs` 經正式 broker／Office 驗證讀取、成果、DPAPI 筆記與路徑逃逸拒絕。

使用既有 loopback SMB 分享下的專用測試子目錄，例如 `scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build'`。腳本不建立分享；只暫時使用空閒磁碟代號，最後核對並移除自己的映射。`offline/network-verification.json` 分別記錄本機分享通過與公司分享尚未驗證。

無效模型回覆的恢復在 runner 先 `archive_results`，再由 Progress 精簡舊 assistant 歷史及工具結果，保留一組近期結果、原始 user 指示及真實狀態。兩次普通＋兩次恢復，每段累計 30；有進展只重設連續計數，不清除累計。HTTP continuation 測試核對恢復後副本不重複編輯。詳見 [驗證](VALIDATION_0_8_38.md)。
