# 0.8.47：郵件成果、圖表與診斷

## 使用者行為

- 郵件內文閱讀後，下一操作一併提交摘要與待寫草稿，不另外啟動摘要推論。每份筆記有 source_operation、摘要、草稿、include／exclude／uncertain、收錄理由與未確認事項；來源版本及閱讀頁面由桌面從成功操作核對。部分閱讀不標成全信完成，新版本會標示舊版筆記過期。
- `read_mail_notes` 分頁查索引／筆記，或指定原操作、頁內位置與核對焦點查最多3000字本地原文。重讀同版本已讀區段先回傳成果；必要時 `set_work_stage(read, note_ids, reason)` 可允許一次指定版本的回讀，不重建整批信件查詢。
- `set_work_stage` 在 read／organize／write／chart 間載入 Outlook／notes／office-edit／dataset-charts，收回前一階段工具。寫入時選最多4份草稿，下輪自動帶入。實際成功的修改／儲存操作與當時選定草稿建立關聯，保留副本與版本；這個關聯不是每條文字完整寫入的語意證明。
- 郵件筆記沿用 DPAPI broker checkpoint 與任務 journal，不新增未加密郵件MD。舊checkpoint缺此欄位時使用空狀態；恢復後仍需本次授權。需求補充會保留舊摘要、清除選定草稿並標示需重新檢視。改寫摘要或反覆切同一階段不能重設無進展額度。
- 量測軸對所有有效系列值計算母體標準差，padding=`clamp(2σ, 0.05×跨度, 0.40×跨度)`，再依跨度選1／2／5刻度向外取整。零保留，空值／略過點不計，離群值不被裁掉；常數以絕對值5%留白，零用−1～1。時間／基準軸不套標準差，長條及面積保留零基準；手動界線優先。畫面、編輯器與PNG共用算法。
- AI工具 `set_chart_reference_lines` 可對現有圖表整組設定最多10條線；X=200、500能呈現在同一散佈圖。原資料及舊PNG保留，編輯器可再修改。多段排版不明時用可選偏好詢問同圖／分圖，並提供其他文字輸入；先處理獨立工作。
- 設定分一般／操作／進階，VNC與診斷移入進階。診斷任務清單改成可見列表，修正晚到回覆覆蓋選取，以及完成Task摘要遮住專案逐輪資料。錯誤有固定代碼與獨立JSON匯出，詳見 [ERROR_CODES.md](ERROR_CODES.md)。DEBUG全文、純token統計、錯誤匯出分開。
- 新請求加入Windows本機日期、星期及ISO週一至日起迄，與任務最初日期分開。使用者指定範圍優先；未知提交仍查原請求，不因跨日改寫。未找到程式固定加七天的證據，不能宣稱已證實公司問題的唯一根因。

## 本機驗證

2026-10-08 17:38（Asia/Taipei）完整發行驗證通過，執行命令為：

```powershell
./scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller
```

- 使用 Rust 1.98.1、MSVC v142 14.29.30133、Windows SDK 10.0.19041.0；實際編譯器探針為 `_MSC_FULL_VER=192930159 x64`。從空 Cargo 快取使用現有 Cargo.lock、vendor 與 `.cargo/config.toml`，所有 Cargo 建置採 `--frozen`。
- fmt、Clippy、284 項單元測試及正式 EXE 編譯通過。新測試涵蓋郵件來源綁定、部分閱讀、版本過期、120 次序列化／恢復、階段工具切換、診斷選取、日期與錯誤匯出隱私。
- 正式 EXE 的 WebView2／原生輸入介面、26 個原生工具流程、9 個上游恢復、12 個圖片案例通過。兩組 68 輪長任務（包含中斷後續接）、DPAPI、AppContainer、PDF、30 個約10 MiB LOG 及舊版續接回歸通過。
- Python 隔離、取消／逾時、Big5 及超過77 MiB LOG、實際 Excel COM → CSV → pandas 通過。Word／Excel／PowerPoint 十種格式修改與重開、新建三種格式、15 張跨檔案／時段圖表、PNG 嵌入與原圖刪除後重開通過。
- localhost SMB UNC 與暫時映射磁碟通過建專案、文件讀寫、DPAPI 重載、路徑逃逸拒絕與原始檔保留；沒有測試公司實際共享磁碟。
- NSIS 正式安裝包實際安裝、自檢及解除安裝通過；更新、啟動交接、失敗回復、使用者檔案保留、Python 原地沿用及遺失檔案／同大小損壞／清單不同三種修復情境均通過。

可追溯報告：[編譯環境](../offline/environment.txt)、[EXE](../offline/exe-verification.json)、[原生介面](../offline/composer-verification.json)、[Python](../offline/python-verification.json)、[Office](../offline/office-verification.json)、[SMB](../offline/network-verification.json)、[NSIS](../offline/installer-verification.json)。完整本機過程保留在 `.build/release-0.8.47-complete.log`，不納入交付。

已通過來源瀏覽器回歸：設定頁籤、鍵盤切換、其他輸入、歷史任務／回合、晚到回覆拒絕、錯誤匯出預設；淺色／深色／520px無溢出。實際ECharts輸出X=200／500兩條線的PNG並目視核對。本機報告 `.build/settings-review.json`、截圖 `.build/settings-*.png` 不納入交付。

`Test-Charts.js` 通過標準差留白、兩個使用者範例、常數／零／離群值、手動範圍及畫面／PNG一致性；`Test-ReplyTables.cjs` 通過一般對話表格回歸。

## 交付產物

| 檔案 | 檔案版本 | Bytes | SHA256 |
| --- | --- | ---: | --- |
| `dist/LM_AI.exe` | 0.8.47.0 | 16,175,616 | `e7befd2baafdeda652152069ed7a310702b5f595a6d29ce4be84a4d54996f95a` |
| `dist/LM_AI_Setup.exe` | 0.8.47.0 | 28,349,030 | `b7a963ffec8fa5f68ac8f87b2417c83d6a22a38d8932c470e0e4c7338f2dfe81` |

兩份 `update-manifest*.json` 均為0.8.47、`kind=nsis`，大小、雜湊及簽章對應同一份 Setup，已通過原生更新清單驗證。Python worker／runtime 指紋與歷史 Rust 離線 ZIP 未改動；請使用完整 NSIS 安裝，不能只移動 EXE。

## 用量與實測界線

維持0.48估算係數及8K–12K軟目標，沒有硬性token截斷。郵件摘要與來源索引也占上下文，選定草稿及必要原文較多時仍可超過目標。動態 mail_note 只在待摘要回合附加小字串欄位，格式說明共用一次，避免每個工具重複整份摘要schema。

兩份使用者回饋共40輪有實際usage，平均輸入10,395 tokens，加權估算高出7.47%；完整分析保留於 [前置設計與用量紀錄](PENDING_SETTINGS_CHARTS_DIAGNOSTICS.md)。這不是同任務前後對照，沒有實測等待時間，不能推算固定加速比例。

合成測試不能證明模型在所有信件都能準確摘要、判斷主導者、選對圖表排版或避免語意重複；仍請在公司長任務核對週報漏項、重複項、日期與數值。沒有使用真實公司郵件或上游模型做本輪效能測試。公司磁碟及真實Outlook COM資料仍需現場驗證。本輪驗證VNC設定頁籤，未重新啟動實際VNC Viewer。
