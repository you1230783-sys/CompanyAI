# 0.8.54：程式碼唯讀與浩辰 CAD 獨立測試

## 授權與交付範圍

使用者要求先完成基礎驗證、進版、NSIS 打包並推送 Git，完整驗證待公司實測後補做。這份文件不沿用 0.8.53 的完整驗收結論。

- 專案資料夾內 C/C++、標頭、Rust、HTML、JS/TS、CSS 及常見設定檔可按純文字讀取；不執行、不新增修改能力。PY 保留既有副本編輯，原件唯讀。
- 來源單檔最大 4 MiB（原始及解碼後容量皆受限），每段最多 200 行／6000 字；清單最多 200 項／掃描 500 項，未覆蓋目錄須縮小範圍。不可宣稱模型已一次讀完整個專案。跨頁須核對 revision，工具不強制鎖定跨頁快照。
- 排除版本庫、依賴、產物、虛擬環境與常見機密設定；不是任意程式庫的完整祕密偵測器。程式來源不進入文件全文摘要快取，閱讀紀錄仍走既有加密保存。
- 一般聊天附件上傳仍依網站規則，這次沒有新增後端附件格式或改 PDF 判斷。
- 浩辰工具獨立於 CompanyAI 與分析 Python，沒有串接 AI 自動畫圖；Solid Edge 未實作。

## 基礎驗證方法

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller
```

- 使用 v142 14.29.30133 x64，C 編譯器探針 `_MSC_FULL_VER=192930159`，同版 linker；SDK 10.0.19041.0。
- 專案保留 Cargo.lock、vendor、.cargo/config.toml，以本次全新空 Cargo home 執行 `--frozen`；沒有新增 Rust 依賴。
- fmt、Clippy all targets、25 項選定單元測試（20 項聊天、1 項附件、2 項程式來源、2 項編碼）、release。
- 正式 EXE 的 WebView2／原生輸入、任務停止追蹤與 DPAPI 恢復自檢；Outlook helper 管線／EOF 不存取信箱；既有 Python runtime 必要自檢；DLL 核對。
- `project_smoke --source-code-only` 實際原生 worker／broker：8 種程式／設定檔、中文 CRLF、唯讀副本拒絕、超過 200 KB 分頁、來源變更 revision、二進位／機密路徑拒絕、無輸出副本與原件保留。
- NSIS 打包及原生更新器核對清單簽章與安裝包；兩份更新清單指向同版 NSIS。

機器紀錄見 [exe-verification.json](../offline/exe-verification.json)、[environment.txt](../offline/environment.txt)。後續完整驗證需補全 workspace、專案／Python 隔離及長任務、Office／SMB、NSIS 安裝／更新／回復／解除安裝；本輪均不宣稱完成。

## 浩辰 CAD 測試包

[工具說明](../tools/gstarcad-probe/README.md) · [本機自檢紀錄](../offline/cad-probe-verification.json)

- 診斷列出 32／64 位元 COM 註冊，讀取指定應用身份與文件數，不讀既有圖面正文。已確認年份由新到舊排序，未知年份依內部版本排列且明示未知；不把 `.27` 當作 2027。執行檔不存在的遺留登錄僅保留在報告。
- 新建 200×100 mm 矩形與四個直徑 10 mm 孔位圓；另一模式新建相同圖面並增加 200、100、直徑 10 標註。另存獨立 DWG、重開讀回幾何與單位，留下文件供人工檢查。
- 只操作自己新建的文件；失敗不重播新增操作、不自動換版本，逾時僅終止工具 Python，CAD 結果標示未知並交由人工確認。
- 包含 CPython 3.13.12 x64／x86、pywin32 311、原版授權及離線重建輸入；依賴 SHA256 固定。這些是上游預編譯二進位，沒有使用本機 v142 重建，不併入 CompanyAI 的分析 runtime。
- 本機兩種位元數的載入、年份排序、合成幾何核對、異常幾何拒絕、無 COM 物件時的子程序失敗報告通過。ZIP 檔案雜湊逐一核對，工具來源與包內來源相同。
- 開發機登錄掃描沒有 CAD 候選；**2008／2022 真實 COM、繪圖、尺寸顯示與 DWG 存檔重開均未實測**。公司請分別手動選版，按診斷 → 新建 → 標註測試，回傳 results 報告。

## 最終產物

最終建置結果：**PASS_BASIC**；紀錄時間 2026-10-14T17:31:30.5810812+08:00。建置與必要封裝成功，完整驗證未完成。

| 產物 | bytes | SHA256 |
| --- | ---: | --- |
| `dist/LM_AI.exe` | 17748480 | `c26cd6c15f812ffe14017a21fb5f1e08653754d97e9f54c429bf59edf645cfc9` |
| `dist/LM_AI_Setup.exe` | 28714325 | `acd65c8b62c67b3c707d1c67137cc65a36a2474e33e9bb08e67b08e942bc1a25` |
| `offline/GstarCAD-Probe.zip` | 78280532 | `a6c540ce523f00e65b008f26033eb180a91af957b83496f580c7e46275046a34` |

既有 Python runtime 指紋、icon.png 與歷史 Rust 離線 ZIP 未修改。offline/installer-verification.json 是上一版完整安裝紀錄，不代表本版已完成安裝流程驗證。
