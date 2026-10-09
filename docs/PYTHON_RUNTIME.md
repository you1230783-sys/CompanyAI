# Python 執行環境與離線重建

## 0.8.48：Python原始碼編輯試用

`python-edit`技能沿用受控文字工具讀取、新建與修訂.py工作副本；`check_python(path,revision)`可檢查專案原文或副本。原文仍由Rust唯讀取得，不把路徑交給檢查器。副本以即將儲存的編碼轉成bytes，固定`check_source.py`經既有AppContainer Python執行器傳入；受檢文字只作資料。

使用標準函式庫的`tokenize.detect_encoding`、`ast.parse`與`compile(..., dont_inherit=True)`。AST建構成功不代表通過所有範圍規則，因此另外編譯但不執行產生的程式物件；例如模組最上層的return也會被拒絕。沒有exec／import受檢模組、.pyc寫入或新的外部套件。編碼宣告解出的內容必須與工作副本文字完全相同。

回傳`syntax_valid`、`source_executed=false`、`functional_tests_run=false`、Python版本、首個錯誤行列、最多5則警告及20個函式／類別索引與總數。檢查通過的文字revision隨工作副本加密保存；`save_copy`對.py要求目前revision已通過，每次內容變動都使舊結果不適用。版本或格式不符、檢查器失敗均不放行儲存。

語法驗證只適用內建CPython 3.13.12，不代表依賴、外部資源、執行結果或其他版本相容。本版沒有pytest／pip，不默默執行整份來源程式。Python worker、套件、runtime清單與指紋不變，安裝仍可通過自檢後沿用舊環境。

整合驗證入口`examples/python_smoke/edit.rs`包含：語法錯誤與修正、編碼宣告、UTF-8 BOM、Big5、未安裝模組、頂層raise不執行、版本檢查失效、拒絕未檢查的發布、成果讀回及原檔不變。實際結果見 [0.8.48驗證](VALIDATION_0_8_48.md)。


## 0.8.45：大檔分段與分析核對

inputs[].log_range 可指定 {start_line:1,line_count:50000}，只適用 kind=text 的 LOG／OUT／ERR／JSONL。原始檔上限 1 GiB，每段最多 2 MiB 完整行；單次其他快照仍限 32 MiB、合計 60 MiB。原生程序核對整份原始 bytes 的版本，回傳 start_line／line_count／next_line／eof；續段必須帶 revision，不混用新舊檔。分段解碼沿用 BOM 優先、無 BOM 的 Big5／UTF-8 選擇，歧義明示，可覆寫 encoding。

Python worker 及 runtime 指紋不變；metadata 自動帶入原始行號與下一段位置。跨段狀態需透過 emit_table 保存，不保留 Python 程序。程式統計取得區間的聯集，分開標示「資料已提供」與「分析結論」。

record_analysis 可引用成功操作的原文／計算快照，並從 result 的 JSON Pointer 核對非負整數筆數。balance 驗證總數等於分項、zero 檢查是否有未配對／重複／反例；不把數量相符當成語意或因果驗證。可重用方法保存為 project 範圍的版本化筆記，新對話依相關性提示，使用前需核對目前樣本。

0.8.44 不變更 Python 套件或環境指紋；相同環境仍可原地沿用。任務內 `run_python` 的連續失敗獨立計算，上限10次，其餘工具共同上限5次；同類成功才歸零。已辨識原生工具的非嚴格參數錯誤也納入，已知失敗不另外增加8次無進展計數。停止保留 checkpoint，使用者按「繼續」後重設；未知提交、嚴格 Schema／權限／身分拒絕不放寬。見 [0.8.44 驗證](VALIDATION_0_8_44.md)。

0.8.42 將 Python 放在 EXE 旁的 `python/`，不嵌入 Rust EXE、不查詢 PATH／登錄／使用者 site-packages。NSIS 安裝與修復會驗證 runtime 清單、設定該目錄的 AppContainer 唯讀執行權，再以真正隔離程序驗證 pandas 和 XLSX 生成；成功後才完成 EXE 替換。

## 0.8.43：相同環境直接沿用

安裝器先將新版 EXE 放到安裝目錄的 `LM_AI.pending.exe`，由它執行 `--python-self-check`。這個入口核對編入新版 EXE 的環境指紋、完整檔案集合、各檔大小與 SHA256，並在 AppContainer 載入固定版本套件、測試分析及 XLSX 產生。通過就直接沿用現有 `python/`，跳過解壓、目錄切換及權限重設；主程式替換失敗時也不移動該環境。

首次安裝、缺檔、內容損壞、清單外檔案、環境版本或執行測試不符，均改走既有完整暫存、驗證、切換與失敗回復流程。不逐檔混補不同套件版本，不查系統 Python。未知檔案仍保留，不遞迴刪除。

主程式進版本身不改變 Python 指紋。目前 `lm_worker.py` 仍屬固定環境的一部分；修改它或依賴清單會完整替換環境。這次最佳化減少解壓後的檔案寫入與清理，安裝包仍攜帶完整離線環境，下載大小不因此縮小；完整比對仍會讀取環境檔案。

