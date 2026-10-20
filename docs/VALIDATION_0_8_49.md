# 0.8.49：獨立工作並行、持續草稿與實測修正

## 行為與使用方式

- `run_batch`一次2–4項獨立工作，同時最多2項。Python各用獨立AppContainer／Job，文字讀取各用獨立原生Worker，CSV圖表準備在不同工作執行緒進行；父broker依序驗證與保存成果。支援read_file、read_code_section、find_text、inspect_dataset、chart_dataset與run_python，仍需相應技能。Office／Outlook／PDF與同一副本修改不並行，不接受巢狀或依賴同批未生成成果。每個子項有自己的成功／失敗、操作ID與原始紀錄，已成功的整批重播使用原結果。
- Python新增行號／區段雜湊編輯，每次最多200行／6000字；不用在修改參數中重送長段舊文。每次修改成功即flush、讀回並更新同一份.py輸出草稿，原件唯讀；未通過語法也保留。續接核對草稿磁碟雜湊與副本版本，外部修改不覆蓋。check_python通過後save_copy將同一路徑標為完成，沒有每改一段就新增_1、_2。不同任務或檔名衝突仍保留原有不覆蓋規則。原地更新不宣稱具有斷電原子性。
- 隱藏／未勾選資料夾的郵件副本，從其他資料夾的標題、內文與匯出入口排除。每次Outlook查詢以本機Message-ID索引為主，任一端缺少ID才使用完整寄收件地址／時間／主旨的保守備援；不同且存在的Message-ID不因相同信封誤判。隱藏索引不讀Body或附件、不交AI。掃描有100資料檔、5000資料夾、十萬項目與120秒界線，不能完整核對時拒絕該批資料，不宣稱部分掃描等同完整隱藏。
- 讀內文後下一操作須附來源綁定摘要；include／uncertain須有可帶入句子。成功接受即更新同一份「郵件整理草稿.txt」，含來源、判斷、摘要、草稿、完整性與疑點。後續自動帶未寫入草稿，四份以上提示先寫週報／工作日誌，避免只閱讀不產出。模型仍需實際執行寫入；草稿不是完成的週報。
- 常見點陣格式JPG／JPEG／PNG／BMP／TIF／TIFF／GIF先用本機WIC轉JPEG，品質85%、透明處白底、保留EXIF視覺方向。原檔不變；原始檔最多64 MB、8192×8192內／1600萬像素。轉檔後每張最多5,000,000 bytes、雙圖合計8,000,000 bytes；每任務累計100張（雙圖計兩張，已完成快取不重扣）。TIFF多頁／動畫只讀第一頁／幀並回報範圍。父上下文只留文字。
- Base64後完整請求仍受網站公告與桌面12 MB較小值限制，8 MB圖片並不保證可通過仍公告10 MB的網站。fast模型、一般聊天／Outlook附件范围不變。未知圖片提交只查原ID，來源／目的／身分一致的舊v1待查仍可接續。
- 自訂文字在一般及排版模式可雙擊進入文字對話框，刪除也在對話框內；保留取消與自動避讓。VNC分類增加自然排序，機台1、2、10，其他分類／密碼／擴充欄位保留。
- 活動紀錄的資料屬性與通知ID分開，點擊歷史或時間不再誤送通知讀取。手動停止顯示正常停止與已保存草稿提示，不包装成Error Code 99。

## 驗證

2026-10-09 11:54（Asia/Taipei）完整發行流程通過，執行命令：

```powershell
./scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller
```

