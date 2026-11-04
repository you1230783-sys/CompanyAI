# Windows 綁定與 COM 邊界

0.9.5評估結論：保留`windows 0.62.2`與`windows-sys 0.61.2`，統一COM生命週期管理。本版沒有宣稱移除其中一套依賴，也沒有為了讓依賴清單只剩一列而重寫Win32回呼、沙箱、GDI或WinHTTP。

## 選擇依據

固定Cargo.lock的`cargo tree --frozen -i windows`顯示：應用、webview2-com及webview2-com-sys都使用同一份windows 0.62.2。WebView2的型別化COM介面本來就需要它，改成只留windows-sys必須改寫這個邊界。

windows-sys 0.61.2目前只有應用直接使用，負責39個原始碼／範例檔案中的原始Win32介面。改成只留windows，需要逐個處理句柄新型別、BOOL／Result、可選指標、字串與輸出參數、回呼ABI及錯誤碼語意。這不是單純更換use路徑，不能假設機械替換後仍保持原行為。

兩套crate提供的是Windows API綁定，並不是把兩份Windows DLL打包進EXE。編譯成本與最終體積需要量測，不由套件數量推斷。本版不聲稱帶來體積收益；體積配置依計畫留到LTO版本實測。

## 現行分工

| 邊界 | 使用方式 |
| --- | --- |
| Office／Outlook／WIC／UI Automation／WebView2 COM物件 | windows型別化介面與既有所有權管理 |
| 視窗／GDI／WinHTTP／沙箱／程序／檔案／加密原始呼叫 | windows-sys，維持既有回傳值與錯誤碼契約 |
| COM初始化與釋放 | `src/com_apartment.rs`單一實作，呼叫端不得新增自己的CoUninitialize守衛 |
| Win32／COM回呼panic | 沿用`src/reliability.rs`既有原生邊界，不放寬全域保護 |

## COM生命週期

- Office、Outlook、主視窗與教學視窗使用`Apartment::sta()`；UI Automation工作執行緒使用`mta()`，模式衝突回報失敗。
- WIC的`for_imaging()`沿用既有規則：可建立MTA，也可借用呼叫執行緒的STA。只有本次CoInitializeEx成功（S_OK或S_FALSE）才平衡一次CoUninitialize；RPC_E_CHANGED_MODE借用既有STA時不釋放它。
- 守衛的Rc標記使其不能Send／Sync，不能把初始化責任帶到其他執行緒。沒有Clone、Default或公開欄位可憑空建立守衛。
- 先建立守衛，再建立COM物件，利用區域變數逆序釋放。結構欄位、回傳值與尾端暫時物件仍需人工檢查；守衛不會自動證明所有COM物件的生命週期。
- UI Automation原先在函式尾端手動釋放；現在正常離開、提前返回或unwind都由同一守衛處理。程式發生panic仍遵循既有保護規則，並非恢復業務流程。

## 驗證

獨立子程序依序驗證五種真COM情境：S_FALSE巢狀初始化、拒絕切換模式不影響原持有者、WIC借用STA、新／既有MTA的平衡、unwind釋放。隔離程序避免其他測試的MTA使未初始化執行緒獲得implicit MTA而污染斷言。

完整驗證仍須包含原生WebView2、WIC、Office保存重開、Outlook記憶體COM案例及NSIS。真公司信箱與公司加密Office另列待驗；詳見[0.9.5驗證紀錄](VALIDATION_0_9_5.md)。
