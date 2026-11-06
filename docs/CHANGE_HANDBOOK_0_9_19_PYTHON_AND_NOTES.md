# 修改手冊：0.9.19 Python 執行加速與版本公告整理

撰寫日期：2026-11-06。本手冊與其他 CHANGE_HANDBOOK 分開，**請勿改寫任何一份手冊**；進度與驗證結果記在 IMPLEMENTATION_PLAN.md 與 docs/validation/VALIDATION_0_9_19.md。

行號以提交 `52b5132` 為準，僅供定位，請以函式名稱為主。本版包含兩部分，請分開 commit：

- **第一部分（P1～P5）**：Python 執行與安裝加速。
- **第二部分（N1）**：開啟程式時顯示的版本更新資訊整理。

不變條件照舊：安裝路徑 `C:\largan\LM_AI`、公司 HTTP、更新同源與簽章、machines.json、原件唯讀、不重播成功寫入、不給 AI 刪檔能力。**Python runtime 的完整性驗證不得降低**（P2 只是避免同一次執行期間重複驗證）。

---

# 第一部分：Python 執行與安裝加速

## 0. 背景與現況（Claude 檢查結果）

- `dist/python`：約 272 MB、6,696 個檔案，**沒有任何 `.pyc`**。
- Python worker 以 `-I -B -u` 啟動（`src/projects/sandbox.rs` 約 217 行），`-B` 表示不寫出 `.pyc`；且 AppContainer 對 runtime 只有唯讀權限（`src/projects/python.rs` 的 `prepare_runtime`）。
- `src/projects/python_worker.py` 的 `analyze`（約 52–60 行）每次都 import pandas、numpy、openpyxl、scipy、matplotlib（含 pyplot）。每次 `run_python` 都是新的 worker 程序，因此**每次都要從 `.py` 原始碼重新編譯這些套件**。EIS 實測中每次 Python 工具耗時 34～41 秒，但計算本身很輕，推測主要耗在這裡（尚未量測確認）。
- `src/projects/python.rs` 的 `execute`（約 364 行）每次呼叫 `verified_runtime`（約 81 行）：開啟並鎖住全部檔案、逐一計算 SHA256（約 272 MB）。`src/projects/python/concurrency.rs` 約 8 行、`python.rs` 約 388 行也會呼叫。
- 安裝程式（`installer/LM_AI.nsi` 約 152–270 行）：Python 不符合時解壓到 `python.pending`，改名切換，再於約 238 行**同步**刪除 `python.previous`。刪除巨集 `PythonRemove`（由 `scripts/Build-Installer.ps1` 產生 `.build/python-files.nsh`）是逐檔 `Delete`（約 6,700 行）加約 540 個 `RMDir`。安裝程式沒有使用 `SetDetailsPrint`，每個檔案都會顯示在畫面上。
- numpy、pandas、scipy、matplotlib 的 `tests` 資料夾約 2,400 個檔案、約 40 MB，執行時用不到。
- runtime 清單由 `scripts/Stage-Python.ps1`／`scripts/Prepare-Python.ps1` 產生（`runtime-manifest.json`），版本核對另有 `scripts/Test-PythonRevision.ps1`。

## 1. 先量測基準（必做，實作前）

在同一台電腦、同一個安裝版本記錄下列時間，作為改善前後的比較：

1. 安裝時「Python 自檢」耗時（Python 未變更、直接沿用的情況）。
2. 安裝時 Python 版本變更的整體耗時，分別記錄：解壓、切換、自檢、刪除舊版。
3. 一次最簡單的 `run_python`（例如 `result={"x":1}`）的總耗時，並拆分為：`verified_runtime`、worker 啟動、import、執行。
4. 連續執行第二次 `run_python` 的耗時。

每項改完後重量同樣項目，記錄在驗證文件。不得只寫「明顯加快」而沒有數字。

## P1. 建置時預先編譯 `.pyc`

- **目標**：Python 執行時直接讀取預先編譯的 bytecode，不再每次重新編譯 pandas、scipy、matplotlib 等套件。
- **做法**：
  1. 在 runtime 建置流程中（`Stage-Python.ps1` 或 `Prepare-Python.ps1`，產生清單之前）以**同一個 runtime 的 python.exe** 執行 `python -m compileall -q --invalidation-mode unchecked-hash -j 0 <Lib 路徑>`。
  2. 產生的 `__pycache__\*.pyc` 必須納入 `runtime-manifest.json` 與雜湊驗證，與其他檔案同等處理。
  3. **不要移除** worker 的 `-B` 參數（`-B` 只禁止寫入，仍會讀取已存在的 `.pyc`），也不放寬 AppContainer 唯讀權限。
