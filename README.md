# LM_AI 0.8.15 — Windows 工作助理

Rust 桌面程式，提供公司瀏覽器登入、文字／附件對話、串流、持久背景任務、選字快捷鍵、網站通知及 Classic Outlook 唯讀助理。
介面使用 WebView2、內嵌 HTML/CSS 與離線 Markdown 套件；不需 Node、Python 或外部 CDN。

## 0.8.15 登入入口與彈窗關閉

- 獨立「登入」按鈕搭配「尚未登入時無法使用其他功能」提示，登入成功後隱藏；有效的保存授權仍可沿用。
- 未登入時停用其他功能，包含 Outlook、VNC 與設定；前端及原生層同時檢查授權。
- 設定、確認、改名、VNC 機台管理與更新提示均可點外部關閉；確認動作視為取消，版本限制仍保留。
- 本版交付 EXE、NSIS 與兩份更新清單，驗證範圍見 [0.8.15 驗收](docs/VALIDATION_0_8_15.md)。

## 0.8.14 VNC 匯入預覽

- 分類與機台採自然排序，未分類最後；同名同 IP 標綠且不可勾選，IP 不同標紅供選擇。
- 保留登入供預覽重取，匯入／捨棄才登出；首次未分類達 50% 時自動重取一次，之後只接受手動重取。
- 使用者已確認測試成功，本版交付 0.8.14 EXE、NSIS 與兩份更新清單。操作見 [VNC 說明](docs/VNC_QUICK_CONNECT.md)，驗證見 [0.8.14 驗收](docs/VALIDATION_0_8_14.md)。

## 0.8.13 VNC 手動同步

- 啟用 VNC 後可記住網站帳密及自訂連結；僅按更新才登入、取得機台清單並登出，各請求前等待 300ms。
- 依分類或個別機台勾選匯入；空白 IP 仍可保存，新機台密碼為 1234，既有密碼及手動機台保留。管理加入多選移動／刪除及整類排序。
- 更新提示明確說明重新啟動 LM_AI 應用程式，不會重新啟動電腦。本版先交付 EXE 與 EXE 更新清單；NSIS 保留 0.8.12，待使用者實測後製作。
- 操作見 [VNC 使用說明](docs/VNC_QUICK_CONNECT.md)，驗證見 [0.8.13 驗收](docs/VALIDATION_0_8_13.md)。

## 0.8.12 統一完成結果格式

- 支援 `result.choices[0].message.content` 正文，以及同層的 `sections`、`citations`；背景與串流完成結果共用解析。
- 重點直接顯示，來源／信心／限制／引用可展開，複製及加密歷史保存完整內容。既有 answer payload 與舊格式備援保留。
- EXE、NSIS 及更新清單進版 0.8.12；驗證記錄見 [0.8.12 驗收](docs/VALIDATION_0_8_12.md)。

## 0.8.11 完成後保留回覆欄位

- 修正只有正文的完成結果覆蓋已收到的重點、來源、信心及限制；支援五段英文 Markdown 壓在同一行的回覆。
- 正文及回答重點直接顯示，其餘可點擊展開。完成正文若與串流不同，另存可展開原文；複製及歷史保存一併保留。
- EXE、NSIS 與兩份更新清單進版 0.8.11；驗證範圍見 [0.8.11 驗收](docs/VALIDATION_0_8_11.md)。

## 0.8.10 完整回覆顯示

- 新版優先讀取結構化正文、回答重點、來源、信心、限制與引用，修正完成後只剩正文的問題；背景與串流完成結果共用解析。
- 正文與回答重點直接顯示，來源、信心、限制及引用可展開；複製與本機歷史保存完整內容。舊格式依中英文編號章節辨識，避免把來源後的正文與重點一併藏起來。
- EXE、NSIS 與兩份更新清單進版 0.8.10，沿用三花貓圖示及既有安裝方式。詳見 [0.8.10 驗收](docs/VALIDATION_0_8_10.md) 與 [回覆契約](docs/STRUCTURED_REPLY_CONTRACT.md)。

