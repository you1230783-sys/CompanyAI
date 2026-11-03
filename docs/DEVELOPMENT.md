# 開發與維護說明

[首頁](../README.md) · [文件索引](README.md) · [專案指引](../AGENTS.md)

## 環境與編譯

CompanyAI 位於 `Y:\Rust\Project\CompanyAI`，是獨立 Cargo workspace。共用 `.tools`、`.local-ai` 及外層安裝腳本維持原位。

目標 Windows 11 x64，Rust 版本由 rust-toolchain.toml 固定。必須使用 MSVC v142／14.2x 的 x64 cl 與 linker；已驗證14.29.30133、`_MSC_FULL_VER=192930159`、SDK10.0.19041.0。VS可作安裝宿主，不採用其預設新工具集。

```powershell
. .\scripts\Enter-DevShell.ps1
.\scripts\Build.ps1 -EmptyCargoCache
```

環境腳本以 `vcvars64.bat -vcvars_ver=14.2` 初始化。Build核對實際編譯器，執行fmt、Clippy、Rust測試、release、原生WebView2／代理及相關整合驗證。依賴使用Cargo.lock、vendor、.cargo/config.toml及`--frozen`；空Cargo快取驗證不得靠既有下載快取補件。

| 選項 | 用途 |
| --- | --- |
| `-BuildOnly` | 只編譯本機候選，不覆寫發行成品 |
| `-ValidateOnly` | 執行驗證，不更新dist／發行報告 |
| `-BasicValidation` | 基本驗證，不能標為完整驗收 |
| `-TestOffice` | 已安裝Office的實際讀寫與保存重開 |
| `-TestVnc` | UltraVNC Viewer本機連線 |
| `-TestCadComparison` | 已開啟2008／2022的獨立圖面比較 |
| `-NetworkTestRoot <路徑>` | 既有SMB分享中的獨立測試目錄 |
| `-IncludeInstaller` | NSIS封裝及安裝／更新／修復／移除驗證 |

完整發行依當次授權組合上述選項。缺少CAD、Office或公司環境時明列未測；自動化、真模型與人工外觀分開判定。現行結果見[0.8.77驗證](VALIDATION_0_8_77.md)。

## 程式碼位置

| 功能 | 主要位置 |
| --- | --- |
| 原生介面與WebView2 | `src/ui/`、`src/webview.rs`、`ui/`、`build.rs` |
| 一般聊天、持久任務與恢復 | `src/jobs/`、`src/ui/work.rs`、`src/jobs/retry.rs` |
| 專案執行、工具及技能 | `src/projects/runner.rs`、`files/`、`skills.rs`、`skills/`、`tools.json` |
| 來源筆記與精確補讀 | `src/projects/reading.rs`、`files/batch.rs`、`progress.rs` |
| 成果／流程／數值綁定 | `artifacts.rs`、`workflows.rs`、`report_values.rs`及各自`files/`入口 |
| Office | `src/projects/office.rs`、`office/authoring.rs` |
| CAD | `src/cad_*.py`、`src/projects/files/cad_*.rs`、`tools/build_cad_runtime.py` |
| 圖表／Mermaid與來源導覽 | `src/projects/charts/`、`diagrams.rs`、`diagram_sources.rs`、`ui/charts*`、`ui/diagrams.js` |
| Python與程式副本 | `src/projects/python/`、`files/python_edit.rs`、`files/source_code.rs` |
| Outlook | `src/outlook/`、`src/projects/mail/` |
| 桌寵／塔羅 | `src/pet/`、`src/ui/pet.rs`、`src/tarot.rs`、`ui/pet.js`、`ui/tarot.js` |
| 本機提醒 | `src/reminders.rs`（DPAPI與單次排程）、`src/ui/reminders.rs`（計時與通知）、`ui/reminders.js` |
| CAD舊環境清理 | `src/cad/cleanup.rs`、`assets/cad-runtime-history.json`、`tools/update_cad_runtime_history.py` |
| 互動教學 | `ui/tutorial-lessons.js`、`ui/project-tutorial.js`、`assets/tutorials.json` |
| VNC／設定與帳號 | `src/vnc/`、`src/config.rs`、`src/auth.rs`、`src/storage.rs` |

程式來源以有界區段、行號及revision讀取，不執行使用者來源。Office／CAD先核對來源版本與授權；非CAD成果預設副本，CAD目前圖面需原生確認並保留Undo。不要因封存舊文件而放寬這些保護。

## 驗證入口

- `examples/project_smoke/`：正式Broker／AppContainer、HTTP工具往返、來源筆記、恢復與圖片快取。
- `examples/python_smoke.rs`：真正Python、來源橋接、隔離、取消與逾時。
- `examples/office_smoke.rs`及`office_authoring.rs`：實際Office、任意合法公式、Formula2、錯誤批次及保存重開。
- `examples/delivery_smoke.rs`：計算值寫入Word／Excel／圖表、來源失效、流程重跑與成果交付。
- `examples/cad_comparison_smoke.rs`、`tools/verify_cad_block_circles.py`：雙版原生比較、普通圓圖塊、Undo及存檔重開。
- `scripts/Test-Settings.cjs`、`Test-PetTarot.cjs`及其他`Test-*.cjs`：真瀏覽器介面回歸；與公司實際通知／拖曳驗收分開。
- `scripts/Sync-TutorialCatalog.js`：同步教學索引與覆蓋表，不使用正式模型或工具。

真模型依[模型驗證流程](LOCAL_MODEL_VALIDATION.md)保存原始參數、錯誤與獨立成果核對，不由測試轉接器補正模型的答案。

## 維護與發行

本機私有資料、測試對話與憑證不進Git。更新私鑰在`.private/`；版本與簽署清單必須對應實際成品。純文件整理不修改Cargo版本或重製二進位檔。

現行成品在dist，最近三版驗證在offline及docs。`python-inputs.zip`、`python-lock.json`、`GstarCAD-Probe.zip`與工具授權仍為離線重建輸入。歷史整包已封存，不能當目前依賴；見[離線交付](OFFLINE_AND_GIT.md)與[封存政策](ARCHIVE_POLICY.md)。

VS Code使用`scripts/Configure-VSCode.ps1`產生本機設定，保留rust-src供程式碼分析，不提交含本機路徑的`.vscode/settings.json`。
