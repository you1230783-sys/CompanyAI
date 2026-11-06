# 0.9.15 驗證紀錄

M9 計畫模式已完成相關驗證、正式編譯、封裝及發布。公司模型驗收由使用者進行。手冊保持原文。

## 實作範圍

- 預設關閉的「先提出計畫」偏好；新任務帶入，續跑以加密 checkpoint 為準。
- 原生工具白名單、broker 與批次再檢查；核准前拒絕寫入並回報代碼14／未執行。
- 提案、使用者編輯核准／退回、重大修訂等待、同一步驟參數調整歷程。
- 使用者核准的產出轉成交付清單；模型不能另行新增、移除或改名來避開重新核准。
- 核對帳號、專案、對話、任務與計畫版本。核准不增加檔案或工具權限；保留既有副本、成果及操作紀錄。
- 卡片呈現完整計畫／修訂前後與歷程，文字不作為 HTML。手動繼續不能繞過核准，未核准的續跑也不覆寫原日誌。

## 發行前的驗證過程（最終結果見後段）

- v142 14.29.30133 環境；最終 empty-cache frozen／封裝仍待完成。
- 核心狀態及加密 checkpoint 4 項通過。原生工具拒絕及身分回歸亦已通過。
- 代碼14調整後 478 項非 CAD Rust 回歸通過（60 項 CAD 過濾）；最後唯讀清單名稱校正正執行最終重驗。
- 計畫介面14項、既有差異預覽40項、對話32項通過。亮／暗／650px 窄視窗已核對；修正初稿輸入框及按鈕邊界。
- 實際 HTTP／AppContainer 計畫流程 r4 通過（10 個唯一請求、未核准續跑不覆寫日誌、單一成果）。既有暫停恢復、正式 EXE 與 NSIS 尚待最終驗證。

## 過程中的失敗與處理

- Clippy r2/r3：新增測試的未使用匯入、閉包型別與未讀取賦值，修正後 r4/r5 通過。
- UI r1：測試重用相同訊息內容，命中既有 DOM 快取而保留停用按鈕；fixture 改用不同內容重建待測畫面，r2/r3 通過。
- 全 Rust r1：新增工具未加入目錄樣本，已補兩個工具解析案例；Windows 托盤測試在受限環境拒絕，改於正常桌面環境 r2 通過。
- 原生 r1 使用相對路徑被既有路徑保護拒絕，r2 改絕對路徑。
- 原生 r3：流程完成，僅測試文字比對未處理正式 UTF-8 BOM；r4 依文字格式比對通過，10 個不同模型請求、只保存一份成果。
- 原生 r2 發現未核准工具先被原生回覆契約拒絕，沒有進到 broker 代碼14。補上計畫專用解析入口，在完成身分／封套核對後回傳未執行的修復結果；一般任務未知工具仍拒絕。
- Clippy r6：新增身分測試誤用扁平欄位，已改以正式 agent_envelope 修改對話 ID，須重驗。

## 限制

本版使用固定合成模型步驟驗證原生執行，不代表公司模型提案品質或長任務已驗收。不重跑完整 Python 科學運算、CAD 繪圖及 Office 文件矩陣；相關既有封裝與 runtime 檢查另記。保留有期限 ttf-parser 維護公告例外。GitHub 手動發行，不部署公司更新伺服器。


## 發布與最終證據

- 來源及 v0.9.15 標籤：`33a07acb4a060997883dd9a0cbba202e4df05336`；後續僅補文件與收據，不移動標籤。
- scripts/Build.ps1 -BuildOnly -EmptyCargoCache：v142 14.29.30133、實際 _MSC_FULL_VER=192930159 x64；Cargo.lock、vendor、.cargo/config.toml 齊備，空快取 frozen 成功。
- 最終 fmt、all-target Clippy、478 項非 CAD Rust（60 項 CAD 過濾）通過。原生身分核對、代碼14與計畫唯讀清單修正均包含在最後重驗。
- 計畫14項、差異預覽40項、對話32項前端檢查通過，亮／暗／650px畫面已檢查。
- 固定HTTP模型／真AppContainer計畫流程通過：10個唯一請求、未核准拒絕、載入技能仍無寫入能力、退回、使用者修改核准、重大修訂暫停、加密續接及單一成果。等待核准直接續跑不改寫舊日誌。
- 既有暫停與中斷恢復回歸通過。正式EXE的五組介面自檢、Python／CAD runtime及完整NSIS安裝／更新／重啟／失敗回復／解除安裝、安裝後runtime檢查通過。
- [同來源CI](https://github.com/you1230783-sys/CompanyAI/actions/runs/42145350765)通過，下載CI報告已核對實際v142、空快取及依賴公告；Hosted CI排除的兩項桌面通知測試已在本機執行。
- [v0.9.15 Release](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.15) 五項公開資產下載大小與SHA256一致。
- EXE SHA256：`5555033ee1aef9d96be9d2ac7819ae2030987a1415a1c83677fdabbd0b968bfe`。
- 安裝包 SHA256：`6d03da48c0858060f1afd7b6588107c8532626f810e1e66a97b15fb0b5d6c3db`。

證據：[發行](../offline/release-verification-0.9.15.json)、[建置及相關整合](../offline/exe-verification-0.9.15.json)、[NSIS](../offline/installer-verification-0.9.15.json)、[前端](../offline/frontend-verification-0.9.15.json)、[CI](../offline/github-ci-verification-0.9.15.json)、[公開下載](../offline/release-download-verification-0.9.15.json)。

這是相關功能驗證，不是完整公司模型／Python科學運算／CAD圖面／Office文件矩陣驗收。公司端另驗。下一版為0.9.16，依授權一路到0.9.17後停止。
