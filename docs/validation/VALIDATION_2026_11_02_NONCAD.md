# 九項改善：非 CAD 候選驗證

> 後續授權：使用者已改為先發行0.8.76，CAD留下一版。下文保留候選階段的版本、驗證與原授權紀錄；最新交付狀態見[0.8.76驗證](VALIDATION_0_8_76.md)。

狀態：2026-11-02 14:50 非 CAD 自動化驗證完成；Luna 模型內容與多步交付未全面通過，不能視為整體驗收完成。版本保持 0.8.75；不進版、不打包 NSIS、不提交或推送。CAD 由使用者手動開啟後另行驗證。本文件記錄本輪證據，不沿用先前發行版結果。

## 建置與範圍

使用 `scripts/Build.ps1 -EmptyCargoCache -ValidateOnly -SkipCad -TestOffice -TestVnc -NetworkTestRoot <本機 SMB 測試資料夾>`。實際 MSVC 14.29.30133，探針 `_MSC_FULL_VER=192930159 x64`，Windows SDK 10.0.19041.0；保留 Cargo.lock、vendor 及專案 .cargo/config.toml，以空 Cargo 快取執行 frozen 建置。

`ValidateOnly` 保留原 dist／offline 發行產物；`SkipCad` 只能配合完整 ValidateOnly，明確跳過 50 個名稱含 CAD 的 Rust 測試、CAD runtime 與原生檢查，不列為通過。

## 已確認的結果

| 範圍 | 本輪證據與結果 |
|---|---|
| Rust | 394 個非 CAD 測試通過；格式與 Clippy 通過。 |
| 新版工作介面 | `Test-Workbench.cjs` 21 項通過，包含偏好／條件、定點修改、流程草稿、成果分組與亮／暗／窄視窗；窄視窗截圖已目視。 |
| 既有介面 | Settings（含55項互動教學）、Preferences、Review、ReplyTables、UiAuth（19個對話框）、ChartLayout、Charts 全部通過。 |
| 檔案與來源 | AppContainer 檔案／網路隔離、副本編輯與來源保護、比較、程式碼分頁、30份約10MiB LOG 與CSV匯出通過。 |
| 原生代理 | 最新同一EXE完整重跑案例0–27通過；包含134次HTTP往返的長任務、checkpoint中斷恢復，以及等待模型時修改條件卡後攔截舊寫入／finish。 |
| 舊契約及恢復 | 舊版工具往返、長閱讀、壞回覆恢復、60次實際工具操作後續接、逾時／未知提交／遲到回覆、連線重試及圖片案例通過。 |
| Python | 真實CSV／XLSX內容指紋、來源版本、Big5、批次部分失敗、Python副本與實際程式驗收、Excel COM資料橋接、隔離、取消、120秒逾時、runtime改寫拒絕及交付時讀回通過。 |
| 其餘完整原生驗證 | Office十種格式及新建文件保存重開、PNG嵌入與移除外部圖片後讀回、報表數值綁定與新來源重跑、本機SMB及兩種VNC連線模式全部通過；VNC未另做畫面目視驗收。 |
| GPT-6 Luna | 已實際往返；三圖無重讀與逐來源筆記流程通過，最新回合有一次圖面指數誤讀；Office／程式多步交付未通過。詳下方，不算全面驗收成功。 |

## 失敗與修正保留

- 首次 Rust 測試有兩個舊預期：快速模型環境提示不再為空、新增交付清單工具後的工具數量；已更新契約測試後通過。
- Python worker 與舊 runtime 不一致時，正式建置正確停止。用原離線輸入重建新目錄，核對只有 `lm_worker.py` 的雜湊／大小改變，更新來源鎖定清單後重新編譯。舊 runtime 保留。
- UI首次受限環境無 DevToolsActivePort；在可啟動桌面程序的環境重跑。新工作對話框使舊測試選到已關閉視窗、登入鎖定測試仍預期16個靜態視窗；修正選取條件與18個靜態視窗預期後通過，另保留動態視窗核對。
- 舊合成模型沒有登記新交付清單，CSV也使用巢狀 `dataset.path`。測試改為透過正式 `update_deliverables` 登記真實工具成果，並增加相應HTTP回合計數；沒有關閉產品的交付門檻。R3失敗與R4提前停止紀錄均保留。
- 關機後先核對保留紀錄與程序狀態再續跑；不把中斷視為通過。測試刻意注入的 panic 有明確 fixture 標記，須後續恢復成功才判通過。

## 證據位置與限制

本機紀錄位於 `.build/noncad-20261102-*`；最新候選來源與EXE雜湊在 `.build/noncad-20261102-feedback-candidate.json`；模型案例及原始參數／回覆保存在 `.build/noncad-20261102-model/`。這些是合成測試資料，未列入發行包。

CAD、公司模型／伺服器、公司SMB權限及真實Outlook信箱尚未實測。Outlook管線與合成來源測試不等於公司信箱通過；本機SMB也不等於公司分享環境。NSIS安裝／更新／移除留待CAD合併後的最終進版。

## 追加修正與最新候選

最新 EXE 為 0.8.75，SHA256 `d42cfdaa8e665c98cc76bccc2dae0aa4649a0a5f67ba15e5d8ac6af0bde01388`；凍結於 `.build/noncad-20261102-model/candidate-feedback-r3/bin/LM_AI.exe`。重連每次等待只寫一筆、歷程保留與分頁、專案獨立收合、短 ID、逐來源筆記與定向查回已接入來源。短 ID 限同一任務；暫停／恢復沿用，不截斷 UUID 或匹配前綴。