## 0.8.9 三花貓圖示

- 使用新的三花貓圖案，統一更新 EXE、視窗、工作列、托盤、介面左上品牌，以及 NSIS 安裝／解除安裝圖示。
- EXE、NSIS 與兩份更新清單均為 0.8.9；沿用單純安裝、不檢查或附帶 WebView2 的流程。詳見 [0.8.9 驗收](docs/VALIDATION_0_8_9.md)。

## 0.8.8 附件選檔修正

- 修正 0.8.7 選完附件後因已移除按鈕的殘留引用而中斷、完全未開始上傳的問題。
- 接收附件期間鎖定目前的對話列操作，完成或失敗後立即恢復；失敗後可重新選取檔案。
- 補上完整選檔接收與失敗重試回歸測試。使用者確認後追加 0.8.8 NSIS 安裝包，固定安裝至 `C:\largan\LM_AI`，保留既有 VNC 設定；EXE、Setup 與兩份更新清單同版。詳見 [0.8.8 驗收](docs/VALIDATION_0_8_8.md)。

## 0.8.7 精簡導覽與 Outlook 說明

- 左上角使用與 EXE 相同的黑貓圖示，側欄保留 Outlook 助理與選用的 VNC；新對話移至最近對話標題旁。
- 每筆對話滑鼠移入或鍵盤聚焦時顯示編輯、置頂、刪除圖示；通知與任務改為右上角圖示，保留數量提示。
- Outlook 授權文字分行並以紅色標示完整內容匯出授權。進入助理時重查模型；品質模型停用時禁用自動補充內文，基本資訊分析仍可使用。
- 本輪只交付 EXE／EXE 更新清單／原始碼；NSIS 保留 0.8.5，待使用者確認介面後再打包。詳見 [0.8.7 驗收](docs/VALIDATION_0_8_7.md)。

## 0.8.6 選用 VNC 與介面修正

- 設定中可啟用預設隱藏的 VNC 快速連線，直接呼叫使用者安裝的 UltraVNC Viewer；沿用 EXE 旁的 Python `machines.json`／`user_config.json` 格式。
- 支援機台管理及手動上移／下移；不自動排序，編輯保留原位置。詳見 [VNC 說明](docs/VNC_QUICK_CONNECT.md)。
- Outlook 日期查詢預設改為目前資料夾及子資料夾，移除所有信箱選項；模式與快捷鍵提示同列，提示依設定更新。
- 本次只交付 EXE、EXE 更新清單與原始碼；NSIS 仍為 0.8.5，離線 ZIP 仍為歷史版本。驗證與部署界線見 [0.8.6 驗收](docs/VALIDATION_0_8_6.md)。

## 0.8.5 公司固定路徑安裝

- EXE、NSIS 安裝／解除安裝程式、已安裝應用程式及關於頁面統一顯示 `Largan, Inc.`。
- `dist/LM_AI_Setup.exe` 固定安裝至 `C:\largan\LM_AI\`，自動逐層建立缺少的資料夾；已存在時沿用，保留未知檔案。不可用 `/D` 改變位置。
- 安裝與主程式維持一般使用者權限，捷徑／解除安裝登錄屬於目前使用者；若公司限制目錄寫入，請 IT 配置權限。
- 交付原始碼、EXE、NSIS、兩種更新 JSON 及驗證文件；既有離線 ZIP 仍是歷史版本。本版部署、舊路徑遷移及驗證界線見 [0.8.5 驗收](docs/VALIDATION_0_8_5.md)。

## 0.8.4 介面整理

- 移除時間估算按鈕、估時請求，以及附件／任務的預估時間；保留實際狀態、進度與排隊順位。
- 狀態說明與「一般／背景處理」排在同一列，長文字在可用空間內換行。
- 設定面板略微加寬，控制項可收縮與換行，消除水平捲軸；單選圓圈與選項文字並排。
- EXE、更新 JSON、原始碼與驗證文件一併交付；不重製安裝包或離線 ZIP。驗證範圍見 [0.8.4 驗收](docs/VALIDATION_0_8_4.md)。

## 0.8.3 隔離內網連線

- 固定公司來源 `http://lp2-en-server:80` 改用 WinHTTP 直接連線，避免先初始化自動代理；模型、登入、聊天、附件、更新與通知共用規則。
- 只對完整公司 origin 與既有 loopback 生效，其他主機仍使用自動代理；不修改 Windows 設定，不需連外網或執行外部腳本。
- 針對 0.8.2 已定位的 `10022（WinHttpOpen）` 提供修正候選；公司電腦仍需實測，尚未確定自動代理元件失敗的根因。
- 交付 EXE、JSON 與原始碼，不重製安裝包或 ZIP；驗證紀錄見 [0.8.3 驗收](docs/VALIDATION_0_8_3.md)。