- **為何用 unchecked-hash**：安裝會改變檔案時間，時間戳記式 `.pyc` 會被判定過期而失效；unchecked-hash 不依賴時間，也不重新雜湊原始碼。原始碼與 `.pyc` 是否被竄改，由既有的清單雜湊驗證負責。
- **注意**：
  - `.pyc` 依 Python 版本而定，runtime 固定 3.13.12，必須以該版本產生。
  - 產生結果必須可重現：同樣的輸入產生同樣的 runtime 清單雜湊（如有差異，記錄原因）。
  - 若 compileall 對個別檔案失敗（例如套件內故意放置的語法測試檔），記錄並說明，不可整體忽略錯誤。
- **驗收**：runtime 內 `.pyc` 數量與清單一致；P1 前後第 1 節第 3、4 項的耗時比較；`self_check` 與 Python 相關原生測試通過。

## P2. 同一次執行期間沿用 runtime 驗證結果

- **目標**：每次 `run_python` 不再重新雜湊整個 runtime；安全性不變。
- **做法**：
  1. 第一次 `verified_runtime` 成功後，把它鎖住檔案的 handle（目前回傳的 `Vec<File>` guards）保存在**程序層級**的共享狀態中（例如 `OnceLock`／`Mutex`），保留到程式結束。檔案在鎖定期間無法被修改，因此後續呼叫只需確認共享狀態存在且屬於同一個 runtime 根目錄，不必重新雜湊。
  2. 若第一次驗證失敗，不保存，下次照常重新驗證；不可把失敗結果快取成成功。
  3. 安裝程式與 `--prepare-python-runtime`、`--python-self-check` 等獨立程序入口照舊完整驗證（它們本來就是新程序）。
  4. 更新安裝時，舊程式必須先退出（現有 PID 交接），不會出現程式持有舊 runtime 鎖而阻擋安裝的情況；請確認並在測試中涵蓋。
- **注意**：鎖住的檔案共用模式要與現在相同，確保 Python worker 仍可讀取；確認不影響兩個 Python 工作程序同時執行（M11）。
- **驗收**：第一次 `run_python` 照常驗證，第二次起不再雜湊（以耗時與測試確認）；runtime 檔案在程式執行中被修改的嘗試仍會失敗；更新安裝流程正常。

## P3. 移除 runtime 中的測試資料夾

- **目標**：減少約三分之一的檔案數，加快安裝、驗證與刪除。
- **做法**：在 runtime 建置流程中刪除 `Lib\site-packages` 下各套件名為 `tests` 的目錄（例如 `numpy\**\tests`、`pandas\tests`、`scipy\**\tests`、`matplotlib\tests`），之後再執行 P1 與產生清單。
- **必須保留**：`numpy\testing`、`pandas\_testing`、`scipy\_lib` 等其他套件會 import 的模組。**只刪名稱完全為 `tests` 的目錄**，並在建置腳本中列出實際刪除的目錄清單與檔案數。
- **驗收**：
  1. 以 runtime 的 python.exe 實際 import 下列模組並執行簡單操作：`pandas`、`numpy`、`scipy.optimize`、`scipy.signal`、`scipy.stats`、`scipy.interpolate`、`matplotlib.pyplot`（Agg）、`openpyxl`。
  2. 既有 Python 原生測試（含 EIS 類型的 NNLS、find_peaks 計算）通過。
  3. 記錄刪除前後的檔案數與大小。

## P4. 安裝時不逐檔顯示

- **目標**：減少安裝畫面逐檔輸出的負擔。
- **做法**：在 `installer/LM_AI.nsi` 中，Python 大量解壓（`File /r "..\dist\python\*.*"`，約 173 行）與每一處 `PythonRemove` 前加 `SetDetailsPrint none`（或 `textonly`），完成後恢復 `SetDetailsPrint both`，並以 `DetailPrint` 顯示一行摘要（例如「正在安裝 Python 分析環境…」、「已完成」）。
- **注意**：錯誤與失敗回復的訊息仍要顯示；不可因關閉逐檔顯示而隱藏失敗原因。
- **驗收**：安裝過程畫面只顯示摘要；失敗回復情境仍能看到錯誤訊息。

