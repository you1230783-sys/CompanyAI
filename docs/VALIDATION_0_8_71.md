# 0.8.71：CAD 註記更新、版本解析與額度續接

## 本輪範圍

使用者要求小部分／針對性驗證後編譯、NSIS 打包及推送；完整驗證延後。本文件不沿用 0.8.70 的完整 PASS，也不代表公司服務或完整模型流程已驗收。

- ICAD2008 目前圖面支援 TEXT／MTEXT 原位改字，包含引線附屬 MTEXT，保留引線本體。
- 已讀、無文字覆寫且可核對端點的平面直徑標註，使用 `diameter=目標直徑` 更新真實量測端點。為讓原生 Undo 完整還原，在同圖替換該標註，結果以 `replacement_handles` 回傳舊→新 Handle；後續須使用新 Handle。不是新增另一張圖，也不是以文字覆寫偽造尺寸。一般尺寸、關聯／複合物件及任意新增／刪除沒有因此開放。
- 改圓與改標註／備註仍是明確的不同操作，不猜測文字與孔的關係或自動全圖取代。圖層鎖、文件／revision 核對、未指定圖元保護、未儲存及一次 Undo 契約保留。
- `cad_plan_start` 內部查本機安裝清單，可解析 `latest`／`auto`、年份或確切 ProgID。使用者當輪指定優先，模型可沿用專案記憶中的偏好作為版本參數。選擇回傳 `selected_cad` 並固定於計畫；指定版本不存在／年份有歧義時拒絕，不偷偷換版。選擇階段的安裝確認與執行階段的連線分開，執行時仍核對 CAD 身分。直接 `cad_draw` 仍只接受確切 ProgID。
- 已核對回覆身分、確定收到 `finish_reason=length` 且沒有可執行操作時，最多自動續接 **5 次**，這是原始失敗後的五次額外推論。空白及截斷的工具參數均不執行；下一輪提示保留原需求／已保存區段，只交下一小步，尺寸／引線可拆成 1–3 項。不是重播 CAD 工具，也不會將未知提交當作新推論重送。
- 續接計數隨 checkpoint 保存，重新讀技能／指南或 inventory 不清除預算；實際新增有效進展才重設。仍受既有取消、時間、往返與工具上限約束。五次耗盡後暫停，手動「繼續」才重開該段預算。提示中的數字來自服務公告的預設額度，不冒稱為本輪實際 tokens 用量。

## 針對性證據

- Rust：25 項進度契約、6 項分段計畫契約、Broker 暫存還原定向回歸通過，涵蓋五次續接／第六次停止、checkpoint 還原、讀指南不重設、有效區段恢復進度、版本選擇、指定不存在不回退及年份歧義拒絕。
- Python：88 項 CAD 契約通過，包含長繁體 MTEXT 字串以字碼頁完整字元分段、直徑端點縮放、字串跳脫、原生模板結構及未支援操作拒絕。
- 真實 ICAD2008 私有新圖：先建兩圓、直徑尺寸、說明、引線與標題，存檔重開；先將圓由 Ø60／Ø12 改成 Ø80／Ø15，再更新直徑尺寸、引線附屬文字、備註及含「許餐」的繁體標題。新直徑實際端點量測為 80，顯示文字含 80；文字內容正確。只用一次原生 Undo，所有圖元、完整原生 revision 及文字回到註記修改前，圓保留先前已修改值；磁碟 DWG SHA256 不變。來源為最後鎖定 CAD runtime，沒有修改使用者目前圖面。
- 早期候選直接改尺寸端點，雖顯示／幾何可更新，Undo 後唯讀 DXF 42 快取仍為 80；重繪及一併寫 42 亦未解決，未接受那些候選。最後改成同圖替換此尺寸，原物件可由 Undo 完整還原；所有失敗證據保留於本機。
- 本輪沒有真模型重跑控制盒提示詞、外觀目視驗收或2022實機回歸，沒有完整 workspace／專案長流程、真 Office／VNC／SMB 或安裝生命週期重驗。自動提示能否改善公司模型輸出，仍需使用者實測。

DXF 格式依據：[直徑標註群組碼](https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-165A992D-9017-4C1E-B8CC-E70A17191BFE.htm)、[MTEXT 群組碼](https://help.autodesk.com/cloudhelp/2020/ENU/AutoCAD-DXF/files/GUID-5E5DB93B-F8D3-4433-ADF7-E92E250D2BAB.htm)。規格文件不能代替上述 ICAD2008 實機核對。

## 建置與封裝

`Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller` 已完成，程序退出 0，`offline/exe-verification.json` 為本版 `PASS_BASIC`。實際 MSVC v142 14.29.30133、cl `_MSC_FULL_VER=192930159` x64、link14.29.30159；空 Cargo 快取與 frozen、fmt、Clippy、基本篩選測試、release、88 項 CAD 契約、原生 runtime／WebView2 自檢及唯讀來源整合通過。NSIS 已打包，兩份更新清單的版本、大小、SHA256 一致，正式原生套件簽署驗證通過。

本輪未執行 NSIS 安裝／更新／解除安裝生命週期；歷史 `offline/installer-verification.json` 為0.8.70紀錄。原生定向結果另見 `offline/cad-annotation-verification-0.8.71.json`。

| 交付檔 | bytes | SHA256 |
| --- | ---: | --- |
| `dist/LM_AI.exe` | 19638272 | `84a38d7d1509f7d734cc64ea5a5faa35f1f73a7926ff490e37b86bcc05ffbbfa` |
| `dist/LM_AI_Setup.exe` | 36352648 | `c5d8de1ca495329968e441d9eb753960cd79b8b8bbea7710a58ce10093f6f648` |

歷史完整 Rust 離線 ZIP、雙版住宅範例及其歷史驗證維持原版，不稱為 0.8.71 的完整離線包或新模型驗收。