## 0.8.2 連線診斷

- 網路錯誤會指出申請登入碼、查詢授權或取得模型清單，以及失敗的 WinHTTP API；版本檢查亦保留錯誤詳情。
- 開啟預設瀏覽器失敗時顯示 ShellExecuteW 回傳碼，與伺服器連線錯誤分開辨識。
- 本版協助定位部分公司電腦的 10022；尚未確認該問題根因，不宣稱已修復。代理設定與登入協定維持原有行為。
- 交付 EXE、簽署 JSON 與原始碼，不製作安裝包或 ZIP。驗證界線見 [0.8.2 驗收](docs/VALIDATION_0_8_2.md)。

## 0.8.1 更新

- 同時讀取 EXE／NSIS 簽署清單，通過驗證後選最高版本；同版優先 EXE。
- EXE 下載後提供手動更換說明與開啟資料夾按鈕；較新 NSIS 保留安裝並重新啟動流程。
- 僅交付 `dist/LM_AI.exe` 與 `dist/update-manifest-exe.json`，停用 `CompanyAI.exe` 相容檔名。
- 本次不重製安裝包與離線 ZIP；儲存庫原有包仍屬 0.8.0，不能用於重建或驗收 0.8.1。
- 網站部署與選版規則見 [0.8.1 契約](docs/UPDATE_0_8_1_CONTRACT.md)，驗證界線見 [0.8.1 驗收](docs/VALIDATION_0_8_1.md)。

## 0.8 既有功能

- 安裝、更新與解除安裝改為 NSIS 原生流程，使用者端不呼叫 CMD／PowerShell／外部腳本。
- 一般更新先同意下載，再另行同意安裝重啟；退出不自動安裝。
- 低於最低版本時鎖住功能；已知門檻跨重新啟動保留，完成更新才解除。
- 內建公鑰驗證更新簽章；完整 Setup 與 `dist/update-manifest.json` 成對發布。
- 網站須更新原本下載頁的 JSON 回應，詳見 [0.8 更新契約](docs/UPDATE_0_8_CONTRACT.md)。
- 本機驗證範圍見 [0.8 驗收](docs/VALIDATION_0_8.md)。公司防毒仍須對正式安裝包實測。

## 0.7 更新

- 一般模式採 SSE 原始事件，背景模式保留持久任務；修正 WinHTTP 等待緩衝區造成的小事件延遲，保留工具狀態供展開查看。
- 防止重複啟動；關閉視窗縮到托盤，再次執行即可叫出。可設定叫出時開新對話，草稿與既有對話會保留。
- 選字快捷鍵可停用，另有預設關閉的浮動 AI 圖示；可設定 Enter 送出／換行、簡單任務用快速模型或維持目前模型。
- 本機對話可改名、置頂；獨立快速模型任務產生最多 15 字標題，手動改名優先。
- Outlook 初篩走背景，接受回答中夾帶 JSON；無法安全解析仍顯示原文，不執行未通過授權檢查的命令。
- 新增每位使用者安裝／解除安裝與更新助手，檔案資訊採 LARGAN、LM_AI、公司 AI 助理（Dev: 1230783）。0.8 已改為 NSIS 與內嵌公鑰更新驗證。

