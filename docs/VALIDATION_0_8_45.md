# 0.8.45：長任務恢復、分析依據與本機專案

## 修改範圍

- 移除圖片辨識快捷按鈕、選圖視窗與原生命令；image-read不再列入技能目錄／工具參數。analyze_image改為基本閱讀工具，普通檔案清單→metadata→辨識可直接完成，不需要載入技能。fast仍不公告／執行圖片工具；來源、20次／5MB額度、快取與未知提交保護保留。舊checkpoint的圖片技能名稱只保留空相容別名，避免舊載入操作破壞續接。
- 舊版工具協定把圖片說明放入獨立的既有技能／指示訊息，避免已含完整工具結構的首則system超過64KB；未提高訊息上限，也未刪減工具結構。原生協定仍按模型能力提供基本圖片閱讀。
- 預設以 Windows `FOLDERID_Profile` 找到目前個人資料夾，確認為本機固定磁碟，建立 `LM_AI_Projects/LM_AI專案資料夾_YYYYMMDD`；同日加序號，不覆寫資料。移除桌面／下載入口，仍可自行選擇本機、UNC 或映射磁碟。映射路徑先轉為目前工作階段的 UNC，再核對實際位置；重複相同錯誤也會再次通知。
- 專案主推論、快速摘要及圖片子請求共用10／30／60／180／300秒延後恢復。僅已核對身分、確認 failed 且沒有 result 的限定上游錯誤可以新ID重試這輪推論；舊網站的 AI_BACKEND_ERROR 即使標 retryable=false 也納入。保留原messages／tools／parent，遞增turn_index；未知結果始終只查原ID。明確的權限、額度、格式或結果未知錯誤不能因此另建推論。詳見 [契約補充](DESKTOP_AGENT_V1_CONTRACT.md#0845-桌面有限恢復補充)。
- 倒數與次數先加密保存，再等待或提交；五次重試耗盡後暫停，按「繼續」可恢復。自動重試不扣Python10次／其他工具5次，也不扣無進展8次。停止按鈕仍取消任務；POST明確未受理仍使用既有拒絕處理，未新增通用POST重送。
- 無進展依實際新資料與處理區間判定。Outlook記錄郵件版本、查詢與標題分頁，十頁可逐頁推進；同內容換操作ID、重寫筆記或只改Python程式不會變成進展。四次提醒原因、八次保存暫停；上下文縮減與續接保留閱讀範圍。標題已取得不等於內文已讀。
- 新增分析概況、可展開工具證據、工具結果筆數／反例核對、目標與失效結論，以及同專案方法記憶。核對程式只證明工具資料中的關係，不能證明AI解析或推論本身正確。完整工具結果保留原有讀取授權。
- LOG 原始檔最大1GiB，Python可用完整行分段快照，每段最多2MiB，保留原始行號、版本、編碼與next_line／eof。Python worker、套件及清單未改，相同完整環境更新可沿用。

## 發行驗證

2026-10-07 已完成正式流程：`scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller`，結束碼為0。本機完整日誌為 `.build/release-0.8.45.log`；機器可讀報告位於 `offline/`，最終紀錄時間為17:31（UTC+8）。

實際工具鏈為Rust 1.98.1、MSVC 14.29.30133、SDK 10.0.19041.0；x64 C探針確認 `_MSC_FULL_VER=192930159`。使用新建空Cargo快取、Cargo.lock／vendor／.cargo/config.toml與`--frozen`，通過fmt、Clippy、263項單元測試及release建置。

- 原生HTTP共25個案例、下方9個恢復案例、12個圖片案例、舊版工具／技能協定、兩組68輪長任務續接、DPAPI中斷與未知請求查回全部通過。圖片基本工具流程不需載入技能；實際WebView2自檢確認圖片快捷UI及原生命令移除、Outlook入口保留。
- 實際Python AppContainer重跑Big5分段、81,920,000 bytes合成LOG、原始行號／版本、證據與數量核對、方法去重及跨對話查回。CSV／XLSX往返、Excel COM→CSV→pandas、OS隔離、取消及120秒逾時通過。
- 真實Office驗證涵蓋Word／Excel／PowerPoint副本及新建成果、三份不同欄序Excel×五時段共15圖、XLSX／XLS各10000點、PNG嵌入／儲存／重開。30份約10MiB LOG、PDF、UNC與暫時映射磁碟讀寫及路徑邊界回歸均通過。
- NSIS實際安裝、更新、等待舊程序、重啟、失敗回復與解除安裝通過，包含本次正式Setup的安裝與WebView2／Python自檢。Python檔案鎖及建立時間確認完整環境未被搬移或覆蓋；缺檔、同大小損壞及不同清單均完成修復。主程式更新失敗時，已沿用的Python仍保留。
- 兩份簽署更新清單均為0.8.45、kind=nsis，長度與SHA256符合實際Setup；原生更新器已驗證簽章。交付EXE及Setup均為0.8.45.0。

| 交付項目 | 大小／SHA256 |
| --- | --- |
| LM_AI.exe | 15,325,696 bytes；`fadf6b3eafdaae30443d7c25bd048e517c0493b632b3ebafc10885d3bcdc7d85` |
| LM_AI_Setup.exe | 28,167,380 bytes（約28.2 MB）；`fa8c0f3db30fd732b9bc19df23e1832229dd99b554b6bf764cd31c468f8394e0` |

新增九個HTTP整合案例涵蓋兩次暫時失敗後成功、初次加五次全數失敗後手動續接、永久額度拒絕、等待途中重啟、POST回覆遺失且查詢404、等待取消、未知上游結果拒絕、查詢401保存但不自動重試，以及倒數停止時取消仍在排隊的原任務。成功案例檢查副本修改與儲存各只執行一次，所有案例均確認原檔未變；測試時縮短等待，正式排程秒數另由單元測試固定核對。release沒有測試加速入口。

先前分析功能另已實跑Big5分段及81,920,000 bytes合成LOG、工具證據與數量檢查、方法去重及跨對話查回；亮／暗／窄版畫面亦已檢查，前置日誌為 `.build/analysis-validation-final.log`。正式0.8.45仍重新執行完整原生測試，不以先前EXE替代新版本驗證。

## 實測界線

本機HTTP、Office及SMB測試使用合成資料，尚不能代表公司F槽權限環境、實際模型採用新工具、網路故障的真實回傳碼或使用者72個LOG的分析品質已驗收。未變更公司網站；網站若以其他錯誤碼表示故障，需依實際紀錄再核對。

交付EXE、完整NSIS與兩份指向NSIS的簽署清單，不重製歷史Rust離線ZIP，不修改icon.png。Python runtime清單指紋仍為 `ba14595d7e9e75268da361f4bc09a75f07388cfac14d985c137e787e344f811c`。
