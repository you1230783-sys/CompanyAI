# 0.9.19 Python 與公告整理：驗證紀錄

發布完成（2026-11-06）：使用者追加授權後，Git 與 [v0.9.19 Release](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.19) 已發布；同來源 hosted CI 與五項公開下載 SHA256 核對通過。以下保留本機完成時的歷史證據，最終發布紀錄見文末。0.9.20 仍暫停。

本機交付階段紀錄：0.9.19 實作、手冊範圍的完整本機驗證、v142 正式建置與封裝封存完成；當時尚未推送或公開發布，後續結果見上方與文末。依使用者最新指示到此暫停，0.9.20 未實作。依 [變更手冊](../CHANGE_HANDBOOK_0_9_19_PYTHON_AND_NOTES.md)；手冊原文不修改。

## 範圍與實作

- P1：Python 3.13.12 以 unchecked-hash 產生 bytecode，固定 co_filename 路徑；保留 worker `-B` 與 AppContainer 唯讀權限，pyc 納入同一雜湊清單。
- P2：程序層級保存全部驗證 handle，失敗不快取；後續仍列舉完整路徑集合以拒絕新增模組，不重新雜湊鎖定內容。
- P3：只裁剪名稱恰為 tests 的目錄。**實測必要例外**：保留 NumPy 2.2.6 的 `numpy/_core/tests/_natype.py` 原檔，因 numpy.testing 與 SciPy 間接匯入；該 namespace package 原本沒有 __init__.py，不另加檔案。全刪版本確實失敗，保留完整 traceback。
- P4／P5：大量檔案操作僅顯示摘要；提交成功後將 previous 改名為隨機 trash，正常啟動背景清理。清理需要已登記歷史清單，先鎖定與驗證全樹，再依 handle 刪除，清單最後刪除；不碰目前／pending／previous。解除安裝走同一原生入口，異常保留並記錄。
- N1：24 個歷史版本全部保留版本號，逐則改成使用者資訊，新增詞彙與長度檢查。[完整修改前後對照](RELEASE_NOTES_0_9_19_DIFF.md)。另新增本版三則公告。
- 公告來源掃描：正式內容只由 `src/ui/onboarding.rs` 載入 `assets/release-notes.json`；啟動累積說明與設定頁共用它。未找到 0.8.75 以前的另一份產品公告；教學中的「更新資訊（示範）」只是入口說明，沒有虛構的歷史版本內容。
- 畫面證據：[原生啟動累積公告](images/0.9.19-startup-notes.jpg)、[設定頁窄版更新日誌](images/0.9.19-settings-history.png)。前者為實際 EXE，後者為正式前端資源的瀏覽器驗收；不把兩者混稱為同一種測試。

## 修改前基準

測量機器與既有 0.9.18 runtime 相同。Python 量測 harness 只拆時間，沒有改 runtime／worker；直接呼叫正式 verified_runtime 與 AppContainer 管道。前後 harness 均為 **debug 建置**，正式交付 EXE 為 release；這是相同條件下的實作比較，不將 debug 絕對耗時當成正式應用的效能保證。分析時間含 import、初始化、序列化與橋接，**不是純 import**。

| 階段（毫秒） | 首次 | 同程序第二次 |
|---|---:|---:|
| 完整性驗證 | 6174.554 | 9326.082 |
| worker 啟動與就緒 | 161.406 | 73.808 |
| 分析管道 | 2461.472 | 2452.919 |
| 實際最小計算 | 0.000400 | 0.000400 |
| 合計 | 8797.442 | 11852.812 |

另以同一 runtime 在 AppContainer 外獨立量測指定套件 import：2381.261 毫秒；不把此值冒充隔離程序內 import。證據 `.build/python-baseline-0918.json`、`import-baseline0918.json`。

安裝基準使用同一 0.9.18 EXE／runtime／LZMA 設定，只改成獨立 `LM_AI_Python_Baseline` 目錄並加入計時。版本切換以清單加空白觸發不符，屬合成版本切換；後續另實測真正 0.9.18 → 0.9.19。首輪新裝 67155.070、沿用 25246.945、切換 60356.458 毫秒；首輪階段記錄因漏 FileSeek END 而被覆寫，不能採用，已修正量測器並重跑。

## 已完成檢查與已知失敗

