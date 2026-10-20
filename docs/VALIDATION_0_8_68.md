# 0.8.68 發行驗證

日期：2026-10-16。狀態：0.8.68 編譯、整合與安裝生命週期驗證通過。交付 EXE、NSIS、兩份簽署更新清單與對應原始碼。

發行版本為 0.8.68。新 CAD runtime 已由鎖定的離線輸入重新建立；最終建置日誌為 `.build/release-0.8.68-final.log`。

## 本版範圍

- 新增 CAD 本機完整快照、按條件查詢、精確資料查回與局部候選預覽。品質／快速模型的圖片入口分開控制；見 [CAD_SNAPSHOT_QUERY.md](CAD_SNAPSHOT_QUERY.md)。建置與既有契約通過不等同新增 CAD 流程已完成實機驗收。
- 保存 CAD 分頁的來源、revision、已讀區間及下一頁位置；相同版本重讀首頁保留已知 Handle。
- 補齊四個 CAD 工具的 progress_note，使上下文整理後仍能交接已核對項目。
- 應用程式只內嵌目前版本公告；設定按鈕開啟對話框，取消跨版本自動提醒及重複清單。保留 14 張教學與既有已讀紀錄。
- 納入開發用 localhost Luna 轉接與隔離測試入口；測試連線檔、聊天正文與本機紀錄不納入 Git／安裝包。

## 建置與驗證

以下為恢復發行後，以包含 CAD 快照／查詢／預覽的本版原始碼重新執行的結果；不沿用先前中止的建置紀錄。

命令：`./scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`。

- 工具鏈：Rust 1.98.1；MSVC v142 14.29.30133，實際探針 `_MSC_FULL_VER=192930159 x64`；Windows SDK 10.0.19041.0。
- 使用專案 Cargo.lock、vendor、.cargo/config.toml，空 Cargo 快取及 `--frozen`；未使用 VS 預設新工具集。
- fmt、Clippy 與 workspace 345 項 Rust 測試通過。
- `node tools/test_onboarding.cjs` 通過；`node scripts/Test-Settings.cjs` 的設定、教學、當前版本公告與窄視窗互動重新執行通過。
- release 主程式已編譯成功，檔案版本為 0.8.68.0；61 項 CAD 合成契約與 Outlook 輔助程序管線通過。
- 原生自檢、專案檔案／隔離、30 份大型 LOG、26 項原生代理案例、重試恢復、圖片、長任務續接、記憶及 PDF 整合通過。
- Python 完整預設流程通過：Big5／大型 LOG、來源證據、並行批次、程式修改、CSV／XLSX 讀回、OS 隔離、取消、120 秒逾時、runtime 完整性及成果遭改寫拒絕。
- 發行前修正 Clippy 回報的工具結構大小（查詢條件改用 Box，JSON 契約不變），以及新增工具導致舊文字目錄超過 64 KB 的問題（共用筆記參數只說明一次）；同步既有工具覆蓋與固定座標長度測試後，全套重跑通過。
- 固定 CAD runtime 已重建為 `cad-cb8088a80fabeb03`；NSIS、更新清單簽署與原生套件核對完成。
- NSIS 隔離及正式安裝包的安裝／更新／重啟／鎖檔失敗回復／解除安裝通過；Python 沿用、缺檔／同大小損壞／清單變更修復、使用者檔案保留、安裝後 Python／CAD 自檢均通過。
- 完整 Build.ps1 結束碼為 0；機器紀錄見 `offline/exe-verification.json`、`offline/installer-verification.json` 及本版 Python／批次／程式驗證報告。未啟用的 Office／SMB／VNC 實測仍明列為 false。

本機完整建置日誌：`.build/release-0.8.68-final.log`，不納入 Git。先前 `.build/release-0.8.68.log` 是使用者暫停的舊流程。

## localhost 與 CAD 實測界線

進版前以同批功能修改、隔離的 0.8.67 測試 EXE 完成兩次實際 Luna／CAD 分頁流程；
645 圖元依序讀取 offset 0、100、200、300、400、500、600，未出現 0／100 循環。
詳細紀錄見 [VALIDATION_LOCAL_LUNA.md](VALIDATION_LOCAL_LUNA.md)。

- 上述舊分頁實測每頁約 53 秒。新增本機索引後的效能尚未量測，不沿用先前數字作為新版表現。
- 第二次需人工更正模型誤填的 CAD 年份版本；最終答案把工具未回傳的 Hatch 屬性判為符合，答案正確性不通過。分頁不重讀不能等同完整圖面核對成功。
- 沒有實測公司模型／網站，也不能用 localhost 結果證明原先逾時來自輸出 token 上限或服務排隊。
- 本次未指定全套 Office、SMB 或 VNC 整合選項；過去版本的相關報告保留歷史意義，不冒充本版驗收。CAD 外觀與公司環境仍需實測。
- 歷史離線 ZIP 未重製；本版交付範圍為原始碼、EXE、NSIS 及更新清單。

## 產物

| 產物 | 位元組 | SHA256 |
| --- | ---: | --- |
| `dist/LM_AI.exe` | 19,391,488 | `85a754988e3480438732c85756fec61f722b42bf544a58460bab0745118c2b26` |
| `dist/LM_AI_Setup.exe` | 36,252,717 | `59f99d2796a83a06f6b70e286930bac695349c04534ee704b2c901495c429073` |

EXE／NSIS 檔案版本均為 `0.8.68.0`。`update-manifest.json` 與相容端點 `update-manifest-exe.json` 內容一致，均指向本版完整 NSIS；原生更新器已核對其簽章及套件內容。歷史整套 Rust 離線 ZIP 未重製，不宣稱為本版產物。
