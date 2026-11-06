# 0.9.18 Excel 閱讀精簡：相關驗證紀錄

狀態：相關測試、v142空快取frozen正式建置、原生Office與候選安裝包驗證通過；Git／Release及五項公開下載核對已完成。此文件不代表完整驗收。

## 範圍

依 [0.9.18 手冊](../CHANGE_HANDBOOK_0_9_18_EXCEL_READ.md)實作 X1、X2，加入 0.9.17 後的 M7 混合 stdout 提示與 Office／CAD 禁止巢狀新增應用程式鎖。文件分類整理一併交付；三份手冊保留原文。B（讀取預算）／C（Excel 索引工具）不在本版。

X1 僅修改說明與技能，保留既有工具權限及已規劃來源的匯出限制。X2 僅改 `read_excel_range` 的模型交付格式；`Page／Row／Cell` 欄位和 serde 仍相同，預設100列與2000資料格限制不變。

`excel-compact-v1` 的 rows 每列第一項為原列號，後續對應 columns。column_kinds／column_formats 取本頁最常見值（同次數依字典序），exceptions 保存差異；cell_texts 保存與預設文字不同者，formulas 只列有公式者。未列出的文字按 text_rule 還原；精確保留 1.00、日期、錯誤及特殊格式。headers 使用顯示文字，特殊表頭以 header:true 的完整 cell 保存，避免 header_row 與資料列重疊時混淆。

## 已執行的相關檢查

- fmt、v142 全目標 Clippy：第一輪因排程 smoke 的 Step 新欄位未補齊而失敗，修正後通過；保留 `.build/clippy-0918-r1.log` 與後續紀錄。
- Excel 精簡轉換：完整逐格還原 value／text／kind／number_format／formula，涵蓋字串、數字、布林、空白、日期／時間、自訂格式、錯誤、公式、不連續欄、特殊表頭及空頁。
- 合成100×9一般數值表：舊格式80,111字元、新格式5,807字元，比例7.25%，低於40%目標。這不是實際API tokens，也不代表公式／例外密集的工作表有相同降幅。
- 新舊 Excel 筆記、圖形摘錄、record_analysis／select、封存精確讀回及 checkpoint 續接；流程舊結果可用，新結果不套用舊 rows／headers pointer，重新保存後可使用新 pointer。
- M7 警告／進度混合 JSON、資源鎖 FIFO 互等拒絕／重入／記憶交易、技能目錄及工具 schema 定向測試通過。原始紀錄 `.build/test-0918-r1.log`。

原生 Office 前兩輪在既有時間篩選 CSV 匯出遇 Excel COM `0x80010108`，後續診斷於 Excel 儲存後重新讀回也捕捉到「查詢既有 Excel.Application 文件數失敗／Workbooks」。這是關閉後 ROT 中短暫存留已斷線實例的時序問題，並非新增的巢狀鎖拒絕。保留 `.build/native-0918-r1.log`、`r2.log` 與 `office-diagnose*.log`；其中一次診斷先因測試資料夾使用相對路徑被拒，改為完整路徑後才實際執行。

已補強 `office/availability.rs`：只對開啟前的 ROT／文件數唯讀查詢最多重查10次，間隔100毫秒；只有明確沒有實例或文件數為零才繼續。已開啟文件立即拒絕、持續未知仍拒絕，不忽略錯誤、不重播寫入。新增兩項測試涵蓋短暫斷線後確認空閒、重查後發現占用及持續查詢失敗，均通過。最終原生與封裝結果見下方。

最終 Office 主測試已通過（`.build/native-0918-r3.log`），緊接的資源並行測試因 Office 占用前置檢查而未開始。另開測試程序並確認已無 Office 程序後，只補跑資源案例；結果另存 `.build/resources-0918-final.*`，不重跑成功的 Office 主測試。

安裝包首輪已安裝成功，但自檢的對話分支案例出現 `native completed task applied`；同一EXE直接重跑通過，另一輪安裝自檢則逾時。讀取**本次合成分支資料**確認兩次分別有 WinHTTP12152／12030，並非公司服務失敗。合成HTTP伺服器的accepted socket未重設blocking，封包延遲時讀取會被當成失敗；Windows accept沿用監聽socket屬性也見[Microsoft文件](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-accept)。

