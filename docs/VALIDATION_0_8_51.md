# 0.8.51：Outlook 日期批次與程式工作資料

## 行為與界線

隱藏副本的識別只使用完整主旨與 SentOn 寄送時間精確到秒，JSON 二元組取 SHA256，避免分隔字元歧義。這是雜湊而非加密；使用者接受兩欄完全相同的不同郵件也被排除。不去除 RE/FW、大小寫或主旨空白，不讀 Message-ID、Recipients 物件、地址簿、隱藏正文或附件。

專案標題查詢沿用起訖日（含結束當日，單次1–93日）。可見收件按 ReceivedTime，寄件按 SentOn 篩選；隱藏索引依這批候選的 SentOn 日期範圍建立，所以月底寄出、次月收到的副本仍可排除。Folder.GetTable 優先日期篩選，Table.GetArray 每次最多100列；不支援篩選時依時間降冪取批次、本機過濾，到起始日之前停止。直接日期篩選已能略過封存舊信，不另外要求逐封探查前五封。

依 Microsoft 文件核對 [GetArray 的欄／列維度](https://learn.microsoft.com/en-us/office/vba/api/outlook.table.getarray)及[內建屬性名稱對應本機時間](https://learn.microsoft.com/en-us/office/client-developer/outlook/pia/how-to-filter-and-efficiently-enumerate-items-in-a-folder)，此處不混用以命名空間讀取的 UTC 欄位。

隱藏資料只批次讀取識別欄位與必要版本資訊。Table 可能截短長文字，主旨達120個UTF-16單位時定向讀回完整 Subject；這項必要例外可能增加長主旨信件的 COM 呼叫。可見寄件者／To／CC 只取顯示文字，不解析每位收件者的伺服器個人資料。缺必要欄位、日期排序不穩、掃描中筆數改變、超過界線時拒絕本次結果，不回報零封。

每次來源查詢重新批次核對日期範圍內實際列，再決定能否重用隱藏鍵集合；不是僅依筆數或時間快取，因此不保證省掉每次 COM 掃描，也不保證郵件變更的即時通知。單次最多可見4000封；另有5000資料夾、20萬掃描列及10分鐘期限。超出時請縮小範圍，不默默交付部分資料。

專案 Outlook COM 在獨立本機輔助程序內執行，所有通訊走匿名管線，沒有未篩選郵件明文暫存檔。每次工具操作共用10分鐘期限；取消或逾時終止並回收輔助程序，不關閉 Outlook。狀態顯示階段、已掃描數量與可確定的單資料夾百分比，不捏造整體百分比；完成隱藏排除後才把可見結果交給模型。舊版「目前選取／最近幾日」介面依候選寄送日延後建立索引；獨立程序的10分鐘期限目前用於專案工具。

程式修改是通用流程，沒有對下載器範例加特殊分支：

- `section_id` 綁定實際 copy_id、revision、行號與原文；修改時只提交代號與新文，避免大段 hash 配小段行號。舊介面保留但不能混用。
- `read_code_section` 超過檔尾自動裁切，超過200行／6000字按完整行分頁，回報實際範圍與下一行。草稿路徑是同一工作副本的別名；原件仍明示唯讀，不自動改指向副本。
- 每次小段修改立即儲存同一草稿。最近必要原文與真實版本、語法／測試／發布狀態置於獨立工作資料，最多兩份副本、原文合計10000字；相同原文在工具投影去重，完整歷史仍保存在既有加密紀錄。
- 原文未變且唯一可定位的區段可更新版本與位置，已變更或有歧義的區段失效；新區段代號隨目前資料交給模型。過期編輯不得靜默套用。
- 最小 unittest/load_target 介面固定帶入，要求先做一組符合使用者需求的測試，再補必要案例，不反覆重讀指南。相同 revision 已通過的語法檢查可重用；修改後語法、測試、核對證據照常失效。
- 同一 run_batch 的重複讀取最多記一次無進展，子項資料與失敗仍分別記錄。四輪提醒／八輪停止保留，不以筆記或批次數量假造進展。

## 驗證狀態

2026-10-09 17:42（Asia/Taipei）完整發行驗證通過。

- Rust 1.98.1、MSVC v142 14.29.30133、SDK 10.0.19041.0；實際 `_MSC_FULL_VER=192930159 x64`。Cargo.lock、vendor、專案 `.cargo/config.toml` 搭配空 Cargo 快取與 `--frozen`。
- fmt、Clippy（警告視為錯誤）、305項單元測試、release EXE 與 WebView2 自檢通過。
- Outlook COM fixture 驗證隱藏副本50→42、80→70、120→110；只有主旨與寄送時間也能比對且保留31封其他主題。缺必要欄位明示失敗；500封跨月資料按100列批次與無伺服器篩選的排序備援皆通過；同筆數但內容改變也重新核對。
- 真實子程序的卡住／取消／逾時案例確認會終止並回收；正式 EXE 的 Outlook 輔助程序收到無效要求能回覆並於EOF退出，該案例不接觸信箱。
- 區段代號編輯、過期代號拒絕、草稿路徑別名、檔尾裁切、原文／原生狀態續接與同版本語法結果重用通過。批次四個重複讀取僅計一輪，八輪仍暫停。
- 真實 Python 副本的語法合法預設值錯誤被功能測試發現，小段修正後通過；假證據、未載入副本、沒有案例不能算通過，缺依賴標為不可驗證。私有工作區、原件／網路／程序隔離、雙程序重疊、取消、120秒逾時與Office來源分析通過。
- 26個原生代理案例、9個恢復案例、13個圖片案例、兩組68輪長任務、PDF與加密記憶／續接通過；這些是本機合成案例，不代表公司模型已驗收。
- Word／Excel／PowerPoint十種既有格式、新建三種格式、15個Excel規劃圖表、PNG嵌入並移除外部PNG後重開通過。localhost SMB 的 UNC／映射磁碟通過，不冒稱公司磁碟已測試。
- 完整NSIS的安裝、更新、PID交接重啟、失敗回復、使用者資料保留及解除安裝通過；Python原地沿用、缺檔／同大小損壞／清單不同的完整修復通過。

報告：[EXE](../offline/exe-verification.json)、[程式功能](../offline/code-verification.json)、[Python](../offline/python-verification.json)、[並行](../offline/batch-verification.json)、[Office](../offline/office-verification.json)、[SMB](../offline/network-verification.json)、[NSIS](../offline/installer-verification.json)、[環境](../offline/environment.txt)。本機完整日誌：`.build/release-0.8.51-complete.log`。

```powershell
./scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller
```

## 交付產物

| 檔案 | 版本 | Bytes | SHA256 |
| --- | --- | ---: | --- |
| `dist/LM_AI.exe` | 0.8.51.0 | 17,485,824 | `44f888cf7b1ed41d1c2c9fbdf704f69071f44d10437ad1255ecec4de72c5cd8b` |
| `dist/LM_AI_Setup.exe` | 0.8.51.0 | 28,626,091 | `8d9307f86774ef7ae5e524c77f2f9ecac452a9672b49360a2a986d53539db226` |

兩份更新清單完全相同，指向上述完整NSIS。Python worker／runtime指紋與歷史Rust離線ZIP未改動。


## 公司實測

本機 COM fixture、真實隔離 Python 執行與合成代理測試不能代替公司 Outlook／伺服器或模型長任務。請觀察月份範圍、隱藏副本數、掃描時間、是否真正開始修改與提交測試；不以語法通過宣稱功能完整，也不承諾模型不會再重讀。
