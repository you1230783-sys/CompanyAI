# localhost Luna 實測（2026-10-16）

## 範圍與建置

使用者授權建立 localhost 轉接並實際送出 CAD 分頁核對。版本維持 0.8.67，
僅建立 target 下隔離測試 EXE；沒有更新 dist、NSIS、版本號或推送 Git。

- `scripts/Build.ps1 -EmptyCargoCache -ValidateOnly -BasicValidation` 通過。
- MSVC v142 x64：VCToolsVersion 14.29.30133，實際 `_MSC_FULL_VER=192930159`。
- Cargo.lock、vendor 與專案 .cargo/config.toml，空 Cargo 快取及 `--frozen`。
- 基礎 Rust、Clippy、前端／Python／CAD 自檢與原生程式碼閱讀流程通過；CAD 合成契約 61 項通過。
- 額外 `cargo test --lib projects::progress:: --frozen`：26 項通過。
- 證據：`.build/luna-build-r2.log`、`.build/luna-progress-tests.log`。
- 首次受限環境的 CAD 暫存目錄測試因 PermissionError 失敗；在授權環境重跑以上整套通過。
  不將第一次失敗當成產品錯誤，也不將本次基礎驗證稱為完整發行驗收。

## 合成圖面

使用正式 CAD worker 在本機浩辰 CAD 2022 建立法蘭板，正弦曲線使用 600 個取樣線段，
總計 645 個 ModelSpace 圖元。建立、存檔、重開及可讀屬性核對完成，
狀態 `DATA_VERIFIED_VISUAL_PENDING`，不代表外觀與製造適用性已核准。

- 圖面：`.build/luna-fixture/flange-600.dwg`。
- 計畫／證據：同資料夾 `flange-600.plan.json`、`flange-600.report.json`。
- 不修改其他已開圖面，核對任務不執行存檔、ZOOM、PURGE、AUDIT。

## 轉接與重試

入口 `http://localhost:8766`，僅監聽 IPv4 loopback。Codex 模型清單核對
`gpt-6-luna` 支援 `low`，使用既有登入，不複製登入憑證。

早期 JSON 模擬工具的第一輪逾時（240 秒）；短句診斷約 4 秒成功，
同份合成請求的另一診斷誤判成沒有 CAD 工具。這不足以證明公司服務的逾時原因。
最終改用 App Server 原生 dynamicTools，直接取得工具名稱與參數再交還 LM_AI。
額外診斷請求曾因未證明資料範圍被自動審核拒絕；檢查只有靜態規則、空記憶與
合成測試要求後，重新提出並獲准執行。桌面送出亦曾遇操作授權逾時，已重新送出。

## GUI 實際分頁測試

在 LM_AI 隔離專案介面輸入並送出唯讀核對，走正式 runner、Broker、上下文整理與 CAD worker。
每輪 Codex 都是新的 ephemeral thread，不靠另一份跨輪記憶記住 offset。

紀錄目錄：`.build/luna-local-1a1470f91e66460b8e592d934c963502`。
`turn-*.request.json` 是桌面真正送出的快照，`turn-*.response.json` 記錄工具選擇與耗時。
`python tools/summarize_luna_test.py <紀錄目錄>` 產生 summary.json。

第一次完整執行（18:17:14–18:24:10）：9 次模型請求，7 次讀圖，
offset 為 `0 → 100 → 200 → 300 → 400 → 500 → 600`，沒有重讀。
全部範圍達 `[0,645]`，all_entities_delivered=true；每輪只有最近一組完整工具結果，
因此包含實際上下文縮減。7 次 CAD 桌面間隔為 52.604–54.152 秒，
讀圖決策的模型輪次為 3.070–4.438 秒，最終回覆模型輪次 9.583 秒。

分頁行為通過，但任務回答未通過：Luna 呼叫 finish，正文卻只說要再查回前段紀錄，
沒有交付核對表。檢查發現 cad_active_read 的工具定義根本沒有 progress_note，
導致讀取 7 頁時交接筆記一直停留在載入技能的舊內容。
已補齊 cad_active_read、cad_active_edit、cad_excel_plan、cad_excel_apply 的筆記欄位；
6 項 schema 測試通過，修正版再次以 v142 x64／空快取 --frozen 完成整套基礎建置，
證據為 `.build/luna-build-notes.log`、`.build/luna-note-schema-tests.log`。

### 第二輪與人工補充

紀錄目錄：`.build/luna-local-f125a218c2e24550b3af79213aaef6e6`。
同一圖面、同一要求與新對話；第一次送出時，模型將 version 寫成 `2022`，
列出 cad_inventory 後仍重複填年份。三次讀取均在版本選擇階段失敗，沒有取得圖元；
因此不能把這三筆 offset=0 當作上下文造成的重讀。

在 GUI 明確補充 version 應填 inventory 已返回的 `GStarCAD.Application.22`，
沒有重開、修改或重畫 CAD。這是有人工補充的續測，不是完全無人介入的成功案例。
續測於 18:34:01–18:41:09 完成，共約 427 秒：9 次模型請求、7 次成功讀圖，
offset 為 `0 → 100 → 200 → 300 → 400 → 500 → 600`，覆蓋 `[0,645]`，
all_entities_delivered=true，沒有重讀頁面。模型的讀圖決策為 4.443–7.685 秒，
CAD 桌面間隔為 52.904–54.131 秒；最後模型回覆 9.823 秒。
第一頁主要幾何與標註資訊確實出現在後續 progress_note，最後輸出了分類核對結果，
在應用程式介面確認已顯示。

但答案正確性不通過：Hatch `handle=2B3` 的讀取結果僅有 Color／Layer／Linetype／
LinetypeScale／Normal，kind=unknown，沒有 PatternName／PatternScale／島嶼邊界；
模型卻宣稱 ANSI31、比例3、孔內未填充均符合。合成計畫雖使用這些設定，
模型並未讀過計畫檔，不能把使用者規格當作讀取證據。另未逐項交代中心線。
故只驗證分頁與筆記流程，不宣稱模型已可靠完成全部 CAD 核對。

年份字串與 ProgID 的參數提示有改善空間，
此次未改 CAD 版本選擇語意，也未將年份自動映射到某個版本。

summary.json 的 finish_called 僅代表模型呼叫結束工具，不表示答案經驗收。
CAD 回傳後到下次請求的間隔另計；其包含桌面輪詢與上下文整理，不冒充精確 COM 計時。
工具請求到來前若尚未收到用量事件，codex_usage 留 null；有值也只是最近一次用量事件，
可能尚未包括工具參數輸出，不能據此推斷完整輸出 token 或公司服務的上限。

## 限制

本機 GPT-6 Luna low 不等同公司模型，不能推算公司排隊、token 上限或實際推論延遲。
目前每頁仍重新掃描全部 ModelSpace，分頁進度修正沒有消除這項成本。
這次使用已保存的合成圖面；未修改途中、公司 CAD 環境、完整第三項控制盒生成流程尚未測試。