- 原生基準首次受受限環境拒絕建立 AppContainer（0x80070005），經原生權限重新執行成功。
- compileall 首輪受限環境禁止 multiprocessing 管線（WinError 5）；改在新目錄以原生權限重建。第二輪發現上述 NumPy 相依失敗；第三輪保留必要模組後通過。
- 第三輪真實 pandas／numpy／scipy.optimize NNLS／signal.find_peaks／stats／interpolate／Matplotlib Agg PNG／openpyxl 保存讀回通過。
- 裁剪後（bytecode 前）4323 檔、235202934 bytes；加入 2006 個 pyc 後 6329 檔、276371732 bytes（均不含 runtime-manifest.json）。pyc 會增加檔案數，不能宣稱最終檔案數減少三分之一。
- 第二份全新 runtime 重建與固定清單逐檔雜湊、大小一致；未更新信任基準，證據 `.build/python-repro-0919.log`。
- 設定與 58 項互動教學無頭瀏覽器測試通過，包含更新日誌窄版畫面；首輪受限環境未能啟動 Edge，使用隔離測試 profile 重試通過。

## 第一輪驗證進度（歷史紀錄）

此時完整功能回歸、v142 空 Cargo 快取 frozen 正式 EXE、574 項 Rust、18 組前端、PythonRevision、雙 Python worker 與取消隔離、原生專案／Python／Office／文件／圖片／排程已通過，NSIS 封裝與安裝矩陣仍未完成。後續失敗、修正與重測結果見下文。

依手冊可跳過 CAD 圖面整體、真 Outlook 信箱、VNC、SMB；公司模型／實際文件用量仍由公司端驗收，不以本機合成測試替代。

## 改善後 Python 量測

同機、同一份固定 runtime、兩次連續的正式 AppContainer 分析；單次樣本，不宣稱統計平均。分析管道包含 import／初始化與橋接，與基準使用相同拆法。

| 階段（毫秒） | 首次 | 同程序第二次 |
|---|---:|---:|
| 完整性驗證／鎖定路徑集合核對 | 6826.648 | 411.849 |
| worker 啟動與就緒 | 106.005 | 83.901 |
| 分析管道 | 1040.001 | 929.716 |
| 實際最小計算 | 0.000400 | 0.000500 |
| 合計 | 7972.656 | 1425.467 |

AppContainer 外同套件獨立 import 為 738.272 毫秒（原 2381.261）；證據 `.build/python-after-0919-r2.json`、`import-after0919.json`。首次隔離量測因尚未準備新 runtime ACL 而失敗（Windows 109），正式 `prepare_runtime` 成功後重測，沒有放寬隔離權限。

原 runtime 為 6695 檔、273441693 bytes；最終含 bytecode 為 6329 檔、276371732 bytes，檔案數減少約 5.47%，大小增加約 1.07%。裁剪效益不能與加入 bytecode 的最終大小混為一談。全部 2006 個 pyc header 皆為 unchecked-hash；額外在 `-B` 下禁止 SourceFileLoader 編譯原始碼，再實際載入科學套件與計算，確認確實讀取既有 bytecode（`.build/bytecode-read-0919.log`）。

## 完整回歸的中途結果

- 第一輪正式建置：v142 `_MSC_FULL_VER=192930159` x64、空快取 frozen、fmt、全 target Clippy、514 項非 CAD Rust 測試通過；另 60 項 CAD 純契約測試通過，不代表圖面實測。
- 18 組前端測試中，DocumentWork 首次因導航初始化未完成而失敗。改為等待完整 document 與 NavigationUI 就緒，保留原 24 項斷言後通過；其餘 17 組通過。
- 真實資源測試已確認 Word 排隊、Excel 獨立、外部占用、兩個 Python worker 重疊執行及取消隔離。原生啟動累積公告已人工核對並留圖 `.build/native-release-notes-0919.png`，設定頁窄版畫面另留圖。
- 第一輪原生專案案例 0–19 通過，20 的來源占用案例失敗。補充診斷後確認是既有 `operation_ownership` 在副本操作前回傳 FILE_IN_USE，繞過原有等待流程。修正為尚未執行操作時先等待，確認後重新核對來源與權限，不永久記為契約錯誤；原案例原斷言通過，之後完整原生流程重測通過。
- Python 原生竄改案例配合 P2 調整：過去先改寫 worker 再要求下次分析拒絕，現在改為斷言寫入立即得到 Windows sharing violation 32、原始 bytes 完全不變、後續分析仍成功。清單外模組偵測保留；首次驗證前的內容損壞另由單元與真實安裝修復案例覆蓋。此調整沒有移除完整性檢查。