- Rust 1.98.1、MSVC v142 14.29.30133、Windows SDK 10.0.19041.0；實際編譯器探針為`_MSC_FULL_VER=192930159 x64`。空Cargo快取使用專案Cargo.lock、vendor及`.cargo/config.toml`，建置採`--frozen`。
- fmt、Clippy、297項單元測試及release EXE通過。新增驗證涵蓋隱藏資料夾A有10封、與B的50封重複8封後只放行42封；不同Message-ID不誤判、缺ID備援、未讀Body、取消與祖先限制。
- 正式EXE的WebView2自檢、AppContainer檔案／網路隔離、26個原生代理案例、9個上游恢復案例、13個圖片案例與兩組68輪長任務通過。另含30份約10 MiB LOG、舊協定、工具重播、中斷續接、加密記憶及PDF。
- 圖片整合實際核對轉檔後JPEG位元組、雙圖請求、50次雙圖共100張後拒絕新增、已完成快取與未知提交不重送。WIC單元測試涵蓋PNG／JPEG／BMP／TIFF／GIF、方向及透明處處理；大小界線以轉檔後資料核對。
- 真正啟動兩個獨立Python AppContainer，各執行約8秒，實測重疊約8.00秒。兩份獨立CSV、重播不重複發布、CSV作圖與下一檔讀取、失敗Python與成功Python並存均通過。這是並行成立的證據，不是公司任務的端到端加速比例。
- Python分段編輯在語法尚未完成時已寫入草稿，後續修改、續接及完成發布沿用同一路徑；外部修改拒絕覆寫，已完成版本再次修改會退回草稿。AST／compile、編碼、原文不執行、原件保留及成果读回通過。
- Python分析、超過77 MiB LOG、實際Excel COM → CSV → pandas、檔案／網路／子程序隔離、取消、真正120秒逾時與runtime損壞拒絕通過。
- Word／Excel／PowerPoint十種既有格式修改及重開、新建三種格式、三份Excel乘五時段共15張圖表、10000點資料、PNG嵌入與移除原PNG後重開通過。
- localhost SMB的UNC及映射磁碟通過：獨立資料夾、文字／Office讀寫、加密筆記重載、原件保留及路徑逃逸拒絕。
- 完整NSIS實際安裝、自檢、更新、PID交接重啟、失敗回復、保留使用者檔案與解除安裝通過。Python原地沿用，以及缺檔、同大小損壞、清單不符三種完整修復均通過。
- `Test-Charts.js`及`Test-ChartLayout.cjs`通過。實際headless Edge驗證一般／排版模式雙擊文字、對話框刪除及取消、拖曳／鍵盤／PNG、活動紀錄與時間點擊不誤送通知命令、VNC單分類排序及版本參數；已目視核對文字PNG。

可追溯報告：[環境](../offline/environment.txt)、[EXE](../offline/exe-verification.json)、[原生介面](../offline/composer-verification.json)、[Python](../offline/python-verification.json)、[並行明細](../offline/batch-verification.json)、[Office](../offline/office-verification.json)、[SMB](../offline/network-verification.json)、[NSIS](../offline/installer-verification.json)。完整本機日誌為`.build/release-0.8.49-complete.log`；瀏覽器報告為`.build/chart-layout-review.json`，不納入安裝包。

## 交付產物

| 檔案 | 版本 | Bytes | SHA256 |
| --- | --- | ---: | --- |
| `dist/LM_AI.exe` | 0.8.49.0 | 16,714,752 | `08d5794e66d6bad8bdd15dc110482f563b59de5f839f3173bbbd1331efd30174` |
| `dist/LM_AI_Setup.exe` | 0.8.49.0 | 28,491,409 | `caa3c7b232c760820c84fd76af6a2970a11882cf268c29a5fbe19310b8f2a962` |

兩份`update-manifest*.json`完全相同，均為0.8.49、`kind=nsis`，大小、SHA256與簽章對應上述Setup。原始碼、產物及本文件一起提交至既有Git main；Python worker／runtime指紋與歷史Rust離線ZIP未改動。更新請使用完整NSIS。

## 實測界線

Outlook副本測試使用記憶體COM fixture，圖片／長任務採loopback固定模型回覆；真實公司信箱、模型能否持續採用小段編輯與並行、速度及tokens改善仍需公司實測。Python語法檢查不執行原始碼，不宣稱相依套件或功能通過。UI原始碼瀏覽器測試與正式EXE／NSIS測試分開紀錄。

實作參考：[Microsoft WIC](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-about-windows-imaging-codec)、[WIC編碼](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-creating-encoder)。