網站須配合 [0.7 契約](docs/DESKTOP_0_7_CONTRACT.md)。建置與公司實機驗收的界線見 [0.7 驗收](docs/VALIDATION_0_7.md)。

## 0.6 既有功能

- 卡住的任務可「停止追蹤」或「移除任務」，404／離線也能解除本機等待；另行嘗試 server 取消，對話保留。
- 切換模型重新查詢附件能力，保留不相容附件並提示原因，避免把圖片送到不支援的模型。
- 全站鈴鐺與 AI 通知共用通知中心。網站通知的已讀／刪除走 Token API，成功後才更新；AI 僅顯示成功回覆／失敗。App 前景時不跳系統氣泡。
- 設定可點視窗外部關閉，關閉時亦結束快捷鍵錄製。
- Outlook 支援多選及收件匣「所有／未讀 × 今天／三天內／本週」。確認後先傳基本資訊；可授權依 AI 請求自動匯出／上傳 MSG 並接續整理，不必手動存檔。

網站須配合 [0.6 契約](docs/DESKTOP_0_6_CONTRACT.md)。實際 Outlook／公司鈴鐺驗收與限制見 [0.6 驗收](docs/VALIDATION_0_6.md)。

## 直接使用

1. 直接執行 `dist/LM_AI.exe`。從 0.8.0 升級時先從托盤離開舊版，再手動更換檔案；舊版更新器尚不支援 EXE 清單。
2. NSIS 只安裝 LM_AI，不檢查或安裝 WebView2，一般安裝完成後請自行使用捷徑開啟。如果主程式啟動時提示缺少 WebView2，請安裝公司另外提供的 `MicrosoftEdgeWebView2RuntimeInstallerX64.exe`，再開啟 LM_AI。Git 的 `dist/` 保留此獨立安裝程式，詳見 [離線安裝說明](dist/WEBVIEW2-OFFLINE.md)。
3. 按左下齒輪 → 瀏覽器登入，核對短碼並允許授權；登入最長保存 30 天。
4. 選擇後端提供的「快速／品質」等模型，輸入文字並送出。預設 Enter 換行、Ctrl+Enter 送出；設定可改為 Enter 送出、Shift+Enter 換行。
5. 使用者訊息靠右、AI 靠左；支援表格、程式碼高亮／複製、數學公式、註腳、任務清單與一般 Markdown。

側欄可收合；齒輪設定包含登入／登出、字體大小（預設 14 px，可調 12–20）、快捷鍵、通知及更新。
輸入框會隨內容長高，上限 160 px。AI 回覆時，停在底部就跟隨新內容；往上閱讀時保留位置，可按「查看最新回覆」跳到底。
公式使用 KaTeX 支援的 TeX 語法，包括 `$...$`、`$$...$$`、`\(...\)`、`\[...\]`；不是完整 LaTeX 文件編譯器。
原始 HTML 不執行，遠端圖片只顯示替代文字；外部 HTTP(S) 連結經確認後用瀏覽器開啟。

## 附件、串流與背景任務

網站須部署持久任務、capabilities 與 [0.7 契約](docs/DESKTOP_0_7_CONTRACT.md)，登入會取得支援模式與附件規則；缺少支援時提示，不退回同步請求。
選「一般」逐段閱讀串流，或「背景處理」先取得任務，完成後收到提示。按迴紋針選檔，也可直接在輸入框貼上圖片。
文件與圖片合計最多 20 個；副檔名、單檔及合計大小由後端提供，不需為一般規則調整更新 EXE。
附件先上傳網站轉換，卡片顯示排隊／轉檔／可送出／失敗；全部就緒後才可送出 AI 分析。桌面不處理 MD 轉換或 OCR。
0.8.4 起不再估算完成時間；附件及任務卡片仍顯示伺服器回報的實際狀態、進度及排隊順位。

