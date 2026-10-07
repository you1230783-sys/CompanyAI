# Python 執行環境與離線重建

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