完整非CAD基線 R5 已於 13:25 通過並另存 `source-verification-r5.json`，包含 Office／SMB／VNC。追加修改以 `noncad-20261102-feedback-full-r3.log` 的編譯、394個Rust測試與原生自檢，加上 `noncad-20261102-feedback-resume-r1.log` 的完整專案harness重跑及正式Python／Office／SMB／VNC區段完成驗證。後者14:50退出0，EXE雜湊完全相同；不是單次從頭到尾成功的Build執行。當輪報告另存 `source-verification-feedback-final.json`，沒有拿R5代替本輪結果。

新增回歸涵蓋：跨秒只產生一筆重連通知與可取消、512筆歷程保存／恢復／追加、各專案獨立收合、350筆介面分頁、短ID相同前綴不混淆與恢復、圖片來源／版本及一次補辨識守衛。14組圖片HTTP案例中新增短ID查封存、同圖改焦點先回筆記、具體缺項補辨識；既有100張圖片額度案例改用真實不同來源版本，沒有關閉產品守衛。

完整追加驗證前兩輪失敗保留：R1 的測試 EXE 仍在執行而造成 LNK1104；R2 的舊CSV固定案例仍以 read_work_log 期待原文。後者已改用具體 reason 的 read_reading_notes，與新版產品契約一致。原生長閱讀案例同樣增加筆記→定向原文回查一步，現在各134次HTTP；刻意注入的中斷須恢復成功才算通過。

## Luna low 真模型結果與內容限制

全部使用明確指定的 `gpt-6-luna`、`low`，透過正式 App／runner／Broker 執行；每次模型推論為獨立 Codex app-server thread，避免額外模型歷史補回應用已移出的資訊。原始請求、工具參數、回覆、錯誤與產物保留，沒有替模型改參數。這是本機合成資料測試，不是公司端模型／延遲的等價驗收。

| 案例 | 最新候選結果 |
|---|---|
| 三張合成 EIS 圖 `reading-final-r1` | 11次推論（8主回合、3圖片回合），每圖僅辨識一次，3份筆記使用 `op_0002/0004/0006`，無查工作紀錄或重讀。流程通過；內容未全通過。 |
| EIS內容核對 | DRT第一峰真值約4×10⁻⁵s，首次辨識誤讀為4×10⁻⁶s，筆記與最終答案照抄此錯誤。其他兩峰位置、三峰高度及座標軸／單位基本相符，沒有捏造相位曲線，化學機制明列未確認。先前 `reading-r1` 同圖13次推論的讀值全部符合估讀容許範圍；保留兩輪差異，不只挑通過的一輪。 |
| 程式／Mermaid `code-r3` | 已保存MMD與SVG，但模型將來源檔名放入文字答覆的交付target，反覆被原生驗證拒絕後暫停。未交付完整第一輪，後續修正／實測要求未開始，不能算通過。 |
| Word／JSON及缺件 `boundaries-r3` | 新建入口提示後可建立Word副本，但後續格式參數反覆缺欄，並將英文修訂說明當成Office revision，原生拒絕並暫停。沒有完整交付，第二輪缺件要求未開始。 |
| Word／Excel／PNG與條件變更 `reports-r2` | 反覆登記清單／載入技能，沒有產出要求成果，停滯守衛暫停。第二輪條件卡更新的真模型情境未開始；條件卡本身已有原生在途變更回歸。 |

這些結果證明「只看筆記」不能保證正確。筆記保留了初次辨識錯誤，沒有自行驗證圖片數字。因此原圖、原文與加密封存維持可查；不採刪除原檔方案，不把沒有重讀或原生任務結束等同於內容全部正確。

先前 `code-r2` 的第一輪靜態分析及MMD/SVG交付通過，第二輪修改字元位置失敗；已增加Unicode字元位置診斷，但最新模型因交付清單先失敗，未走到再次驗證該修正，不能宣稱模型已用新診斷成功修好程式。原生Unicode位置單元回歸通過。

轉接器亦保留修正前失敗：圖片原先只有文字序列化，現依正式 image input 傳送原始圖片位元組；一般文字回覆改為原樣交桌面格式修復。早期Office案例試圖使用本機Codex的額外node_repl MCP，偵測後停止，不列為通過；最新測試明確關閉此MCP並保留額外工具偵測。協定依本機CLI產生的schema與[官方App Server文件](https://learn.chatgpt.com/docs/app-server)、[MCP設定參考](https://learn.chatgpt.com/docs/config-file/config-reference)核對。

模型彙整：`.build/noncad-20261102-model/validation-summary.json`，各案 `independent-inspection.json` 及讀圖案 `content-review.json`。本輪不進版、不打包；CAD仍待使用者開啟，模型多成果與內容品質仍需後續改善與驗收。

追加R3停在新歷程斷言：案例使用source=null建立新檔案，測試卻尋找「建立副本：完成」。已改為「建立新檔案：完成」，保留大於120筆及首次操作只出現一次的檢查。應用程式沒有因這次測試修正而改動；再次由正式Build.ps1以空快取編譯，核對EXE仍為上述d42cfdaa雜湊，再重跑完整project_smoke及正式Build.ps1剩餘Python／Office／SMB／VNC區段。證據為 `noncad-20261102-feedback-resume-r1.log`，不是把先前失敗改寫成通過。

最終核對：365份凍結來源檔案與EXE雜湊一致；新增歷程斷言修正僅影響測試。最終自動化結果 `AUTOMATED_NONCAD_PASS_MODEL_ACCEPTANCE_SEPARATE`，模型彙整為 `AUTOMATED_NONCAD_PASS_MODEL_NOT_FULLY_ACCEPTED`。原dist／offline發行產物未更新，版本仍0.8.75；CAD與模型未通過項目需在最終進版前處理及驗收。
