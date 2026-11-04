# 0.9.0 驗證紀錄

日期：2026-11-04。本版只包含分版計畫 R01～R10；新功能批次、GitHub Releases／LFS、CI及編譯最佳化延後。

0.9.0本機驗證包已完成v142編譯、空Cargo快取frozen建置、本輪指定的本機原生測試與NSIS封裝／生命週期驗證。公司實機與SMB仍有缺項，不能稱完整環境驗收；本輪本輪授權推送GitHub main；公司實機缺項仍保留。

## 本版行為

- README精簡、使用指南按功能整理，AGENTS歷史全文與搬移對照保留。
- 程序入口安裝同步panic紀錄；背景工作、主要WndProc及WebView2／COM回呼捕捉unwind。失敗後停止新工作，保留診斷、複製及離開；要求重啟並核對原成果，不自動重播寫入。
- 診斷先預覽原生保存的快照，再確認匯出JSON；前端只傳快照ID，不能指定內容或任意路徑。沿用既有JSON格式，沒有另包ZIP。預設不含Token、對話、設定、machines.json、檔名或私人路徑；選用錯誤細節可能包含敏感內容，必須檢視預覽。
- 一般對話與專案都常駐能力提示；錯誤資訊可在未登入時複製、匯出。
- 新版啟動顯示累積版本說明；無舊版紀錄時明示未知，不能推測使用者原版本。
- 納入先前未編譯的VNC側欄入口、桌寵獨立字級及相反色描邊。

## 已執行的驗證

| 範圍 | 實際結果與證據 |
|---|---|
| v142 x64 | 14.29.30133、實際compiler probe 192930159；Rust 1.98.1；空Cargo快取＋vendor＋frozen |
| Rust | fmt、Clippy全target及473項測試通過 |
| CAD契約 | 101項合成契約通過，不代表真模型圖面驗收 |
| Outlook helper | 無效要求回覆及EOF退出通過，未讀取真信箱 |
| panic | Debug與Release的background／WndProc／真WebView2 ExecuteScript完成回呼／未捕捉ABI abort四種獨立程序均通過；中止前有不含fixture機密的同步紀錄 |
| 前端 | Auth 20項、Workbench 21項、Settings與57教學、Pet/Tarot 47項、Reliability UI 18項通過 |
| 原生WebView2 | 新四按鈕、既有VNC側欄操作、composer及視窗自檢通過；本版panic測試另以真正WebView2回呼驗證 |
| 專案隔離 | AppContainer檔案／網路限制、Unicode副本、重播抑制、來源保護、保存讀回、越界及hardlink拒絕通過 |
| 原生整合與封裝 | Python隔離／執行、Office讀寫與圖片、CAD比較、loopback UltraVNC及NSIS安裝／升級／回復／移除均通過；見下方證據 |

代表命令：

```powershell
./scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller -TestOffice -TestCadComparison -TestVnc
node scripts/Test-UiAuth.cjs
node scripts/Test-Workbench.cjs
node scripts/Test-Settings.cjs
node scripts/Test-PetTarot.cjs
node scripts/Test-ReliabilityUI.cjs
```

`Build.ps1`現已納入`Test-Reliability.ps1`，非預期錯誤測試不與一般單元測試共用程序。

R2完整流程先驗Debug panic harness，另以相同v142環境執行`cargo build --example reliability_smoke --release --frozen`及`Test-Reliability.ps1 -NoBuild -Probe target/x86_64-pc-windows-msvc/release/examples/reliability_smoke.exe`，四種情境也通過；證據見`.build/reliability-release-0.9.0.log`。正式建置入口已固定使用這組Release命令；主程式來源及EXE沒有因此改變。

## 保留的失敗與修正

1. 首輪原生WebView2自檢仍斷言頂部只有兩個按鈕；本版新增複製錯誤與診斷按鈕後，此斷言失敗。已更新為核對四個按鈕的位置與身分，重跑原生自檢及完整R2建置均通過。原始log：`.build/build-0.9.0-full-R1.log`。
2. 真模型合成文件案例要求建立Markdown摘要，但既有工具僅允許從MD來源建立MD副本；模型反覆嘗試後回報部分完成，第二個要求又達到本機bridge的35輪上限（HTTP 400）。因此本案例不通過，也沒有CSV副本交付。保留`tools/scenarios/reliability-documents.json`及本機原始往返，不將摘要回答當作檔案成功。
3. 另立`tools/scenarios/reliability-documents-supported.json`，只將第一項成果格式改為現有支援的TXT，以驗證既有讀寫流程；它不能取代或抹去上項失敗。本版不順勢加入Markdown建立功能。

