# 0.8.42：離線 Python 分析與完整 NSIS

發行狀態：2026-10-07 完成 v142 建置、Python／Office／網路驗證及正式 NSIS 安裝驗收。

| 交付項目 | 大小／SHA256 |
| --- | --- |
| LM_AI.exe | 14,685,184 bytes（約14.7 MB）；`22b98da1847798f21f5e782687bd511ae997c53924b87d3079949b06c4acdb09` |
| LM_AI_Setup.exe | 28,022,132 bytes（約28.0 MB）；`dc5156b10e8ad380efaf4ed7c1da578ff83d28a2085fdd86226643b546f39706` |
| 獨立 Python 目錄 | 3,973個檔案、106,753,992 bytes（約101.8 MiB）；不計入主程式 EXE |
| runtime 清單 | `ba14595d7e9e75268da361f4bc09a75f07388cfac14d985c137e787e344f811c` |

## 實作範圍

- CPython 3.13.12 x64、pandas 2.2.3、NumPy 2.2.6、openpyxl 3.1.5 與固定的必要依賴；全部放在 EXE 旁的 `python` 子目錄。沒有系統 Python、PATH、pip 或使用者套件備援。
- 新增 `python-analysis` 技能與 `run_python` 工具，工具目錄48項。技能提供穩定 API 範例，模型不需上網查套件用法。
- Excel 原檔繼續透過 Rust Excel COM，匯出追蹤 CSV 後交給 pandas。Python 只取得核准快照；生成的 `_AI_Output` XLSX 可用 openpyxl 再讀。
- 分析使用獨立 AppContainer／Job，完成、取消與逾時均終止；沒有網路 capability，單一程序、1GiB、120秒計算。來源最多8份、單檔32MiB、來源快照合計60MiB。
- 結果使用精簡 JSON；CSV／XLSX 由原生 broker 保存並讀回。CSV 最多100000列／16欄，XLSX 每表100000列／32欄、最多12表／8MiB；每次最多8份成果。生成成果加入 checkpoint 及最終交付，精簡上下文後仍保留 XLSX 索引。
- NSIS 安裝至 `C:\largan\LM_AI`，先準備新 runtime、驗證與隔離自檢，再完成 EXE 替換；失敗回復舊版。只刪除固定清單的套件檔案，保留未知檔案、VNC 設定與聊天資料。兩份更新清單都指向完整 NSIS。

## 已核對的 Python 行為

- UTF-8 CSV 保留批號前導零與字面 `NA`；pandas 分組 count／mean／sum，以及 LOG 起訖時間計算。
- 追蹤 CSV 與多工作表 XLSX 生成、再次讀回；`=1+1` 經標準輸出 helper 保持字面文字。
- 真正 Excel COM → CSV → pandas，數值及來源列核對，原始 Excel bytes 不變。
- 同 operation id 不重複發布；過期來源版本、路徑越界、原始 XLSX 直接讀取、非法成果檔名及超大摘要被拒絕。
- 真正 AppContainer 內嘗試讀取未交付的測試檔、連線本機監聽埠及另開程序均遭拒；父程序合成環境標記未傳入。
- 執行中的無限迴圈可取消，120秒回應逾時會終止；清單外與改寫的 runtime 檔案被拒絕。
- 修改成果後不能沿用舊交付；成果 checkpoint 可恢復，內容改變時拒絕恢復。
- 從 `python-inputs.zip` 在新目錄離線重建，PowerShell 5.1／7 均逐檔符合固定清單，輸出相同清單 SHA256。

## 建置與測試

完整命令：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller
```

Rust 主程式與 C++ 驗收使用 v142 x64：MSVC 14.29.30133、實際 `_MSC_FULL_VER=192930159`、SDK 10.0.19041.0；Rust 1.98.1。Cargo.lock／vendor／.cargo/config.toml 搭配新的空 Cargo 快取及 `--frozen`。247 項單元測試、fmt、Clippy 已通過。

完整 Build 流程通過主程式、Python、Office、UNC／映射磁碟與封裝。安裝驗收最初因 NSIS 非同步解除安裝尚未刪除最後的登錄項目，測試提前檢查而中止；修正的只有測試等待條件，隨後獨立重跑 `Test-Installer.ps1` 全部通過。主程式與正式 Setup 位元組未變，最後已比對原生建置紀錄／安裝紀錄的 SHA256，並由發行 EXE 再驗證兩份 NSIS 更新清單。

NSIS 實測包含固定路徑與捷徑／登錄、舊程序交棒、升級重啟、主程式被占用及 Python 初始化失敗的雙重回復、真實 Rust／Python 自檢、真正交付 Setup 安裝，以及解除安裝保留未知檔案／VNC 設定。缺少 WebView2 的情境只對測試子程序指定不存在的位置；Setup 可完成，主程式開啟時仍提供原有安裝提示。

Python 與 wheels 為固定的上游預編譯發行品，沒有在本機用新 MSVC 編譯；官方 CPython 標示 MSC 1944。這與本專案 v142 建置分開記錄，不能把上游 Python 宣稱為 v142 成品。來源與 SHA256 見 `offline/python-lock.json`，個別套件授權保留於安裝目錄。

機器紀錄：`offline/python-verification.json`、`python-rebuild-verification.json`、`exe-verification.json`、`office-verification.json`、`network-verification.json`、`installer-verification.json`、`environment.txt`。歷史 `CompanyAI-offline.zip` 未重製，新加入的 Python 依賴 ZIP 不是新版整套 Rust 工具鏈包。

## 公司驗收範圍

本機使用合成 Excel／CSV／LOG、已安裝的 Office 及 localhost SMB。尚未驗收公司加密產品、內網模型的程式生成品質、防毒／AppContainer 政策及真正網路分享。Git 推送不代表公司網站更新路由已部署。

建議先安裝本版 Setup，在專案輸入：「用 Python 依機台統計 CSV，保留批號前導零，產生統計 CSV 與 Excel 報告。」再用公司原始 Excel 要求相同分析，核對工具紀錄中先走 COM 匯出、再執行 Python，以及原檔不變、成果可開啟。

完整設計與離線重建方法見 [PYTHON_RUNTIME.md](PYTHON_RUNTIME.md)。
