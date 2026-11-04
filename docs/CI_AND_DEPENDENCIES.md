# Windows CI 與依賴檢查

0.9.7 新增 `.github/workflows/windows.yml`。CI 使用 Windows Server 2022 runner，明確準備 MSVC v142 14.29 x64 與 SDK 10.0.19041.0；不能用宿主 Visual Studio 的預設工具集代替。Rust 版本依 `rust-toolchain.toml` 固定，Actions 也固定至官方提交 SHA。

## 準備與驗證分開

`Initialize-CI.ps1` 只允許在 GitHub Actions 執行。下載工具鏈與 `cargo vendor --locked --versioned-dirs vendor` 需要網路，不能稱為完全離線取得輸入。準備不改 Cargo.lock 或 `.cargo/config.toml`。

`Test-CI.ps1 -HostedRunner` 之後以空 Cargo 快取執行 `Build.ps1 -BuildOnly -EmptyCargoCache`，確認真正的 cl 編譯器為 `_MSC_VER` 1920–1929、x64，使用指定 linker，再以 `--frozen` 建置。另執行 fmt、完整／核心配置的 all-targets Clippy（警告視為錯誤）與 workspace Rust 測試。

Hosted runner 沒有正常互動 Explorer 桌面，明確排除以下兩項通知測試，並在 JSON 留名；不能把 CI 當成完整發行驗收：

- `ui::reminders::tests::windows_shell_accepts_native_reminder_notification`
- `pet::tests::native_notification_visibility_and_settings_survive_restart`

本機 `Build.ps1 -EmptyCargoCache -IncludeInstaller -TestOffice` 仍執行全部原有測試，以及 Office、WebView2、Python／CAD runtime、NSIS 生命週期。CI 不部署公司伺服器，不讀取私人設定，不持有更新簽章私鑰，也不發布安裝包。

## 漏洞與維護公告

`Test-DependencyAudit.ps1` 使用 RustSec 官方 cargo-audit 0.22.2 Windows 預編譯工具，ZIP SHA256 固定為 `0a7316540862c13d954f648917ceacca593747baed6eec180fafa590be2710ab`。這是開發工具，不宣稱它由本專案以 v142 編譯；應用建置規則不變。工具只保存在 `.build`，不加入 Git 或 Release。

每次檢查刷新官方 RustSec 資料庫，記錄修訂、日期、Cargo.lock 雜湊及完整 JSON。安全漏洞、unsound、yanked 及未審查的警告都失敗；不限制只看 Windows，也不以 `--ignore` 隱藏公告。每週、main push、PR 與手動觸發皆執行。

每週觸發只刷新依賴掃描，不重做沒有原始碼變更的編譯；push、PR及手動觸發才執行建置與Rust回歸。

首次檢查找到 `yoke-derive 0.8.3` 已撤回，0.9.7 單獨更新至 0.8.4，其他套件不批次升級。

唯一有期限的維護例外是 [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html)：`ttf-parser 0.25.1` 停止維護，沒有修正版。相依鏈為 `pdf-extract → lopdf → ttf-parser`；目前鎖定的 PDF 擷取上游仍依賴它。此公告是維護狀態，並非已公告的安全漏洞，但解析使用者 PDF 的長期風險仍存在。現有解析在 AppContainer 執行，另有輸入、頁數、輸出、時間及記憶體限制；這些限制不等於保證沒有漏洞。

暫緩整組 PDF 引擎替換，避免把大幅解析行為變更混入 CI 建置版。例外只匹配該 ID、套件、版本與 `unmaintained` 種類，必須在 **2027-02-01 前重新評估**；到期自動失敗。新漏洞或其他公告不能沿用例外。完整警告仍保留，摘要標為 `PASS_WITH_REVIEWED_MAINTENANCE_NOTICE`，不宣稱零警告。

## 產物與官方參考

Actions 只上傳 `.build/ci/reports/*.json`，保存七天；不含 EXE、vendor、target、離線 ZIP 或私有紀錄。GitHub Release 維持既有五項正式交付資產，公司 HTTP／同源／簽章更新不變。

- [Cargo vendor 與 locked／frozen](https://doc.rust-lang.org/cargo/commands/cargo-vendor.html)
- [Microsoft 工具集元件清單](https://learn.microsoft.com/en-us/visualstudio/install/workload-component-id-vs-build-tools?view=visualstudio)
- [Visual Studio 安裝程式參數](https://learn.microsoft.com/en-us/visualstudio/install/use-command-line-parameters-to-install-visual-studio?view=visualstudio)
- [RustSec cargo-audit 0.22.2](https://github.com/rustsec/rustsec/releases/tag/cargo-audit/v0.22.2)