支援格式案例共30輪，已完成並獨立讀回核對：來源CSV與備忘錄逐位元不變；TXT摘要包含甲／乙／丙及總額60；CSV副本恰為10／20／40、合計70；測試根目錄及專案內沒有`escape-marker.txt`，摘要未帶入惡意標記。這只驗證此合成案例，不是全面防prompt injection的證明。結果與雜湊見[模型驗證摘要](../offline/model-verification-0.9.0.json)。

panic與前端證據：[原生panic](../offline/reliability-verification-0.9.0.json)、[診斷與更新介面](../offline/reliability-ui-verification-0.9.0.json)、[設定／桌寵／教學](../offline/ui-verification-0.9.0.json)。

模型原始往返留在`.build`，不提交對話／認證資料。提交的報告只記合成輸入、結果與限制。

## 公司限制與剩餘驗收

- `src/config.rs`、`src/deployment.rs`、`src/vnc/config.rs`及`.gitattributes`未變更；公司HTTP、lm_server更新同源及簽章驗證、machines.json明文格式均維持。
- 固定安裝位置`C:\largan\LM_AI`、NSIS安裝／升級／回復／移除與兩份簽署清單均經本輪檢查通過；機台設定與未知使用者檔案保留。
- CAD使用獨立新圖／副本。不能將原生固定參數測試當成真模型設計與圖面驗收。
- 公司登入、網站模型、真Outlook、公司加密文件、公司機台VNC、lm_server真升級及SMB均需公司環境補驗。
- 桌寵字級／保存／GDI字型及前端預覽已測；多螢幕實際DPI、拖曳及通知點擊仍需人工實機驗收。
- 不保證從原生存取違規、OOM、強制結束、斷電或無法寫入磁碟恢復。捕捉panic後不能把部分寫入宣稱為已回復。
- 不新增使用統計／評價API或遙測；0.9.x後再評估。

完整核心驗收清單見[CORE_REGRESSION.md](CORE_REGRESSION.md)，逐版工作狀態見[IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md)。

## 程式碼複核入口

- [reliability.rs](../src/reliability.rs)：同步最小紀錄、unwind邊界、不可再執行工作的保護狀態；[main.rs](../src/main.rs)在程序入口安裝hook。
- [ui.rs](../src/ui.rs)及[diagnostics.rs](../src/ui/diagnostics.rs)：新工作門檻、在途取消／晚到事件隔離、原生預覽快照與儲存位置。
- [webview.rs](../src/webview.rs)、[pet.rs](../src/pet.rs)、[selection_popup.rs](../src/selection_popup.rs)及[tutorial_preview.rs](../src/ui/tutorial_preview.rs)：主要WebView2／COM與WndProc回呼邊界。
- [onboarding.rs](../src/ui/onboarding.rs)與[onboarding.js](../ui/onboarding.js)：舊紀錄遷移、未知原版本、累積範圍與首次啟動提示。
- [Build.ps1](../scripts/Build.ps1)、[Test-Reliability.ps1](../scripts/Test-Reliability.ps1)及[reliability_smoke.rs](../examples/reliability_smoke.rs)：獨立程序的背景／回呼／abort測試。

## 本機成品與最後核對

- [LM_AI_Setup.exe](../dist/LM_AI_Setup.exe)：SHA256 `d10e2f82add4eae5cf758a95afebcbe7461d68919d0eaf52ff0a207e133c611c`。
- [LM_AI.exe](../dist/LM_AI.exe)：SHA256 `50744caacfa5f58a580a67cb80dde3223fdfc84ad69f043e5577d5e833c37bb5`。
- [SHA256SUMS.txt](../dist/SHA256SUMS.txt)與兩份更新清單均對照本輪成品；清單仍是公司`/lm_server/desktop/releases/LM_AI_Setup.exe`，沒有改成GitHub。簽章驗證使用應用程式既有RSA更新驗證；沒有新增Authenticode程式碼簽章。
- [完整建置摘要](../offline/exe-verification-0.9.0.json)、[NSIS生命週期](../offline/installer-verification-0.9.0.json)、[Office](../offline/office-verification-0.9.0.json)、[Python](../offline/python-verification-0.9.0.json)、[VNC](../offline/vnc-verification-0.9.0.json)。本機完整log：`.build/build-0.9.0-full-R2.log`。
- 私有設定、對話、Token、machines.json、`.build`、`.codex`及`dist/onboarding-state.json`未納入公開驗證摘要；原始模型往返只留本機。
- 原有0.8.79 EXE／Setup與清單另留在本機`.build/baseline-0.8.79-47845a3cf4774951b0006e4c72560ea2`供核對。

發布前檢查：code-verification 的公開摘要已遮蔽一處 Windows 帳號路徑；原始報告保留於本機 .build/publication-original-reports，不改測試結果或成品雜湊。
