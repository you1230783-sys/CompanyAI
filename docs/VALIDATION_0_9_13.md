# 0.9.13 驗證紀錄

範圍：手冊 M7，以及第二節複核補充的通用 Python 詞彙與暫停通知。真實模型 EIS 比較依使用者指示留待公司端，不更動 50K 輸入預算，不加入新的 checkpoint、AI 刪檔或 M4 第二階段偵測。

## 實作與初步驗證

- M7：`7f3e385`。NumPy 純量／小陣列轉換、12,000 字元上限、NaN／Inf 路徑提示；stdout 不升格為摘要；寫入前契約拒絕與記憶保存失敗分流。
- 通用技能：`364df0e`。移除 EIS 專用名詞，保留數值登記與來源規則。
- 暫停通知：`15bcf31`，補通知中心可見性與對話導向，沿用彈出偏好及既有桌寵通知。
- 六項封裝 Python 單元測試、v142 frozen 的分析 4 項／新增登記與提示 2 項測試通過；all-target Clippy 通過。
- 原生 UI 新測試初次編譯因漏傳通知讀取的 session 參數失敗，補正後 Clippy 通過；不隱藏失敗。

## 最終驗證狀態

已完成本版整合建置、封裝、同提交 CI 與 Release 公開下載校驗，詳下方結果。

CAD 圖面、公司真實模型與真實 Outlook／加密文件不在本輪本機驗收範圍。暫停通知自檢使用隔離帳號與隱藏視窗，不代替公司使用者手動點擊桌寵的驗收。

## r1 整合執行進度（發布前紀錄）

- 468 項非 CAD Rust 與 5 組前端通過。隱藏視窗通知自檢 15 項通過，包含新增的暫停通知保存／可見性／彈出偏好／對話導向。
- 原生 HTTP 案例 0～31 全數通過；M7 案例31走 8 個模型回合，驗證 print-only JSON 不升格、兩項契約拒絕為 Code14，之後 NumPy result 能被引用與登記並附表。
- 案例19的 `fixture: simulated interruption` 是測試刻意注入、由 catch_unwind 捕捉的中斷；後續 69 步／134 POST 完成且原件保留，不列為非預期失敗。
- 來源 `5f89ae0` 的 GitHub CI `42127495093` 通過；下載產物驗證 v142 192930159 x64、空快取、預設／核心配置及依賴政策，Cargo.lock 雜湊與本機一致。
- 本段紀錄時，剩餘 Python／Office 整合及封裝仍在執行；最後結果以下方發行證據為準。


## 發布與最終證據

- 來源與 v0.9.13 標籤：`5f89ae0da50c4469332df69d3871ad7f06b0b377`；後續提交僅補文件及公開收據，不移動標籤。
- `scripts/Build.ps1 -IncludeInstaller -EmptyCargoCache -TestOffice -SkipCad` r1 完成。實際 MSVC v142 14.29.30133／192930159 x64；保留 Cargo.lock、vendor、專案 .cargo/config.toml，以空 Cargo 快取 frozen 驗證。
- fmt、all-target Clippy、468 項非 CAD Rust（60 項 CAD 篩除）、release 故障恢復及實際 EXE 自檢通過。
- 32 組原生 HTTP 案例全數通過。新增案例31以真 AppContainer 證明 print-only JSON 兩種引用被拒絕且為 Code14，改用 NumPy result 後 record_analysis、register_report_values 與回答附表成功。
- 六項 Python 正規化測試通過；實際 AppContainer 另核對 NumPy 純量／陣列、NaN／Inf 鍵路徑、超限拒絕且不冒稱未執行。離線清單僅 lm_worker.py 變更，套件版本未改。
- 隱藏視窗自檢核對暫停通知保存、可見性、關閉／開啟彈出偏好、點擊開啟正確對話且導向僅消耗一次。自檢抑制真正桌寵彈出；既有原生桌寵可見性／設定測試另外通過，不宣稱公司端人工點擊已驗收。
- Python 科學計算、隔離、專案長任務／checkpoint、大型 LOG、Office 副本與格式讀回、圖片與排程占用處理通過。
- NSIS 安裝、更新、PID 交接、重啟、失敗回復、解除安裝及安裝後 Python／CAD runtime 自檢通過；兩份簽署更新清單一致。
- [同提交 GitHub CI](https://github.com/you1230783-sys/CompanyAI/actions/runs/42127495093) 與下載證據核對通過，涵蓋預設／核心配置及漏洞政策；Hosted CI 排除的兩個桌面通知測試已於本機執行。
- 五組前端：ReplyTables、Settings、Conversations、Workbench、ReliabilityUI 通過；不宣稱重跑其他無關介面。
- EXE SHA256：`3a97f4c6d178f7f19f22f6621c608a9b25786380e425e4603bba9c86f5afed05`。
- 安裝包 SHA256：`33d48801609ac33860b17a43ee83eb791a1f95a4cf37603790e9a06f1354b1e9`。
- [v0.9.13 Release](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.13) 五項資產以公開、無授權標頭的網址下載，大小與 SHA256 均核對通過。

證據：[發行](../offline/release-verification-0.9.13.json)、[完整建置](../offline/exe-verification-0.9.13.json)、[安裝包生命週期](../offline/installer-verification-0.9.13.json)、[CI](../offline/github-ci-verification-0.9.13.json)、[前端](../offline/frontend-verification-0.9.13.json)、[原生通知](../offline/composer-verification-0.9.13.json)、[Python 摘要](../offline/python-summary-verification-0.9.13.json)、[公開下載](../offline/release-download-verification-0.9.13.json)。私有原始日誌留本機，以雜湊對照。

真實模型 EIS／DRT 仍由公司端驗收；合成串接成功不代表科學方法或單位正確。M4 第二階段偵測另排。既有 Word 彩色 emoji PDF 文字讀回限制、經審核且有期限的 ttf-parser 維護公告例外保持。GitHub 是手動下載入口，未部署公司更新伺服器。

依使用者最後指示，本版發布及公開下載校驗完成後停止。0.9.14～0.9.17 尚未實作，待再次指示；需求手冊原文未修改。