0.9.18 安裝量測修正版：新裝 66152.215 毫秒、沿用 35838.560 毫秒、合成清單不符切換 65315.605 毫秒。沿用自檢 5031 毫秒；切換階段解壓 26359、改名 125、自檢 3672、同步刪舊 5125 毫秒；其餘為啟動、準備權限、CAD 核對等既有成本。證據 `.build/baseline-install-total-r2.json`、`baseline-install-timing.txt`。

## 功能回歸基準成品

- `.build/full-build-0919-r2.log`：使用 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller -SkipCad -TestOffice`，完整功能流程通過；該次腳本在後續安裝矩陣中斷，沒有成功退出。
- 514 項非 CAD Rust 與 `.build/cad-unit-0919-final.log` 的 60 項 CAD 純契約測試皆通過。空 Cargo 目錄 `cargo-empty-7f4a257df1724664bb610ce1c6bdd57d` 沒有下載套件，registry 只有 CACHEDIR.TAG；依賴由專案 Cargo.lock、vendor、.cargo/config.toml 提供。
- 原生代理 0–31 共 32 案例通過；長任務正常／中斷恢復各 69 步、134 POST，沒有重做已完成讀取。`fixture: simulated interruption` 是案例 19 的預期中斷，之後恢復成功。
- 完整 Python 原生：真實 SciPy NNLS、Big5／BOM、並行批次、Excel COM → CSV → pandas、程式編輯驗收、隔離、取消、120 秒逾時、保留 handle 拒絕改寫及後續分析、交付時拒絕被改寫檔案，全部通過。
- 完整 Office 原生：doc／docx／docm、xls／xlsx／xlsm／xlsb、ppt／pptx／pptm 的保存與重開；大型 Excel、15 個不同欄序／區間圖表、中文 PDF、文件比較、帶樣式翻譯、PNG 嵌入後移除原圖再重開、數值綁定報告與排程占用處理通過。
- `.build/resources-0919-final.json`：功能回歸基準成品的雙 Python worker／取消隔離及 Word 排隊、Excel 獨立操作通過。PythonRevision 在每次正式重建後另外重跑，最後收據對應最後成品。
- 最終原生公告畫面已重新以 EXE `79a178266d02ecd11ecb8ff3ae6efa8175ceb46b9e89f48658440c6393330d17` 核對；保留 25 版本順序、目前版本及稍後閱讀操作。前端最終彙整為 `.build/frontend-0919-final.json`，第一輪失敗紀錄仍保留。

## 安裝驗證的中斷與重測

- 完整 Build r2 在功能階段全部通過後，於安裝矩陣中斷，沒有產生整體完成收據；不能宣稱該次腳本成功退出。中斷後的已知測試目錄先解除安裝，剩餘 `python.previous` 與未知 CAD 合成資料完整保存於 `.build/installer-interrupted-0919`，沒有遞迴刪除。
- `Test-Installer.ps1` 增加每個程序的開始／結束記錄，並在等待前非同步讀取 stderr，避免輸出管線滿載造成互等。重跑完整矩陣的 `.build/installer-0919-r3.log` 揭露下述實際改名問題。
- 原生安裝畫面首次測試因測試器自行覆寫 `LOCALAPPDATA`，使 AppContainer 暫存路徑與 Windows 真正配置不符而自檢失敗；安裝器正確回復並顯示[失敗提示](images/0.9.19-installer-error.jpg)。修正測試環境、沿用 Windows 真正 profile 後已[安裝成功](images/0.9.19-installer-success.jpg)。未改產品 ACL、未放寬隔離權限。失敗與成功計時獨立保存，不混入正式效能比較。
- 手動確認 Python 解壓與刪除只呈現階段摘要，仍看得到自檢、切換、失敗回復訊息；沒有逐個列出 Python 檔案。

### 完整安裝矩陣發現的改名問題

r3 於缺檔修復後失敗：正式 helper 對 `python.previous` 改名得到 Windows sharing violation 32。原因是安裝根目錄的一般讀取鎖不分享 WRITE，連本程序 rename 對目的父目錄所需的存取也被阻擋。新增成功改名、延後刪除、占用時保留及根目錄不可被改名的案例；未修正版本明確重現失敗。

修正只對改名階段使用保留 GENERIC_READ、分享 READ／WRITE、仍禁止 DELETE 分享的根目錄 handle；先取得新鎖再釋放原鎖。曾嘗試只讀屬性，新增的根目錄保護測試抓到它不能阻止目錄改名，因此未採用該候選。最後修正的 9 項清理測試通過，來源提交 `5942f2c`；不變更 runtime ACL 或裁剪清單。

修正後 576 項全 workspace Rust、fmt、全 targets Clippy、v142 空快取 frozen 正式建置、PythonRevision 與完整安裝矩陣 r4 通過。先前功能回歸成品保存於 `.build/pre-retire-fix-0919`；本次改名修正版保存於 `.build/pre-background-fix-0919`。證據中分別記錄雜湊，不將先前 EXE 冒充最後成品。

### 真實升級發現的背景清理衝突

真正安裝 0.9.18，再升級 0.9.19 後，舊 Python 已正確延後清理，但正常啟動的累積公告沒有保存新版本：背景清理仍用一般根目錄讀取鎖，擋住旁邊 `onboarding-state.json` 的原子替換。失敗時的舊 JSON、新版暫存 JSON 與合成使用者檔案均保存於 `.build/upgrade-onboarding-failure-0919`，沒有降低公告範圍斷言。

提交 `655bf12` 讓改名與背景清理共用 `pin_installation_root`：先固定父目錄，再以分享 READ／WRITE、禁止分享 DELETE 的 handle 開啟並核對安裝根目錄；避免暫時使用一般鎖的轉交期間仍阻擋設定。舊 runtime 的檔案與目錄仍逐項獨占核對。新增實際檔案系統測試，確認持鎖時公告可原子替換，但安裝根目錄仍不能被改名；10 項清理測試通過。測試器輪詢公告時亦允許 DELETE 分享，避免測試本身阻擋產品保存。

修正後 577 項全 workspace Rust、fmt、全 targets Clippy 與 v142 空快取 frozen 正式編譯通過，證據 `.build/background-final-build-0919.log`／`.err`。後續封裝、真實升級 r3、完整安裝矩陣 r5 與最終安裝計時皆通過，結果如下。


## 最終成品與驗收結果

- 最後產品來源提交 `655bf128c5df59eb95a135d4948d4bef1063dae8`；後續提交僅整理文件與收據。與功能回歸基準的產品差異限於上述 Python 清理根目錄鎖，功能回歸與最後成品雜湊分開保存。
- MSVC v142 14.29.30133 x64，實際 `_MSC_FULL_VER=192930159`。最終空 Cargo 快取 `.build/cargo-empty-f52307330c6d4e8ab1cbc5305b6c778e` 僅有快取管理檔，沒有 registry/src 或 registry/cache；使用 Cargo.lock、vendor、.cargo/config.toml 與 `cargo build --frozen`。
- 577 項全 workspace Rust、fmt、全 targets Clippy、18 組前端、完整原生功能回歸均通過。功能基準包含 32 組原生 HTTP、長任務正常／恢復、完整 Python／Office／文件／圖片／排程；最後成品另重跑 PythonRevision 與完整安裝矩陣，並非宣稱先前中斷的單一 Build 腳本成功退出。
- 真正 0.9.18 → 0.9.19 升級通過：保留恰一份舊 runtime、額外檔案／唯讀／占用時拒絕清理、正常啟動背景清理及累積公告範圍保存、真實程序層級 runtime 鎖的 UPDATEPID 交接、解除安裝清理及使用者資料保留。紀錄 `.build/python-upgrade-0919-r3.log`，升級 50417.129 毫秒，持鎖 PID 交接 51492.343 毫秒；這是功能案例，與下方獨立效能測量分開。
- 完整安裝矩陣 r5 通過：新裝、更新與重啟、占用及 Python 初始化失敗回復、未變更 runtime 沿用、缺檔／同大小竄改／不同清單修復、已知中斷清理續作、未知內容保留、正式固定路徑、使用者資料保留、解除安裝，以及安裝後原生／Python／CAD 自檢。CAD 自檢只核對依賴，不代表 CAD 圖面操作。
- Junction、硬連結、載入中的 DLL、未知或被修改的清單等清理拒絕案例由實際 Windows 檔案系統 Rust 測試覆蓋。修改內容的原生清理拒絕另由完整安裝矩陣覆蓋。
- 簽署更新清單由最後 EXE 的原生驗證入口核對通過；兩份相容清單、EXE、Setup 與 SHA256SUMS 已封存於本機 `舊版留存/0.9.19/`，逐檔雜湊一致。未執行 hosted CI、未發布 GitHub Release、未建立新的離線 ZIP。

| 最終成品 | SHA256 |
|---|---|
| dist/LM_AI.exe | `101ddd9f50b6949aff617558d0d70d817b86601c104d47f677444425553b78f7` |
| dist/LM_AI_Setup.exe | `c297074d32b7a23e37254b1b5a6093edb0114eb9495e55f14037ee685e6d583c` |
| Python runtime-manifest.json | `ebaacb792f2e2d1d6bd0ccf9443d94682bdd64629f988c3eed9cd4721ac4a1ca` |

完整機器收據：[release-verification-0.9.19.json](../../offline/release-verification-0.9.19.json)。其中明確分開功能回歸基準 EXE 與最後 EXE，各項 JSON 依實際成品標示，不以舊結果冒充新成品。

## 最終安裝效能比較

同一台電腦、相同 LZMA 設定、各情境單次樣本。兩側皆使用獨立測試安裝目錄，0.9.19 量測包內的 EXE 與最終正式 EXE 相同；僅加入階段計時，不將安裝自檢錯誤或失敗重試混入數值。清單不符切換由清單增加空白觸發，真正跨版升級另由上節驗證。

| 整體耗時（毫秒） | 0.9.18 | 0.9.19 |
|---|---:|---:|
| 新裝 | 66152.215 | 52074.975 |
| Python 未變更沿用 | 35838.560 | 16717.461 |
| 清單不符切換 | 65315.605 | 47456.102 |

| Python 階段（毫秒） | 0.9.18 | 0.9.19 |
|---|---:|---:|
| 沿用時自檢 | 5031 | 2437 |
| 切換時解壓 | 26359 | 25187 |
| 切換目錄 | 125 | 125 |
| 切換後自檢 | 3672 | 2484 |
| 同步刪舊／改名延後處理 | 5125 | 15 |

0.9.19 延後清理另耗 1172.848 毫秒，不包含於安裝等待時間。整體還包含 CAD 核對、權限準備與程序啟動等成本；單次數字不代表平均值或公司端效能保證。原始階段記錄及量測包雜湊納入最終收據，上一候選的數字保留為歷史，未混用。

## 保留的驗收界線

依手冊跳過 CAD 圖面整體、真 Outlook 信箱、VNC 與 SMB。公司模型、真實 EIS／Excel 文件及公司電腦效能仍待公司端實測。0.9.18 的 B／C 已抄入 [未來待做](../FUTURE_IDEAS.md)；本版不實作。0.9.20 手冊保留原文，依最新指示暫停。


## GitHub 發布與公開下載核對

- Git main 已推送，Release 標籤 `v0.9.19` 固定於 `2a9bf7374afe7c4ca0a1d1637e0ccb10d7699a7e`。正式成品的產品來源仍為 `655bf12`，兩者僅有文件／收據差異；未重新編譯或替換已驗證的 EXE／Setup。發布後的文件提交不移動標籤。
- [同來源 GitHub Actions](https://github.com/you1230783-sys/CompanyAI/actions/runs/42215112199)（run `42215112199`）通過。下載 CI artifact，核對 v142 14.29.30133、實際 x64 編譯器、空 Cargo 快取 frozen、fmt、default/core Clippy 與 Rust 測試、Cargo.lock 雜湊及 RustSec 報告。Hosted runner 排除既定兩個互動桌面測試；本機完整驗證另有覆蓋。維持有期限的既有 ttf-parser 維護公告例外，未出現新增漏洞。
- [正式 Release](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.19) 的安裝包、兩份簽署清單、verification.json 與 SHA256SUMS.txt 共五項資產，全部以**不帶登入憑證**的公開下載重新取得，大小及 SHA256 與上傳前本機檔案相同；標籤 commit 亦由 GitHub API 核對。
- 這次發布 GitHub 手動下載，沒有部署公司更新伺服器。0.9.20 保留手冊且不實作；公司模型與實機驗收界線不變。
- 機器證據：[CI 核對](../../offline/github-ci-verification-0.9.19.json)、[發布前整合驗證](../../offline/release-publication-verification-0.9.19.json)、[公開下載核對](../../offline/release-download-verification-0.9.19.json)。原本的 release-verification-0.9.19.json 保留本機交付時的狀態，不回寫歷史時間。