## P5. 舊版 Python 延後刪除

- **目標**：安裝時不必等待刪除數千個舊檔案。
- **做法**：
  1. 安裝成功提交後（現在約 238 行刪除 `python.previous` 的位置），改為把 `python.previous` **改名**為 `python.trash-<隨機識別>`（改名是單一操作，幾乎不耗時）。
  2. 由 LM_AI 在下次啟動後於背景清理 `python.trash-*`。**比照 0.8.79 清理舊 CAD 環境的做法**（`src/cad/cleanup.rs`、`--cleanup-old-cad-runtimes`）：
     - 只清理名稱符合 `python.trash-*`、位於安裝目錄下的普通目錄。
     - 先鎖住目錄與檔案，依該目錄內的 `runtime-manifest.json` 核對內容；拒絕連結、占用、唯讀、清單外的額外檔案或被修改的檔案。
     - 以已驗證的 handle 刪除，**不遞迴掃描刪除**；清單最後刪除，以支援中斷後重試。
     - 任何一項不符就保留該目錄並記錄，不重試強制刪除。
  3. 安裝失敗的回復流程（`rollback_python`，約 260–269 行）不受影響：回復仍使用 `python.previous`；只有在**成功提交之後**才改名為 trash。
  4. 解除安裝時，一併處理遺留的 `python.trash-*`（沿用同樣的核對規則；無法核對者保留並告知）。
- **驗收**：
  1. 升級安裝後立即存在 `python.trash-*`，啟動 LM_AI 後被清理。
  2. trash 目錄內被加入額外檔案、被修改，或檔案被占用時：保留不刪，有紀錄。
  3. 連結（junction）形式的 trash 目錄被拒絕。
  4. 清理中斷後再次啟動可以接續完成。
  5. 安裝失敗回復情境仍正確還原舊版 Python。
  6. 解除安裝後不遺留可核對的 trash 目錄。

## 第一部分的驗證範圍

P3、P5 改動 runtime 內容與安裝流程，**本部分不可只做相關驗證**，必須執行：

- v142 空 Cargo 快取 `--frozen` 正式建置、fmt、Clippy。
- 全部 Python 相關 Rust 測試與原生測試（含兩個 Python 工作程序同時執行、取消隔離）。
- `scripts/Test-PythonRevision.ps1` 與 runtime 清單核對。
- **完整安裝生命週期**：新裝、從 0.9.18 升級（Python 變更）、Python 未變更時沿用、升級失敗回復、Python 修復、解除安裝；並核對使用者資料保留。
- 第 1 節的耗時比較數字。

可跳過：CAD 圖面整體驗證、Outlook 信箱、VNC、SMB（本部分未改動這些功能）。跳過項目要在驗證文件中列出。

---

# 第二部分：版本更新資訊整理（N1）

## 0. 背景

開啟程式時與「設定 → 教學與更新日誌」顯示的更新資訊，來源是 `assets/release-notes.json`（由 `src/ui/onboarding.rs` 約 150 行 `include_str!` 載入），目前收錄 0.8.75～0.9.18 共 24 個版本。內容有許多使用者不需要知道的內部細節，例如驗證範圍、建置設定、安全機制的具體做法、內部參數；也包含塔羅牌與桌寵的描述。

## 1. 目標

1. 每一則只寫**使用者看得到、用得到的功能或改善**，用簡短的白話說明。
2. 安全性、穩定性、內部架構的修改，只用概括說法，例如「加強安全性」、「提升穩定性」、「修正數項問題」、「改善大型資料處理效率」。
3. **全數移除塔羅牌、桌寵的相關描述**，包括角色、動作、泡泡樣式、牌義、抽牌、互動教學數量中提到的這兩項等。
4. 範圍：`assets/release-notes.json` 內**全部版本（0.8.75 起）**逐則檢查。

## 2. 判斷規則

對每一則問：「**使用者看到這句話後，會不會改變他使用程式的方式？**」不會的就刪除或概括。

