# 0.8.43：圖表座標、Python LOG 與安裝沿用

2026-10-07 已完成完整 v142 建置、Python／Office／SMB 驗證及正式 NSIS 安裝回歸。兩份更新清單均為0.8.43、kind=nsis，與下列正式安裝包的長度及SHA256一致。

| 交付項目 | 大小／SHA256 |
| --- | --- |
| LM_AI.exe | 14,833,664 bytes；`85f72ec176d0146c4f009821aa3cf3fac37263adcd94bfcd35734b793b1f83a9` |
| LM_AI_Setup.exe | 28,064,565 bytes（約28.1 MB）；`d794dce58b816c056b395badefe80ed05addeab33ad3a0a62e50339e7dd84690` |
| Python runtime 清單 | 與0.8.42相同：`ba14595d7e9e75268da361f4bc09a75f07388cfac14d985c137e787e344f811c` |

## 本版修改

- 完成圖表的編輯器與 AI `transform_chart` 均可調整實體 X／Y：保留原值、加上位移，或以起點／非零間距重新編號。使用者要求從1開始時，先照原始 CSV／Excel 欄位建圖，再衍生座標；來源 A／Index、D／Mean 的欄位核對仍保留。
- 移除無值位置只刪除所有系列均為 null 的列，0保留；先刪列再依共同列順序編號，避免多系列各自壓縮而錯位。水平長條的類別軸是Y、量測軸是X。文字類別可改為序號，但不能直接數值平移。
- 原始數據與CSV不改；資料表明示原始值，圖軸／PNG標示轉換。使用者設定以DPAPI保存，PNG快取包含圖表版本，重新轉換不取用舊PNG、不覆寫既有成果。
- 異常Y值視窗加入三張假設五點示意，說明保留缺值的斷線、略過並接續且保留X、設為0。原選項、預設及提交方式不變。
- Python LOG／OUT／ERR無BOM時先嚴格Big5（CP950），失敗再試UTF-8；UTF-8 BOM優先且不因內容損壞退回。metadata提供實際編碼及歧義，模型可核對樣本後以encoding明確指定。TXT／JSON預設UTF-8；CSV契約不變。UTF-16仍經既有LOG工具或匯入文字。
- 工具參數錯誤最多列6個欄位路徑與原因，例如 `$.inputs[2].kind`，不回送完整程式或值，不猜參數執行。Python技能加入完整多檔LOG範例及 `texts[name]` 對應；模型的Python語法錯誤仍由隔離程序回報。
- NSIS先以待安裝EXE完整驗證已安裝Python；相同且可執行就原地沿用，省去解壓／覆寫／權限重設；缺檔、損壞或版本不同走完整替換及失敗回復。worker及套件清單與0.8.42相同，主程式進版不使Python環境過期。完整安裝包仍含Python，下載大小不因沿用機制縮小。
- 專案JPG／JPEG／PNG改為一般支援，AI按任務逐張選用；每任務20次不同要求，單張5,000,000 bytes，相同來源與焦點快取不扣新額度。列目錄／read_file中繼資料不送圖；fast、Outlook與一般聊天附件流程不變。
- 現有累積進度筆記已含目標、已完成、目前步驟與待辦，本版不另加待辦工具或強制規劃回合。

## 建置與驗證

```powershell
.\scripts\Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller
```

Rust主程式及C++安裝測試固定v142 x64：MSVC14.29.30133、實際 `_MSC_FULL_VER=192930159`、SDK10.0.19041.0、Rust1.98.1。Cargo.lock、vendor及.cargo/config.toml搭配新的空Cargo快取與 `--frozen`。

已通過255項Rust單元測試、fmt及Clippy。圖表Node回歸亦通過，涵蓋10000點／8系列、共同列對齊、X／Y轉換、原值保留及PNG使用相同資料。實際瀏覽器檢視亮色／深色／窄視窗的三張示意及編輯器，並核對轉換後散佈圖：X為1、2、3，保留真正0值且不連線。

完整Build流程退出碼為0；原生UI、HTTP、AppContainer、Python／Office／SMB及正式NSIS安裝回歸均通過。正式驗證機器紀錄位於 `offline/`，本次建置日誌保留於本機 `.build/release-0.8.43.log`。

- Python：Big5中文LOG、UTF-8 BOM與無BOM備援確實經過Rust快照及隔離Python，`texts[name]` 文字一致、metadata編碼正確且原檔不變；單元測試另涵蓋200 KB以上內容、雙編碼歧義／明確選擇、非法bytes及BOM不符。CSV／XLSX、真正Excel COM→pandas、取消、120秒逾時、檔案／網路／子程序隔離與runtime改寫拒絕均通過。
- 圖表與圖片：原生HTTP case21包含10次工具往返，來源7001起的圖經transform_chart後從1起，原始值保留；12組圖片情境涵蓋清單／中繼資料不送圖、多圖、20／21次額度、快取、未知請求續查與父子身分。
- 長任務：30份各10,526,720 bytes的LOG、9頁60個結果、長行續讀與版本改變拒絕；原生跨批次68次POST、模擬中斷／重啟、已完成操作去重、DPAPI筆記及上下文恢復。
- Office：三份欄序不同的真實Excel×五時段共15圖；XLSX／XLS的10000點、型別／NG／錯誤／合併儲存格與各種異常處理；DOCX／DOC／DOCM、XLSX／XLS／XLSM／XLSB、PPTX／PPT／PPTM副本及新建文件。PNG嵌入、來源移除後重新開啟驗證均通過。
- 網路：localhost SMB的UNC與暫時映射磁碟，核對文字發布、DPAPI筆記、Word／Excel儲存讀回、路徑越界拒絕及原檔不變。
- NSIS：固定路徑、捷徑與登錄、舊程序交棒、升級重啟、真實Rust／Python自檢及解除安裝。以禁止寫入／刪除的worker檔案鎖及建立時間核對原地沿用；主程式替換失敗仍保留已沿用Python。缺檔、同大小損壞、不同清單的完整修復與舊環境回復均通過，保留未知檔案及VNC設定。實際交付Setup亦完成安裝、WebView2自檢及解除安裝，測試產品目錄已清理。

## 交付與限制

本版交付EXE、完整NSIS、兩份指向同一NSIS的簽署更新清單及對應原始碼。歷史 `CompanyAI-offline.zip` 未重製；Python依賴及runtime指紋不變。上游CPython／wheels為預編譯發行品，CPython標示MSC1944，不宣稱上游套件由本專案v142編譯。

驗收使用合成LOG／Excel／CSV、本機Office及localhost SMB。使用者已回報圖片可正常閱讀、大量LOG會自主選用Python；這不代表新版本在公司加密、防毒、網路分享及模型生成程式品質上已全面驗收。Git部署只更新既有儲存庫，並不修改公司網站更新路由。

設計細節見 [功能設計紀錄](PENDING_PYTHON_REUSE_AND_PROJECT_IMAGES.md) 與 [Python執行環境](PYTHON_RUNTIME.md)。