任務面板可查看狀態、回到對話或取消。可同時處理不同對話，同一對話先等當前工作結束；最多追蹤 8 個未完成聊天工作。
串流斷線保留片段、改查原任務；退出後重開並登入同帳號，也會查回結果。重試沿用 request_id，不直接新建重複工作。
最小化自動進入系統托盤，右鍵可還原或離開。離開 App 不會取消伺服器已接受的任務；退出期間沒有桌面即時通知。
詳見 [0.5 使用與驗收說明](docs/VALIDATION_0_5.md)。EXE、視窗、工作列及托盤已統一使用使用者提供的貓咪圖示；原圖、多尺寸 ICO 與重建方式見 [圖示說明](assets/README.md)。

## 選取文字

保持 LM_AI 開啟，在一般權限的來源程式選取文字，按 **Win+Esc** 並放開按鍵；文字會接到草稿後面，確認後再按送出、翻譯、摘要或潤飾。
快捷鍵不自動送出、不監聽一般按鍵；可在設定修改或停用。浮動 AI 圖示是獨立選項，啟用後只查 UI Automation 選區位置，按圖示才複製文字；不支援 TextPattern 的程式不顯示。沿用一般 Ctrl+C 行為，會改變剪貼簿，不保留原圖片或富文字。
草稿上限 16,000 個 UTF-16 code units；擷取失敗、送出 Ctrl+C 前來源變動或內容過長時保留原稿。某些編輯器未選字也會複製整行，送出前請核對。

快捷鍵設定可直接點輸入框／錄製，按下組合鍵後再套用；既有快捷鍵偏好保留，升級後可自行錄製 Win+Esc。PDF 相容流程與限制見 [0.4.1 快捷鍵說明](docs/HOTKEY_0_4_1.md)。

## 通知與 Classic Outlook

- 通知中心顯示網站事件；使用 WebSocket 喚醒及每 60 秒 REST 補查，支援已讀、去重、期限與加密快取。
- Windows 提示只在 App 不在前景時顯示，不搶焦點，點擊可開啟通知／任務面板。最小化及關閉視窗均保留在系統托盤；托盤選「離開」才真正退出，不另裝背景服務。
- Classic Outlook：選取一封信，按「讀取選取郵件」先預覽基本資訊；確認分析才送給 AI。
- 單封手動模式的正文需另外勾選並確認，不含附件；多封自動模式則依本批授權匯出 MSG。不寄信、刪信、移動或修改未讀狀態。
- 單封手動預覽仍需另外確認正文；多封自動模式使用本批授權，僅能匯出勾選的郵件。Skill 與工具由 App 限制，不寄信、不修改信箱。

## 本機保存與固定服務

偏好位於 `%LOCALAPPDATA%\CompanyAI\settings.json`；延續舊資料夾名稱以保留升級相容。
Token、對話與通知分別為 `session.dpapi`、`history.dpapi`、`notifications.dpapi`，以目前 Windows 使用者的 DPAPI 加密。
對話最多 200 個、每個最多 20 輪、未加密資料合計最多 32 MB；成功回覆後保存。磁碟錯誤會標示未保存，損毀原檔不被空紀錄覆寫。
草稿在切換對話、縮到托盤與正常退出時保存；未送出的郵件預覽只在記憶體。登出保留本機歷史，但新登入從新對話開始；再次送出舊對話才會把它交給目前帳號。
右上刪除可移除目前對話。登出只清除本機 Token，不等於伺服器撤銷。