| 類型 | 處理 | 例子 |
|---|---|---|
| 新功能、操作方式改變 | 保留，精簡用語 | 「新增差異預覽，可查看 AI 修改了哪些地方。」 |
| 使用者會遇到的限制 | 保留，白話說明 | 「本機任務需在程式開啟或縮到托盤時才會執行。」 |
| 安全機制的具體做法（簽章、雜湊、同源、DPAPI、AppContainer、鎖、Junction、完整性檢查、ACL 等） | 概括為「加強安全性」，不寫做法 | 「公司更新來源與簽章驗證不變」→ 刪除 |
| 驗證範圍與流程（回歸、候選、NSIS、Git、CI、v142、證據、公司實測待驗、本版只提交原始碼等） | 刪除 | 「本版採相關功能驗證，未執行完整回歸…」→ 刪除 |
| 建置與內部架構（ThinLTO、codegen、Cargo features、模組拆分、panic、COM 生命週期） | 刪除，或概括為「提升穩定性與效能」 | 「依實測選用ThinLTO…」→「縮小程式體積、提升效能。」 |
| 內部參數與技術名詞（50,000 tokens、40,000、操作 ID、JSON Pointer、excel-compact-v1 等） | 改成白話效果 | 「輸入軟目標50,000 tokens…」→「改善長任務的上下文保留，減少重複讀取。」 |
| 開發文件、GitHub、內部政策（README 整理、公司 HTTP、機台 JSON 格式維持） | 刪除 | — |
| 塔羅牌、桌寵 | **全部刪除** | 混合句只刪除該部分，例如「側欄以進階功能整合桌寵、塔羅、Outlook 及 VNC」→「側欄整合 Outlook、VNC 等進階功能入口。」；提醒功能「選擇桌寵或 Windows 通知」→「以 Windows 通知提醒」 |

## 3. 格式要求

- 每個版本保留在清單中（`version` 不刪除），`section` 標題也依上述規則改寫，不得提到塔羅或桌寵。
- 若某版本刪除後沒有剩餘內容，改為一則概括說明，例如「介面與使用體驗改善。」或「提升穩定性並修正數項問題。」
- 每版建議 1～4 則；每則以一句話、約 60 字以內為原則。
- 用語通順、口吻一致，不使用內部縮寫或版本計畫代號（例如 M7、X2、D03）。
- 不新增原本沒有的功能描述，不誇大效果。

## 4. 修改處

- `assets/release-notes.json`：逐版改寫。
- `scripts/Test-Settings.cjs`（約 35 行讀取此檔）與 `src/ui/onboarding.rs` 的測試：若有依內容或數量斷言，**依新內容更新斷言，不可放寬或刪除檢查**。
- 新增一項檢查（Rust 測試或 cjs 測試皆可）：`release-notes.json` 中不得出現下列字詞：`塔羅`、`桌寵`、`牌義`、`抽牌`，以及常見內部用語（例如 `回歸`、`候選`、`NSIS`、`v142`、`ThinLTO`、`codegen`、`panic`、`AppContainer`、`DPAPI`、`SHA256`、`JSON Pointer`、`tokens`）。字詞清單寫在測試中，方便日後維護。
- 確認沒有其他地方在程式內顯示 0.8.75 以前或其他來源的更新內容；若有，一併依同規則處理並在驗證文件中列出。

## 5. 不在本次範圍（除非使用者另行指示）

- `CHANGELOG.md`、`README.md`、`docs/USER_GUIDE.md` 與 docs 下的驗證、開發文件：維持原樣。
- 塔羅、桌寵功能本身、設定開關與互動教學：不變（教學已由「額外功能」開關控制）。

## 6. 驗收

1. 逐版對照表：在驗證文件中列出每個版本「修改前 → 修改後」的 section 與各則內容，方便使用者審閱。
2. 新增的字詞檢查測試通過。
3. 啟動時的累積更新說明與「教學與更新日誌」畫面正常顯示，版本順序與累積範圍邏輯不變（既有 `release_pending`／`release_from` 測試通過）。
4. 第二部分只需相關驗證：上述測試、`Test-Settings.cjs`、程式內畫面確認。

---

## 附錄：commit 建議

1. 第一節基準量測紀錄（文件）。
2. P1、P3（runtime 建置腳本與清單）。
3. P2（runtime 驗證沿用）。
4. P4、P5（安裝程式與延後清理）。
5. N1（版本公告與字詞檢查）。
6. 整合驗證與發行紀錄。