未發行的安裝器回歸可用 `scripts/Test-Installer.ps1 -ValidateOnly -AppSource <已驗證的 EXE>`；須先載入專案 v142 環境。測試包與結果留在 `.build/installer-test`，不改正式 `dist`、簽署清單及 `offline` 發行驗證紀錄。涵蓋原地沿用、沿用後主程式替換失敗、缺檔、同大小損壞與清單指紋不同的修復。

## 0.8.43：LOG 編碼與呼叫修正

Python 文字快照在 Rust 端解碼，不把原始路徑交給模型程式。`kind=text` 的 LOG／OUT／ERR 無 BOM 時先嚴格嘗試 Big5（Windows CP950、無損往返），失敗再嘗試 UTF-8；UTF-8 BOM 優先遵循，BOM 與內容不符即拒絕。TXT／JSON／JSONL 預設 UTF-8，CSV／資料集維持原契約。UTF-16 仍導回既有 LOG 工具或文字匯入，不誤當 Big5。

`inputs[].encoding` 可省略／null／auto，或明確指定 big5、utf8（僅 kind=text）。Python 取得 Unicode 的 `texts[name]`，`metadata[name].encoding` 為實際編碼；兩種都有效但文字不同時，auto 採 Big5 並以 `encoding_ambiguous=true` 提醒模型核對少量中文樣本，必要時指定 UTF-8 重讀。來源版本仍以原始 bytes 計算，不替換無效字元、不改原檔、不套用一般文字文件的 200 KB 上限。

工具參數拒絕時最多回傳6個欄位位置及原因，例如 `$.inputs[2].kind`，不回送完整 code／欄位值，也不猜參數後執行。技能提供完整 LOG 呼叫範例，明確區分 `name` 與 `path`。worker 腳本及套件沒有改動，因此 0.8.42 完整 Python 環境可通過新版自檢後沿用。

## 固定依賴

CPython 3.13.12 x64（官方 embeddable）、pandas 2.2.3、NumPy 2.2.6、openpyxl 3.1.5，以及 python-dateutil 2.9.0.post0、six 1.17.0、pytz 2025.2、tzdata 2025.2、et-xmlfile 2.0.0。沒有 pip、pywin32、PowerShell、matplotlib；畫圖仍用現有離線圖表工具。上游授權留在 runtime 的 LICENSE／dist-info 目錄。

`offline/python-lock.json` 記錄來源、版本、輸入檔案與 `python-inputs.zip` 的 SHA256。這些是上游預編譯發行品；CPython `sys.version` 為 MSC 1944，不能稱為本專案以 v142 編譯。Rust 主程式及 C++ 測試仍由固定 v142 x64 建置；Python 相容性另以安裝後實測核對。

## 資料流

原始 Excel → Rust Excel COM（唯讀、固定操作）→ 追蹤 CSV → Rust 驗證快照 → Python／pandas → 精簡摘要與 CSV／XLSX 成果 → Rust 驗證並保存 `_AI_Output`。

一般 CSV／文字直接由 Rust 唯讀取入；`_AI_Output` 內生成 XLSX 可交給 openpyxl。模型取得來源與結論，不會因呼叫 Python 自動取得全部檔案內容。統計衍生列會明示為衍生資料，不冒充原始列；原始路徑、版本、Excel 欄位及逐列型別／顯示值在輸入 metadata 中保留。

Python 每次建立新的 AppContainer 與 Job，沒有網路 capability，單一程序、1GiB、120秒回應期限。完成、取消或逾時均終止 Job；不保留前次 Python 變數。只有 IPC 管道被繼承，父程序環境採固定白名單。資料範圍仍由 Rust broker 驗證；實際隔離是 Windows OS，不把 Python 的 exec 字典當成安全沙箱。

## 重建

1. 保留 Cargo.lock、vendor、.cargo/config.toml，安裝 v142 x64／指定 SDK。
2. `scripts/Prepare-Python.ps1` 從固定離線包建立全新的 `.build/python-runtime`。可用 `-Destination` 指定另一個新目錄；不覆寫舊目錄。
3. 每個重建檔案必須符合 `assets/python-runtime-manifest.json` 的大小與 SHA256，驗證後才複製固定清單；其雜湊必須與 `assets/python-runtime-sha256.txt` 相同。這避免 PowerShell 5.1／7 文化排序不同導致相同內容產生不同清單。修改 worker／依賴時，維護者在新目錄執行 `Prepare-Python.ps1 -Destination <新目錄> -RefreshManifest`，檢查兩份固定資源的差異後再編譯；一般建置不更新信任基準。worker 原始碼使用 LF，確保 checkout 後一致。
4. 更新環境時，將原 `.build/python-runtime` 更名保留，再把已核對的新目錄更名為 `.build/python-runtime`。`scripts/Stage-Python.ps1` 複製至指定 EXE 目錄；清單外、缺少或被修改的檔案均由原生啟動檢查拒絕。
5. 執行 `scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot <測試SMB根目錄> -IncludeInstaller`。建置不下載套件，使用者端也不执行 pip 或 PS。

獨立 `LM_AI.exe` 保留供對應原始碼核對；本版實際發行與兩個更新清單都使用完整 NSIS。歷史 `CompanyAI-offline.zip` 未重製，Python 輸入包不代表整個新版 Rust 離線工具鏈包。