公司主機、聊天、登入、版本、模型、下載與通知路由固定且不提供 UI 編輯。
聊天維持 `Authorization: Bearer <個人 Token>` 與 model alias。一般使用 `stream:true`、`execution_mode:"stream"`；背景使用 `stream:false`、`execution_mode:"background"`。
版本及模型啟動時、登入後、每 5 分鐘與手動重新整理時查詢。低於 minimum_version 才強制更新；僅低於 latest_version 可繼續使用。
版本檢查失敗暫時允許使用，但同次執行已確認的強制更新不因斷線解除。模型或登入失敗仍需處理，不能藉版本暫用政策略過。
網站提供更新清單且目前／新版 EXE 均通過相同發行者的簽章驗證後，可背景下載、立即重啟或退出時套用。未簽章版本需由 IT 手動部署。依公司指定目前採內網 HTTP，沒有 TLS 傳輸加密；SHA256 不能取代簽章，將來切 HTTPS 需同步更新固定設定。

## 後端契約與驗證

- [DESKTOP_0_7_CONTRACT.md](docs/DESKTOP_0_7_CONTRACT.md)：SSE、用途旗標、Outlook 容錯與更新清單。
- [VALIDATION_0_7.md](docs/VALIDATION_0_7.md)：本次自動檢查與待完成的公司實機驗收。

- [DESKTOP_0_6_CONTRACT.md](docs/DESKTOP_0_6_CONTRACT.md)：全站鈴鐺、模型規則與 Outlook MSG。
- [VALIDATION_0_6.md](docs/VALIDATION_0_6.md)：新版測試與公司驗收。

- [DESKTOP_0_5_CONTRACT.md](docs/DESKTOP_0_5_CONTRACT.md)：附件、模式、持久任務與通知的歷史契約；估時功能於 0.8.4 移除。
- [VALIDATION_0_5.md](docs/VALIDATION_0_5.md)：新版操作、本機測試與公司驗收。
- [WEB_INTEGRATION.md](docs/WEB_INTEGRATION.md)：既有登入、聊天、版本與模型契約。
- [NOTIFICATIONS_AND_OUTLOOK.md](docs/NOTIFICATIONS_AND_OUTLOOK.md)：本次網站需新增的通知 API、WebSocket 與 Outlook 分析格式。
- [VALIDATION_0_4.md](docs/VALIDATION_0_4.md)：本機驗證範圍與公司驗收步驟。
- [BACKEND_ROADMAP.md](docs/BACKEND_ROADMAP.md)：已完成與後續範圍。

本次未加入 UNC 知識庫、skills 工具呼叫、RAG、桌面 OCR 或寄信。

## 本機示範

```powershell
.\dist\LM_AI.exe --demo
```

齒輪 → 登入，在本機頁面允許授權後可測附件狀態、串流、背景任務、通知與已讀。模擬後端不做真實轉檔，狀態僅保存在記憶體。Outlook 助理會使用虛構郵件，不讀真實信箱。
只監聽 loopback，所有回覆明示本機模擬；不連公司服務。一般使用不需要 Rust 或 Visual Studio。

## 使用 VS Code 編輯

