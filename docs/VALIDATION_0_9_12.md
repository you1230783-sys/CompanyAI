# 0.9.12 驗證紀錄

依 CHANGE_HANDBOOK_EIS_FEEDBACK.md 實作 M1–M6。使用者已指定真實模型比較留待公司端驗收；本機合成資料不能代替公司 EIS／DRT 品質驗收。

## 實作與定向驗證

- M1（4ccf71f）：結構化 executed:false 契約拒絕使用14，真正或未知執行失敗保留原分類；分類測試通過。
- M2（1722c5b）：一次綁定六項、失敗不部分更新、取代關係保存與循環拒絕；Broker驗證原檔保留、主清單篩選及舊成果雜湊仍必須正確。三項測試通過。
- M3（a803fce）：Python操作不能當原文筆記，live與archived操作都提供原文／登記值指引；測試通過。
- M4（ebcb048）：45項登記值跨保存與壓縮仍保留，固定區塊限制32,000 bytes；原生回答表包含原始值／顯示值／單位。兩項定向測試通過。
- M5（da6fcd4）：空白截斷、相近與不同輸入、缺失usage、可見內容、工具呼叫、保存／續接及CAD提示分開測試；新舊三項測試通過。
- M6（364d313）：耗時實測、缺值與零、截斷合計及加密多輪紀錄；五項診斷測試通過。

## 整合驗證與發行

已完成r3整合驗證、封裝、同來源CI、GitHub Release與五項公開下載校驗。原生HTTP新增案例28–30覆蓋第二次空白截斷暫停、兩類契約拒絕14，以及Python worker→數值登記→下一輪→最終原生表。

公司端仍需同資料、同需求的真模型比較與Claude複核；M4第二階段自由文字數值不一致偵測未納入。50K輸入軟預算、伺服器輸出額度、既有checkpoint及AI不得刪除檔案的限制保持原約定。

## 驗證中修正紀錄

- 首次完整建置 r1 的 Clippy 遇到 Windows os error 32：原生整合測試正在持有輸出目錄。停止重疊執行，改依序跑原生案例再完整建置。此項是測試排程造成的檔案鎖衝突，不隱藏第一次失敗。
- 五組前端回歸（ReplyTables、Settings、Conversations、Workbench、ReliabilityUI）已通過。

- 原生HTTP案例28、29已通過；案例30第一次因測試誤用load_skill.name被Schema拒絕，改為既有id欄位後通過。三項均使用正式runner；Python案例真的執行AppContainer worker，非直接注入登記狀態。
- r2指定v142探針（192930159 x64）與all-targets Clippy已通過；其後完整Rust回歸發現下列兩項遺漏，已修正並由r3完整重跑。
- RustSec刷新與12項政策測試通過；保留已審核且有期限的ttf-parser維護公告例外，不宣稱零警告。

- r2完整Rust回歸為464通過、2失敗：新交付工具遺漏progress_note共用Schema欄位，目錄解析案例也未加入兩項工具。補齊欄位與實際解析樣本，保留既有契約測試，不放寬斷言；完整流程改由r3重跑。

## 最終結果

- 來源提交：`42fc74f5a0f725426ce98afdf71da48224c69625`，標籤v0.9.12固定指向此提交；後續提交僅補文件與證據。
- `scripts/Build.ps1 -IncludeInstaller -EmptyCargoCache -TestOffice -SkipCad` r3完成，MSVC v142 14.29.30133／實際192930159 x64、Cargo.lock／vendor／專案.cargo/config.toml與空快取frozen。
- fmt、all-targets Clippy、466項非CAD Rust、release故障恢復、實際EXE的Python／原生UI自檢通過。
- 31組原生HTTP案例、30個約10MiB LOG、隔離／原件保護／分頁／重試／checkpoint續接／加密筆記／PDF等專案回歸通過。新增28–30於修正後完整r3再度通過。
- Python科學計算與隔離、真Office副本／Markdown格式讀回／圖片／交付、排程占用安全略過通過。
- NSIS真正安裝、更新、PID交接、重新啟動、失敗回復、解除安裝及Python／CAD安裝後自檢通過；兩份簽署更新清單一致。
- [同提交GitHub CI](https://github.com/you1230783-sys/CompanyAI/actions/runs/42053066119)與下載的驗證產物核對通過，含預設／核心配置及漏洞政策。Hosted CI排除兩個互動桌面通知測試；本機466項包含它們。
- 五組前端：ReplyTables、Settings、Conversations、Workbench、ReliabilityUI；沒有宣稱重跑所有舊版前端測試。
- 正式EXE SHA256：`ff7b17715cfa1e6dae35cc439a6c8b15f8df66aca3818696bfb573d52000b03f`；安裝包：`b781e421396865e9b976ee5bb9c94cc34e9d2660fefcff25694dea87f42226e0`。本輪整合與封裝為同一EXE。
- [Release](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.12)五項資產以公開、無授權標頭的網址下載，逐項大小與SHA256核對通過；[下載收據](../offline/release-download-verification-0.9.12.json)。

證據索引：[本版發行](../offline/release-verification-0.9.12.json)、[完整建置](../offline/exe-verification-0.9.12.json)、[安裝生命週期](../offline/installer-verification-0.9.12.json)、[同來源CI](../offline/github-ci-verification-0.9.12.json)。原始私有日誌留本機，以雜湊對照，未提交模型對話或憑證。

## 公司端驗收界線

真實模型EIS／DRT比較依使用者指示留待公司端；需比較Error99、交付清單重送、截斷次數／耗時、資料描述與成果版本。本次合成值0.001Hz／121筆僅驗證串接，不能代替反演品質或公司資料正確性驗收。M4自由文字數值不一致偵測另排第二階段。

本機未重跑CAD圖面、公司VNC／SMB／信箱／防毒與多螢幕。既有Word彩色emoji轉PDF的文字讀回限制保持；RustSec保留有期限的ttf-parser維護例外。GitHub是手動下載入口，未部署公司更新伺服器。

五組前端的逐項結果與日誌雜湊見[前端驗證收據](../offline/frontend-verification-0.9.12.json)。證據彙整最初假定所有腳本輸出PASS字串；Workbench實際回傳passed:23，已核對完整23項結果並改依欄位驗證，測試本身通過。
