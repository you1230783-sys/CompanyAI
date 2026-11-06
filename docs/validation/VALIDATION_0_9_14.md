# 0.9.14 驗證紀錄

範圍：手冊 M8，工作副本的本機差異預覽。使用者已恢復逐版發布授權；公司端驗收另行進行。

## 驗證範圍

- Rust 端文字 LCS、Word 插入段落、Excel 公式／數值／結構比較。
- 固定建立時基準、來源變更提示、新建與不支援格式、分頁／截斷。
- 原生與前端入口不呼叫模型，診斷不包含差異內容。
- v142 x64、空 Cargo 快取 frozen、相關本機整合、必要封裝與同來源 CI。
- Git／Release 發布與公開下載 SHA256。

以下範圍已完成本機驗證與發布；公司驗收另行進行。

## 開發中檢查

- 初次 Clippy 在新比較模組的 JSON 巨集陣列索引語法失敗，補括號後 v142 all-target Clippy 通過（r2）。
- 前端 Review 回歸通過 40 項，含新增 9 項 M8 本機命令／分頁／過期回覆／來源提示／純文字顯示／新建與不支援入口。既有明暗與窄視窗截圖檢查通過；新預覽視覺核對另補，不混稱已驗。

- 6 項 review Rust 測試通過（含 5 項新增 M8 案例），檢查原件變更、Word 中間插段、Excel 公式／值／結構及分頁／容量限制。
- 新預覽明暗／650px 畫面通過版面測試，已人工檢視明亮及窄版截圖；對話工具回歸 32 項通過。
- 本版採相關功能驗證：Build.ps1 -BuildOnly -EmptyCargoCache 後另跑非 CAD Rust、真 EXE 自檢、原生副本／預覽與 Office 文件回歸及 NSIS。未重跑的模型長任務／Python 科學運算不列為本版 PASS；沿用前版已發行功能，不把歷史證據改成新版本。

- 來源提交 `77bf1bb` 已推送；v142 r3 Clippy、473 項非 CAD Rust（60 項 CAD 篩除）全數通過。CI `42139510276` 執行中；發行建置與安裝測試尚待完成。

- v142 空 Cargo 快取 frozen 發行編譯、五組原生介面與 Python／CAD runtime 自檢、真 AppContainer 副本／差異整合、真 Word／Excel 比較／翻譯／匯出及格式讀回通過。同提交 CI `42139510276` 及其下載收據通過；NSIS 封裝／生命週期進行中，尚未發布。
- 最終來源前端重驗 40 項通過；深色預覽截圖已檢視。


## 發布與最終證據

- 來源與 v0.9.14 標籤：`77bf1bb3e0187c457fa86a7b3896301130498507`；後續僅補收據與文件，不移動標籤。
- `scripts/Build.ps1 -BuildOnly -EmptyCargoCache` 成功：MSVC v142 14.29.30133、實際 `_MSC_FULL_VER=192930159` x64；Cargo.lock、vendor、.cargo/config.toml 齊備，以空快取 frozen 驗證。
- fmt、all-target Clippy、473 項非 CAD Rust（60 項 CAD 篩除）及 6 項 review 定向測試通過。初次巨集語法錯誤已修正並重驗。
- 最終來源差異預覽／Review 前端 40 項、對話介面 32 項通過；明暗與窄視窗截圖檢查通過。
- 發行 EXE、Python／CAD runtime、五組原生介面自檢通過。實際 AppContainer 副本建立、修改、保存兩版本、原件外部變更後的固定基準預覽通過。
- 真 Word／Excel 文件比較、翻譯、匯出與格式讀回通過；完整 NSIS 安裝、更新、PID 交接、重啟、失敗回復、解除安裝與安裝後 runtime 驗證通過。
- [同來源 GitHub CI](https://github.com/you1230783-sys/CompanyAI/actions/runs/42139510276) 及下載證據核對通過。Hosted CI 排除的兩個桌面通知測試已於本機 Rust 回歸執行。
- [v0.9.14 Release](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.14) 五項資產公開下載後，大小及 SHA256 全部一致。
- EXE SHA256：`0bdc2274a5bad83312ea155d99a8a34ed748a90ec25aabef4d626edeb6ef29f4`。
- 安裝包 SHA256：`e872876c1655afb6f70e0fb9627070e673a96f2d3efeee14c36041a6feb6dfc6`。

證據：[發行](../../offline/release-verification-0.9.14.json)、[相關範圍建置／整合](../../offline/exe-verification-0.9.14.json)、[安裝生命週期](../../offline/installer-verification-0.9.14.json)、[CI](../../offline/github-ci-verification-0.9.14.json)、[前端](../../offline/frontend-verification-0.9.14.json)、[Office](../../offline/document-work-verification-0.9.14.json)、[公開下載](../../offline/release-download-verification-0.9.14.json)。

本版採相關功能驗證，不宣稱執行完整 Build.ps1 整合矩陣；完整原生 HTTP 長任務、Python 科學分析、CAD 圖面、公司模型／信箱／網路／加密文件未在本版重跑。比較是保存內容快照，不代表 Office 版面或語意核准。文字每側 2,000 行、差異 200 處／48KB、每頁 25 處，超限明示。維持有期限的 ttf-parser 維護公告例外。GitHub 為手動下載入口，未部署公司更新伺服器。

依最新授權接續 0.9.15，最終停止點為 0.9.17。