請以 VS Code 的「開啟資料夾」開啟本專案 `CompanyAI`，並使用 `rust-lang.rust-analyzer` 擴充套件。
第一次使用或搬移專案後，先在專案根目錄執行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Configure-VSCode.ps1
```

接著在 VS Code 按 `Ctrl+Shift+P`，執行 `Developer: Reload Window`（重新載入視窗）；既有終端機請關閉後重開。
腳本會讓 rust-analyzer 與新終端機取得正確的 Rust / MSVC 環境，解決 `cargo metadata ... program not found`。
已編譯的 EXE 可獨立執行；編輯器的提示、跳轉及檢查則需要 Cargo、rustc 和 rust-src。
電腦專用設定存於 `.vscode/settings.json`，不提交 Git 或放入離線 ZIP；包內含相同設定腳本，換電腦後重新產生即可。
如果設定檔含有 PowerShell 5.1 不支援的 JSON 註解，腳本會停止並保留原檔，請先備份再手動合併設定。

## 編譯與驗證

本專案根目錄為 `Y:\Rust\Project\CompanyAI`。以下指令都在這個資料夾執行：

```powershell
cd Y:\Rust\Project\CompanyAI
```

原始碼、Cargo.lock、編譯設定、target 與 dist 都屬於此專案。
`scripts\Enter-DevShell.ps1` 優先使用本專案的 `toolchain`，否則使用外層 `Y:\Rust\.tools`；套件由本專案 `vendor` 提供。
拿到 Git 專案後，要離線編譯請將 `offline\CompanyAI-offline.zip` 解壓到新資料夾，並在解壓後的根目錄執行下列指令。
包內包含 Rust 工具與依賴，不需要使用家裡的共用資料夾；公司電腦仍須預先安裝下列 MSVC 與 SDK。

固定 Rust/Cargo 1.98.1、`x86_64-pc-windows-msvc`、MSVC 14.29.30133（v142）、SDK 10.0.19041.0。
公司完整的 MSVC 修補版本與 SDK 版本仍待核對。

```powershell
.\scripts\Build.ps1
```

腳本會執行格式檢查、Clippy、測試、release 編譯、WebView2 介面自我檢查、DLL 依賴檢查，最後更新 `dist\LM_AI.exe`。
完整編譯記錄存於 `offline\environment.txt`。

若 Windows PowerShell 的執行原則阻擋腳本，可單次使用：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Build.ps1
```

開發時先載入專案環境，再使用 Cargo：

```powershell
. .\scripts\Enter-DevShell.ps1
cargo run --frozen -- --demo
```

環境預設離線，透過 `vendor` 與 `--frozen` 建置；執行 EXE 不需要開發工具。

## 離線交付與 Git

本資料夾包含原始碼、`dist\LM_AI.exe`、`offline\CompanyAI-offline.zip` 及相關文件／校驗記錄。
完整 ZIP 含相同版本的原始碼、EXE、Rust 工具鏈及所有依賴，方便整包帶到公司。

修改完成後，使用以下指令更新交付檔案並驗證全新解壓後的離線編譯：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Prepare-Delivery.ps1
```

Git LFS 規則已寫入 `.gitattributes`；遠端儲存庫為 [CompanyAI](https://github.com/you1230783-sys/CompanyAI)，主要分支為 `main`。
完整目錄說明、離線操作、校驗與 Git 工作流程見 [OFFLINE_AND_GIT.md](docs/OFFLINE_AND_GIT.md)。

## 開發用瀏覽器測試

真正瀏覽器的開發用整合測試：

```powershell
cargo run --example browser_smoke --frozen
```

開啟輸出的本機網址並按允許。程式會驗證 Token、DPAPI、中文訊息回覆及登入碼不可重複使用，成功時印出 PASS。

## 程式碼位置

| 檔案 | 責任 |
| --- | --- |
| `src/main.rs` | EXE 入口、--demo、--self-check |
| `src/ui.rs` | 控制項、草稿、使用狀態與背景結果 |
| `src/appearance.rs` | 原生介面配色、字型與卡片繪製 |
| `src/config.rs` | 固定路由、偏好序列化、同來源驗證 |
| `src/service.rs` | 版本門檻、失敗政策、模型清單 |
| `src/selection.rs` | 全域快捷鍵、明確觸發的複製、剪貼簿邊界 |
| `src/auth.rs` | 登入輪詢、帶 Token 的聊天請求 |
| `src/protocol.rs` | Chat Completions、登入 JSON 與錯誤提示 |
| `src/transport.rs` | WinHTTP、TLS、代理與逾時 |
| `src/storage.rs` | 偏好保存與 DPAPI |
| `src/demo.rs` | 本機模擬網站及往返測試 |

維護時遵循 [AGENTS.md](AGENTS.md)：簡單易懂、繁體中文註解充足、方便接手修改。