修正僅限 `ui/branches/smoke.rs` 的自檢伺服器：accepted stream改為blocking，讀／寫仍有3秒上限；新增連線後延遲、HTTP表頭與內容分段送出的兩次真實TCP要求回歸，通過。未提高自檢45秒總上限、未刪除完成條件、未修改正式聊天的網路流程。重新以v142空快取建置及封裝，最終實測另記。保留 `.build/package-0918-r1.log`、`r3.log`、直接自檢紀錄及延遲封包測試紀錄。

測試安裝清理初次因解除安裝尚未完全結束而停止；之後核對只剩合成標記檔，才刪除該標記與空目錄，未遞迴刪除其他資料。

## 本版未執行的完整驗證項目

依使用者指示，跳過完整非相關 Rust 回歸、完整前端套件、CAD 圖面整體驗證、Outlook 信箱、VNC、SMB、完整科學分析／模型長任務、完整安裝 PID／回復／修復矩陣。為避免推送觸發完整回歸，提交標記 `[skip ci]`；本版不宣稱同提交完整 CI 通過。v142 空 Cargo 快取 frozen 建置仍依專案環境規則執行。

公司端需觀察同檔同需求的實際輸入 tokens、是否改走 CSV／Python、日期／錯誤／欄位映射是否正確。本機固定資料不能代替上述驗收。

## 最終候選與證據

- 48 項不重複相關 Rust 測試通過；fmt、v142 workspace all-targets Clippy通過。包含原先45項、COM占用重查2項及自檢HTTP延遲封包1項；未擴大執行完整測試套件。
- MSVC14.29.30133、實際 `_MSC_FULL_VER=192930159` x64；Cargo.lock、vendor、.cargo/config.toml，以空Cargo快取 `--frozen` 正式建置通過。
- 原生Office：15個規劃／時間篩選CSV與圖表、各Office格式讀寫保存重開、XLS／XLSX精簡日期／錯誤／公式／特殊表頭與read_work_log回讀通過；來源未變，無巢狀鎖誤擋。
- 資源原生：Word FIFO、獨立Excel、使用者Word占用拒絕、兩個Python工作程序及取消隔離通過。
- 最終NSIS候選：基本安裝／更新／啟動、Python／CAD安裝後自檢、使用者檔案保留及解除安裝通過；不代表完整PID／回復／修復矩陣。
- 文件：59份依用途搬移，除路徑外正文一致；377個本機連結核對無失效，文件皆有索引入口。三份手冊及FUTURE_IDEAS.md保留原文並提交。

機器收據：`offline/related-verification-0.9.18.json`、`offline/exe-verification-0.9.18.json`、`offline/office-verification-0.9.18.json`、`offline/resource-verification-0.9.18.json`、`offline/installer-verification-0.9.18.json`。發行後另保存release-verification與公開下載核對收據。

原生Office／資源測試使用修正COM後的候選（EXE SHA256 c403944814bf0ab321753c160d2120d74825d962270c7cdb3df0c0ff39d063e5）。之後只修正自檢的HTTP合成伺服器；Excel／Office／鎖／Python正式功能與runtime未再更動，因此不重跑整套已成功的原生矩陣。最終重建版本另通過延遲封包回歸、Clippy、安裝後自檢與封裝流程，最終雜湊以發行收據為準。

## Git／Release 交付

- 來源提交與v0.9.18標籤：`531efeeae9d40c3639a1f1c29870b8fade5a850e`；發行後僅補文件與收據，不改動標籤。
- [0.9.18 Release](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.18)：安裝包90,118,568 bytes，五項資產皆以未登入方式下載並核對SHA256通過。
- 最終EXE SHA256：`f293ff697b5482063788b936a2ced45e61b75780a506a484b5a747aff6cbe19b`。
- Setup SHA256：`3c6ce78eb293ec7bd17fa3be69e144343ad298b3098b36fd312887be19d54da5`。
- 詳細證據：`offline/release-verification-0.9.18.json`、`offline/release-download-verification-0.9.18.json`。需求文件另記Git儲存內容SHA256，區別工作樹換行正規化。
- 依要求使用 `[skip ci]`，未跑同來源完整hosted CI；公司端真實模型／文件與tokens仍待驗收。此版交付完成後停止，不擅自實作B／C或進下一版。
