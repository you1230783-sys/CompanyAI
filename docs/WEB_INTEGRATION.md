# LM_AI：網站／API 串接契約

> **0.8.1 更新流程以 [UPDATE_0_8_1_CONTRACT.md](UPDATE_0_8_1_CONTRACT.md) 為準**：支援 EXE／NSIS 簽署 JSON，合法最高版本優先、同版選 EXE。EXE 手動更換，NSIS 保留原生安裝；聊天與 SSE 以 0.7 契約為準。

> **0.6 新功能請見 [DESKTOP_0_6_CONTRACT.md](DESKTOP_0_6_CONTRACT.md)**：依模型的附件規則、全站鈴鐺同步／已讀／刪除及 Outlook MSG 自動補充。本頁以下保留既有相容契約。

> **0.5 新功能請先實作 [DESKTOP_0_5_CONTRACT.md](DESKTOP_0_5_CONTRACT.md)**：附件規則路由、文件／圖片上傳、串流、持久背景任務與通知（歷史估時功能已於 0.8.4 移除）。此處保留既有登入／版本／模型／通知基礎；驗證範圍見各版驗收文件。

## 0.8.12 統一完成結果格式

使用者確認背景模式原先未由網站完整回傳，並指定串流／背景的最終 `result` 統一為 Chat Completions 物件。桌面新增支援：正文取 `result.choices[0].message.content`，重點／來源／信心／限制取 `result.sections`，引用取 `result.citations`。兩模式完成後均經既有 TaskStatus 解析及保存，不依賴串流原文補齊。外層 task_id／client_request_id／state 與 POST／查詢路由維持原契約，result.id 不能取代任務識別碼。

沿用先前授權打包並推送，進版 0.8.12；EXE／NSIS 各與對應更新清單成對部署。網站可公告 latest_version 0.8.12，不需提高最低版本。詳見 [回覆契約](STRUCTURED_REPLY_CONTRACT.md) 與 [驗收記錄](VALIDATION_0_8_12.md)。Git 推送不等同內網部署。

## 0.8.11 完成回覆保留修正

0.8.10 對只有 answer 的 payload 直接採用，可能忽略同回覆更完整的 content；完成結果也可能覆蓋串流全文。0.8.11 保留結構化非空欄位優先，但在正文相符時從較完整的結果或同任務已收到的完整五段文字補空欄位。標題與內文同列、五段英文 Markdown 壓成單行皆支援；無法安全合併的原文放進可展開區，隨 DPAPI 歷史及複製文字保存。詳見 [回覆契約](STRUCTURED_REPLY_CONTRACT.md)。

接續先前打包推送授權，EXE、NSIS 與兩份更新清單進版 0.8.11；網站可公告 latest_version 0.8.11，不調高最低版本。檔案與其清單須成對部署，Git 推送不等於內網已部署。驗證範圍見 [0.8.11 驗收](VALIDATION_0_8_11.md)。

## 0.8.10 結構化回覆與章節收合修正

桌面舊格式顯示改為按編號章節辨識中英文正文、回答重點、來源、信心與限制；標籤依使用者提供的網頁規則，只收合後三者，避免把 Sources 後再次出現的正文或回答重點一起藏起來。沒有編號或格式無法確認時維持完整 Markdown，原始訊息不刪減。串流與完成訊息共用相同顯示規則。規則與測試範圍見 [介面與串流](UI_AND_STREAMING.md)。

依使用者提供的背景回覆範例，桌面新增結構化優先解析：正文 `answer`、字串陣列 `sections.key_points / sources / limitations`、字串 `sections.confidence`、陣列 `citations`；無有效結構化欄位時才解析舊格式。支援結果直接是 payload，以及任務／result／message 上的 `response_payload_json` 物件或 JSON 字串。修正原先只讀 `result.choices[0].message.content` 而漏掉其他欄位的問題。背景與串流完成共用同一解析，串流 delta 及任務 REST 路由不變；完整欄位保存到本機歷史，也包含在複製回答與後續對話文字中。外層 `confidence: medium` 不覆寫 `sections.confidence: High (高)`。詳細支援形式及非顯示欄位界線見 [結構化回覆契約](STRUCTURED_REPLY_CONTRACT.md)。

使用者已要求打包並推送。本次進版 0.8.10，使用 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 交付 `LM_AI.exe`／`update-manifest-exe.json` 與 `LM_AI_Setup.exe`／`update-manifest.json`；各組檔案必須成對部署。網站可公告 `latest_version: 0.8.10`，本次修正不需要提高 minimum_version。既有同版本優先選 EXE 的規則不變：若要使用 NSIS 自動安裝，所有探索來源應只公告新版 NSIS 清單，不能只更換下載檔。驗證範圍見 [0.8.10 驗收](VALIDATION_0_8_10.md)。歷史離線 ZIP 不重製，Git 推送不等於內網網站已部署。

## 0.8.9 三花貓圖示發行

本版只更新使用者提供的圖示並進版 0.8.9，不改動聊天、附件、Outlook 或 VNC API。交付 `LM_AI.exe`／`update-manifest-exe.json` 與 `LM_AI_Setup.exe`／`update-manifest.json`，每組檔案必須成對部署，網站可公告 `latest_version: 0.8.9`，一般圖示更新無須提高最低版本。

同版本仍優先選 EXE；若要使用 NSIS 自動安裝流程，所有可探索的更新來源只能公告本版 NSIS，EXE 清單不公開或保留舊版。NSIS 延續只安裝、不檢查或附帶 WebView2，一般安裝完成後手動開啟主程式。驗證見 [0.8.9 驗收](VALIDATION_0_8_9.md)，歷史離線 ZIP 不更新。

## 0.8.8 附件接收修正

使用者實測後追加 0.8.8 NSIS 發行，`dist/LM_AI_Setup.exe` 與 `dist/update-manifest.json` 必須成對部署；固定安裝至 `C:\largan\LM_AI`，保留既有機台設定。獨立 EXE 與其更新清單也維持 0.8.8。既有雙格式更新選擇規則不變：同版本兩種格式同時被公告時，桌面優先選 EXE（手動替換）；需要 NSIS 安裝流程時，網站所有可探索的清單來源應只公告該版 NSIS，EXE 清單不公開或保留較舊版，不能只修改 download 路由卻保留同版 EXE 清單。Git 上傳本身不會部署內網網站。

0.8.7 移除右上刪除按鈕後，附件接收中的鎖定流程仍引用舊 DOM ID，造成選檔後尚未發出 `file_begin` 就中斷並停留在忙碌狀態。0.8.8 改為鎖定對話列控制項，並在接收結束／失敗時依目前狀態還原；初始畫面更新亦納入 finally 清理範圍。附件 API 不變，不需因本次錯誤修改後端 job_id 或路由。

新增 WebView2 回歸案例：已有文字對話後選檔、跨區塊原始內容一致性、接收期間停用對話操作、成功後立即恢復、原生拒絕後中止及重新選取。同時保留原生 loopback 的預約附件／PUT 原始內容／查詢狀態／帶 token 聊天測試；尚不能代替公司服務實測。

### 0.8.8 精簡安裝包與 WebView2 分開提供

最新 `LM_AI_Setup.exe` 只安裝 LM_AI，不偵測、不嵌入或安裝 WebView2；公司另行提供 Git `dist/MicrosoftEdgeWebView2RuntimeInstallerX64.exe` 至內網下載區。一般安裝完成頁不提供自動開啟，使用者安裝後自行使用捷徑啟動。主程式沿用原有 WebView2 初始化失敗提示，缺少 Runtime 時先補裝再開啟 LM_AI，不必重跑 Setup。先前 Setup 的 Runtime 退出碼 2 阻擋流程已移除。

主程式維持 0.8.8；部署時必須一起替換 Setup 與新簽署的 NSIS 清單，清除舊下載快取，避免同檔名舊完整包與新 SHA256 不符。已安裝 0.8.8 的用戶不需要為拆包重新更新。獨立 Runtime 與主程式更新清單分開提供。

SFX 測試包因使用者要求已從最新 Git 移除。既有 NSIS 更新協定仍保留 `/UPDATEPID`／`/RESTART`：只有使用者在 App 同意安裝並重新啟動時才自動重啟。一般手動安裝不啟動主程式；API／簽署格式不變。

## 0.8.7 Outlook 模型可用性與介面整理

每次進入 Outlook 助理時重新取得既有模型清單，依穩定代號 `quality` 判斷自動補充內文是否可用，不以顯示名稱猜測。網站停用模型時應從模型清單移除該代號；桌面不讀取網站 env。未登入、查詢中或查詢失敗時不可啟用自動補充；成功查詢但沒有 quality 時顯示「目前品質模型維護中，暫時停用自動補充內文功能」。恢復供應後由使用者重新勾選，不自動恢復先前被清除的授權。

初篩仍可只送基本資訊；選用自動補充時，確認視窗分行並以紅色標示完整內容匯出授權。確認視窗開啟期間模型若停用，送出前再次阻擋，原生層亦檢查查詢狀態及 quality 代號，最終整理沿用既有能力與模型檢查。API 路由、附件 token、MSG 與模型請求格式不變。

左上品牌沿用嵌入的 EXE 黑貓 ICO；側欄新對話移至最近對話旁，各對話列提供滑鼠／鍵盤可用的編輯、置頂、刪除圖示，右上僅保留通知與任務圖示及狀態。EXE／EXE JSON 進版 0.8.7，NSIS 0.8.5 與歷史離線 ZIP 不重建。

## 0.8.6 EXE 發行與選用 VNC

一般／背景模式與輸入快捷鍵提示置於同一列，工作狀態另列；換行、送出與選字快捷鍵提示依已套用的本機偏好更新，停用選字快捷鍵時隱藏該提示。Outlook 日期查詢預設為目前資料夾及子資料夾，介面移除所有信箱範圍。

本次只發布 `dist/LM_AI.exe` 與 `dist/update-manifest-exe.json`，網站將 EXE／JSON 成對部署，`latest_version` 可改為 `0.8.6`；一般更新不必提高 `minimum_version`。版本較舊的 NSIS 清單可維持 0.8.5，不能將舊 Setup 改標為 0.8.6。既有 0.8.1 以上客戶端會依版本選擇新版 EXE，下載後仍需使用者手動替換；0.8.0 客戶端不支援 EXE 更新。驗證界線見 [0.8.6 驗收](VALIDATION_0_8_6.md)。

另新增選用的本機 VNC 快速連線：設定 `vnc_enabled` 預設 false，使用者啟用後才顯示側欄入口。設定檔固定沿用 LM_AI.exe 旁的 `machines.json` 與 `user_config.json` 原格式；機台只以使用者手動上移／下移調整順序，不自動排序。Rust 直接呼叫已安裝的 UltraVNC Viewer，不新增網站 API，也不將機台、密碼或連線操作交給 AI。規格與驗證見 [VNC 快速連線](VNC_QUICK_CONNECT.md)。

## 0.8.5 NSIS 固定位置發行（既有安裝包）

- 本版交付 `LM_AI_Setup.exe` 與 `update-manifest.json`，以及 `LM_AI.exe` 與 `update-manifest-exe.json`；版本均為 0.8.5。
- 供應商顯示 `Largan, Inc.`，安裝位置固定 `C:\largan\LM_AI\`，一般使用者執行，缺少的資料夾自動逐層建立。無新增聊天／登入 API。
- **若要讓既有 0.8.1～0.8.4 用戶透過更新取得 NSIS，網站此次只發布 0.8.5 NSIS 清單**：`/desktop/download` 回傳 NSIS 清單，獨立 EXE 清單路由保留舊版或回 404。同時提供兩種 0.8.5 清單仍會依既有契約優先選 EXE，不能只改安裝包檔名或 JSON 的 kind。
- Git 保留兩種已簽署 JSON 供部署選擇；此次上傳 Git 不代表已部署公司網站。`latest_version` 可設 0.8.5，無須因此提高 `minimum_version`。
- 舊安裝路徑處理與測試界線見 [0.8.5 驗收](VALIDATION_0_8_5.md)。

## 0.8.4 移除估時與介面整理

- 桌面移除估時命令、背景請求與顯示，不再呼叫 `POST /lm_server/api/desktop/chat/estimate`，亦不再為估時預先建立伺服器對話。
- `capabilities.timing_estimates`、任務／附件 `timing` 即使仍由舊後端回傳，也視為額外欄位忽略；既有加密工作紀錄仍可載入。伺服器不必先刪除欄位或下線供舊客戶端使用的路由。
- 一般／背景模式、實際進度、排隊順位、任務取消及斷線補查維持原有契約。狀態說明移到模式按鈕同一列。
- 設定面板改為 520 px（仍受視窗寬度限制），只保留垂直捲動；快捷鍵控制項可換行，單選選項採圓圈與文字並排。
- 無新增網站 API；繼續使用 0.8.1 的 EXE／NSIS 雙格式選版，0.8.3 的固定內網直連。驗證範圍見 [0.8.4 驗收](VALIDATION_0_8_4.md)。

0.4.1 僅更新桌面端的快捷鍵錄製與剪貼簿擷取，既有 API 契約不變，不需新增路由。
網站下載頁可提供新版 EXE，並依部署政策更新 latest_version／minimum_version；不要只因版本不同就強制更新。
桌面操作與公司實機驗收方式見 [快捷鍵錄製與 PDF 選字](HOTKEY_0_4_1.md)。

## 1. 本次必須修改的內容

本版使用固定主機 **`http://lp2-en-server`**，下列完整路徑已寫在 `src/config.rs`，一般使用者不能查看或更改連線設定。
這是產品操作限制，不是保密機制；原始碼與 EXE 本身仍可被檢視。變更主機或路由時，需重新建置並發佈桌面版。

| 方法 | 固定路徑 | 驗證／用途 |
| --- | --- | --- |
| POST | `/lm_server/api/desktop/oauth/device` | 匿名申請一次性登入碼 |
| POST | `/lm_server/api/desktop/oauth/token` | 以 device_code 輪詢與兌換個人 Token |
| POST | `/lm_server/v1/chat/completions` | Bearer Token；模型代號替換後轉送 |
| GET | `/lm_server/api/desktop/version` | 匿名查詢最低／最新版本 |
| GET | `/lm_server/api/desktop/models` | 可匿名或依 Bearer Token 回傳有權使用的模型選單 |
| GET | `/lm_server/desktop/download` | 更新清單 JSON；格式見 0.8 契約（可依 Accept 保留 HTML 頁） |

另外需提供瀏覽器授權頁，例如 `/lm_server/desktop/activate`（GET 顯示、POST 允許／拒絕）。
這個頁面的 URL 由 device 回應決定，不必固定上述例子，但入口必須與公司主機同來源。
它可沿用網頁登入／SSO，完成後讓使用者核對登入碼並明確允許。

**建議實作順序：固定路由 → models → version／download → 完整驗收。**
選取文字、快捷鍵與翻譯／摘要／潤飾在桌面端完成，不需要新增選字 API；按下按鈕後仍送相同 Chat Completions JSON。

所有桌面 API 請求會帶 `X-Client-Version: 0.4.1` 與 `Accept: application/json`。
此 Header 是相容性提示，不是驗證憑證；伺服器仍須自行檢查 Token、權限、模型白名單及配額。
`client_id` 固定為 `company-ai-desktop`，是公開客戶端識別，不配置 client_secret。
API 回應用 UTF-8 JSON，禁止 API redirect／HTML 登入頁；建議 `Cache-Control: no-store`。
瀏覽器授權頁及下載頁可以使用正常網頁流程。

## 2. 「一次性」與「30 天」是兩種不同的期限

- **登入碼**：建議有效 300 秒，使用者核准後只能成功兌換一次。
- **使用憑證（access_token / 應用程式專用 API Key）**：建議有效 2,592,000 秒，即 30 天。
- 此版不支援永久 Token 或 refresh_token。到期後重新開瀏覽器登入。
- EXE 接受伺服器回傳 1 秒至 30 天的期限；日後要超過 30 天，需一併修改桌面端上限。
- 網站必須每次檢查 Token 的到期與撤銷狀態，不能只依靠 EXE 的本機時間判斷。

不要把公司的共用模型服務 API Key 直接寫進 EXE。建議網站簽發個別使用者專用的 30 天 Key；
聊天閘道驗證該 Key 後，才在伺服器端套用真正的上游模型 API Key。

## 3. 申請登入碼

EXE 請求：

```http
POST /lm_server/api/desktop/oauth/device HTTP/1.1
Content-Type: application/x-www-form-urlencoded
Accept: application/json

client_id=company-ai-desktop&scope=chat%3Awrite
```

網站成功回應（HTTP 200）：

```json
{
  "device_code": "由伺服器產生的高熵隨機秘密字串",
  "user_code": "ABCD-EFGH",
  "verification_uri": "http://lp2-en-server/lm_server/desktop/activate",
  "verification_uri_complete": "http://lp2-en-server/lm_server/desktop/activate?user_code=ABCD-EFGH",
  "expires_in": 300,
  "interval": 5
}
```

請同時加上 `Cache-Control: no-store` 與 `Pragma: no-cache`。
`verification_uri_complete` 可省略，省略時 EXE 開啟 `verification_uri`，使用者手動輸入 EXE 顯示的 `user_code`。
登入網址必須與 EXE 設定的網站同來源（scheme、host、port 相同）；該網頁可以再導向既有 SSO。
本版接受 `expires_in` 1–900 秒、`interval` 1–60 秒；省略 interval 時為 5 秒。

`device_code` 不出現在瀏覽器 URL、介面或 log；`user_code` 才是讓人核對的短碼。
建議 device_code 至少有 256-bit 亂數，資料庫只保存其雜湊。短碼需限速且限制猜測次數。

## 4. 網頁上的授權確認

使用者從 EXE 開啟頁面後：

1. 若尚未登入，導向既有公司登入／SSO，完成後回到此頁。
2. 顯示使用者帳號、應用程式名稱 LM_AI、授權用途「傳送 AI 聊天請求」、期限 30 天。
3. **顯示 user_code，要求使用者確認與桌面程式相同**。帶 query string 不能視為已同意。
4. 提供「允許登入」與「拒絕」按鈕；使用 POST + 既有 CSRF 保護提交。
5. 伺服器驗證登入碼、期限、目前使用者權限後，原子地把 pending 更新為 approved 或 denied。
6. 網頁顯示「已完成操作，請回到 LM_AI」，不把 access_token 或 device_code 放進網址、HTML 或剪貼簿。

短碼已過期、已處理或不存在時，顯示「請回 EXE 重新登入」。
不要因為 GET 到確認頁就自動授權，以免瀏覽器預先載入或誤點造成登入。

## 5. 輪詢並兌換 Token

EXE 會在每次請求前等待 interval 秒：

```http
POST /lm_server/api/desktop/oauth/token HTTP/1.1
Content-Type: application/x-www-form-urlencoded
Accept: application/json

grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code&device_code=...&client_id=company-ai-desktop
```

回應情境：

| 情境 | HTTP | JSON 本文 |
| --- | --- | --- |
| 使用者尚未操作 | 400 | `{"error":"authorization_pending"}` |
| 輪詢過快 | 400（亦接受 429） | `{"error":"slow_down"}` |
| 使用者拒絕 | 400 | `{"error":"access_denied"}` |
| 過期／已兌換／碼不存在 | 400 | `{"error":"expired_token"}` |
| client_id / grant_type 不合法 | 400 | `{"error":"invalid_request"}` |
| 核准且尚未兌換 | 200 | 下方 Token JSON |

```json
{
  "access_token": "desktop_一段足夠長的隨機APIKey",
  "token_type": "Bearer",
  "expires_in": 2592000,
  "scope": "chat:write"
}
```

`access_token` 應為 URL-safe / ASCII 可見字元，不含空白、換行；例如 `desktop_` 加 base64url 亂數。
本版支援最多 8192 bytes；JWT 或隨機 opaque Token 都可。`token_type` 必須為 Bearer。

Token 兌換必須在資料庫交易中：

1. 鎖定／條件更新該 device_code，確認 client_id、期限、狀態為 approved。
2. 建立包含 user_id、scope、issued_at、expires_at、revoked_at 的個人使用憑證；隨機 Key 只存雜湊。
3. 將登入碼標記 consumed，提交交易，再回傳 Token。
4. 同一 device_code 的第二次兌換必須失敗，包括兩個並行請求。

EXE 遇到 slow_down 會將之後的輪詢間隔增加 5 秒。取消或逾期即停止；取消不等於撤銷伺服器上的 Token。
若兌換成功但回應因網路中斷遺失，使用者需重新登入；伺服器可清理沒有使用過的孤立憑證。

## 6. Chat Completions 請求與回覆

預設 Header：

```http
POST /lm_server/v1/chat/completions HTTP/1.1
Authorization: Bearer desktop_xxxxxxxxx
Content-Type: application/json; charset=utf-8
Accept: application/json
```

正式 EXE 固定使用 Bearer Header，沒有 Header 切換或 JSON 預覽欄位。憑證不放入 JSON 本文。

實際 JSON 格式：

```json
{
  "model": "fast",
  "messages": [
    { "role": "user", "content": "你好，請回覆連線測試成功。" }
  ],
  "stream": false
}
```

連續對話會依序帶入先前的 user / assistant 訊息。此版最多 20 輪；按「新對話」開始新對話。
使用者輸入最多 16,000 個 UTF-16 code units，每則訊息（含歷史回覆）上限 64 KB，HTTP 回應上限 1 MB。

網站驗證個人憑證後，可直接呼叫模型，也可代理至既有的 OpenAI 相容服務：

```text
桌面 EXE --個人 30 天 Key--> 公司網站聊天閘道
                              |
                              +--伺服器持有的上游 API Key--> 模型服務
```

不要把桌面傳入的 model 當成 URL；由網站檢查模型白名單與使用者權限。
若既有路由只認共用 API Key，需加一層閘道，讓網站簽發的個人 Key 能通過驗證。
本版要求登入 API、驗證頁與聊天入口同來源；上游服務可以由網站代理到其他主機。

成功回傳 HTTP 200、UTF-8 JSON：

```json
{
  "id": "chatcmpl-example",
  "object": "chat.completion",
  "created": 1789560000,
  "model": "fast",
  "choices": [
    {
      "index": 0,
      "message": { "role": "assistant", "content": "連線測試成功。" },
      "finish_reason": "stop"
    }
  ],
  "usage": { "prompt_tokens": 12, "completion_tokens": 8, "total_tokens": 20 }
}
```

EXE 讀取 `choices[0].message.content` 的字串；其他欄位可保留但不是本版必要欄位。
尚未處理 SSE、stream=true、tool_calls、圖片、檔案、多模態 content array。

錯誤範例（使用正確 HTTP 狀態，不回 200）：

```json
{
  "error": {
    "message": "登入已到期，請重新登入。",
    "type": "authentication_error",
    "code": "token_expired"
  }
}
```

- 401：缺少／無效／過期／已撤銷 Key；EXE 清除本機憑證，要求重新登入。
- 403：無權限；保留登入，顯示錯誤。
- 404：路由錯誤。
- 426：版本不再支援，顯示下載更新提示；版本端點需同時提供新的 minimum_version，桌面才能套用明確門檻。
- 429：限流；由使用者稍後重試。
- 5xx：服務問題。

登入與聊天 API 不可回傳 HTML 登入頁或 301/302；EXE 不會跟隨 API redirect。
EXE 不自動重送聊天請求，避免逾時後重複計費／執行。網站錯誤本文不得回顯 Token、Header 或上游密鑰。

## 7. 版本檢查與下載頁

```http
GET /lm_server/api/desktop/version HTTP/1.1
Accept: application/json
X-Client-Version: 0.4.1
```

HTTP 200：

```json
{
  "latest_version": "0.4.0",
  "minimum_version": "0.4.0",
  "message": ""
}
```

- 版本使用 `major.minor.patch` 三段非負整數，不使用 v 前綴、beta 或日期字串。每段最多 u32。
- `minimum_version` 不得大於 `latest_version`；message 可省略，最多 2,000 UTF-8 bytes。
- 目前桌面版本小於 minimum_version：顯示更新提示，停止新的登入及聊天，下載按鈕仍可用。
- 只小於 latest_version：顯示可更新，允許繼續使用。
- 連線失敗、非 200、JSON 不合法：依使用者指定暫時允許使用；同次執行內已知的強制更新不因後續失敗而解除。
- 啟動、每 5 分鐘與手動重新整理時查詢。首次查詢尚未完成時沒有已知門檻；當次已送出的聊天不會自動撤回。
- 版本門檻沒有持久化至磁碟。後端若需強制限制舊客戶端，應在登入與聊天端點同步檢查版本，不能只依賴客戶端介面。
- 登入／聊天拒絕舊版本時可回 HTTP 426 的標準 error JSON，並保持 version 路由可匿名取得。桌面不會自動重試聊天。

固定下載頁：`http://lp2-en-server/lm_server/desktop/download`。
頁面需顯示最新版本、更新說明、Win11 x64 EXE 下載連結（可另設二進位檔路由）及 SHA256。
版本 JSON 不接受外部 download_url；桌面只會開啟固定下載頁，不執行安裝器或背景覆寫。
使用者下載新版後關閉舊版、替換 EXE，個人設定與 DPAPI 憑證不在 EXE 旁，因此可保留。
請先讓新版檔案可下載，再提高 minimum_version，避免使用者被阻擋卻拿不到新版。

## 8. 動態模型選單

```http
GET /lm_server/api/desktop/models HTTP/1.1
Accept: application/json
X-Client-Version: 0.4.1
Authorization: Bearer desktop_xxxxxxxxx
```

未登入時沒有 Authorization。若清單需權限，回 401 即可；桌面仍允許登入，取得 Token 後會再查一次。
HTTP 200：

```json
{
  "models": [
    {"id": "fast", "label": "快速", "description": "日常翻譯與短文整理"},
    {"id": "quality", "label": "品質", "description": "較仔細的分析"},
    {"id": "ultra", "label": "Ultra"}
  ],
  "default_model": "fast"
}
```

| 欄位 | 契約 |
| --- | --- |
| models | 1–40 個選項；空清單代表目前無可用項目，桌面會停用送出並提示 |
| id | 穩定、唯一、不透明的代號；1–64 bytes，只能英數、點、底線、連字號 |
| label | 使用者看到的名稱；1–40 個字元，不能全空白或含控制字元 |
| description | 可省略，最多 1,000 UTF-8 bytes；此版保留解析但不顯示 |
| default_model | 可省略；如提供，必須在 models 清單內 |

桌面顯示 label，送出 JSON 的 `model` 是 id。優先選上次偏好，再選 default_model，最後才是第一項。
禁止只回傳 `["快速","品質"]` 或 OpenAI 原始 `/v1/models` 格式；本版使用上述桌面選單契約。
後端負責把 `fast`／`quality` 替換成實際模型名稱，轉送後可將回應 `model` 改回 alias，避免暴露內部名稱。
新增 Ultra 只需修改清單和後端映射，不必更新 EXE；不要把 alias 當 URL 或任意上游參數使用。

建議清單只列出該帳號有權限的選項，每次聊天仍再次驗證 alias、權限與配額。
未知 alias 回 400、有此選項但無權限回 403；錯誤提示可請使用者重新整理選單。
此版無模型快取離線兜底；查詢失敗會提示，需登入或重新整理取得清單後才可聊天。

## 9. 選取文字的後端行為

一般送出：messages 末尾是使用者草稿。翻譯／摘要／潤飾：末尾 user.content 為桌面動作指示、空行、使用者文字。
例如摘要：

```json
{
  "model": "fast",
  "messages": [{"role": "user", "content": "請以繁體中文摘要以下文字，列出主要重點，不補充未提供的事實。\n\n這裡是使用者選取並確認送出的文字。"}],
  "stream": false
}
```

這些動作只需要現有聊天路由，不另傳剪貼簿來源、視窗標題、程式名稱或郵件識別碼。
按快捷鍵只帶入草稿，不產生聊天 API 請求；使用者明確按處理／送出後才會傳送。
後端將文字視為一般使用者輸入，不能因選字含有指令就執行網站管理、任意工具或未授權工作。

## 10. 儲存與連線邊界

- 偏好檔只保存 model、hotkey、font_size、sidebar_collapsed、notification_popups 與 dark_mode；固定連線資訊來自程式，忽略舊 settings.json 裡的 server_url、路由與 Header。
- Token 由 Windows DPAPI 加密，綁定目前 Windows 使用者及實際端點。換端點需重新登入；只換模型不必。
- 對話成功回覆後以 Windows 使用者 DPAPI 加密保存；未送出草稿只在記憶體。登出清除本機 Token，不代表伺服器端撤銷。
- 目前明確採指定內網 HTTP；HTTP 會明文傳輸 Token 和內容。若日後啟用 HTTPS，使用 Windows 信任庫，不略過憑證錯誤。
- 0.8.3 起固定公司 origin `http://lp2-en-server:80` 與 loopback 使用 WinHTTP 直接連線；其他 origin 使用系統自動代理。登入／metadata 讀取逾時 15 秒，聊天讀取逾時 120 秒，屬各階段逾時而非整次硬性總時限。
- 無自訂代理帳密／用戶端憑證選取；不需要 CORS。SSO、Cookie 和 CSRF 由瀏覽器授權頁處理。
- 正式模式不開本機 HTTP port，只有 --demo 會在 127.0.0.1 隨機埠啟動模擬服務。

### 0.8.2 登入連線診斷

登入依序為：POST device 取得登入碼 → ShellExecuteW 開啟預設瀏覽器 → POST token 輪詢授權。若 device 連線失敗，瀏覽器尚未開啟。Edge 能讀取 version JSON，只代表該瀏覽器當時能連到 version GET，不代表桌面 WinHTTP 或兩個登入 POST 已通過。

網路錯誤現在包含用途與 API 階段，例如「申請登入碼失敗（尚未開啟瀏覽器）：Windows 網路錯誤 10022（WinHttpSendRequest）」。這只是訊息格式範例，尚未在公司故障電腦確認實際失敗階段。新增診斷不包含網址、查詢參數、Header、本文、登入碼或 Token；請回報完整錯誤文字即可，不需提供憑證。

10022 對應 WSAEINVAL，表示參數或連線狀態無效，不能只憑代碼判定為防毒、代理或 WebView2 缺漏。0.8.2 診斷版仍使用自動代理。回傳成功但寫入 0 bytes 時改報傳送無進度，不讀取可能殘留的 GetLastError；這是錯誤呈現修正，不代表已證實為本次 10022 的原因。

參考：[Microsoft WinHTTP](https://learn.microsoft.com/en-us/windows/win32/api/winhttp/nf-winhttp-winhttpopen)、[Windows Socket 錯誤碼](https://learn.microsoft.com/en-us/windows/win32/winsock/windows-sockets-error-codes-2)。

### 0.8.3 隔離內網直連

公司實測回報模型與 device 都在 WinHttpOpen 回傳 10022，尚未呼叫 WinHttpConnect 或送出 HTTP 請求。使用者確認公司網路與外網完全隔離，Windows 代理為「自動偵測」。因此固定公司 origin 改用 `WINHTTP_ACCESS_TYPE_NO_PROXY`，不再為這個目的地初始化 `WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY`。這是針對已確認的失敗階段與部署環境調整，仍不能斷言 Trend 或某個 Windows 服務就是根因。

規則以 `src/config.rs` 的 SERVER_URL 完整 origin（scheme、host、有效 port）比對；不將任意單段主機名、私有 IP、相似網域或其他埠號一律直連。HTTP API、更新下載及 WebSocket 使用同一個 session 建立函式。若日後公司要求此 origin 經代理，需同步調整桌面連線契約，不能只改 Windows 代理設定而預期此版自動跟隨。

不修改系統代理、不增加外網探測、不呼叫 PowerShell，也不重送 POST。HTTPS 仍使用系統信任憑證。新的初始化錯誤會區分 `WinHttpOpen/direct` 與 `WinHttpOpen/automatic-proxy`；若直連仍在 WinHttpOpen 出現 10022，應繼續排查該電腦的 WinHTTP／網路元件，不能視為已修復。

## 11. 網站驗收順序

1. version 回 0.4.0／0.4.0，models 回 fast／quality；桌面顯示對應名稱。
2. 完成瀏覽器授權，核對短碼，取得 Bearer 個人 Token。
3. 送出繁體中文，伺服器收到 alias、完整 messages、stream=false 與 X-Client-Version。
4. 伺服器映射到真正模型並回覆，桌面顯示內容、可追問和複製。
5. models 新增 ultra，按重新整理後出現 Ultra，不更新 EXE。
6. version 回 latest=0.5.0、minimum=0.4.0，允許使用；minimum=0.5.0，提示更新、禁止新登入與聊天。
7. 全新啟動時只有 version 回 503，models／登入／聊天正常：可使用；已知強制更新後 version 再斷線：同次執行仍阻擋。
8. models 先回 401，完成登入後可取到清單；空清單、重複 id、錯誤 default 不可默默送出未知模型。
9. 選取文字按快捷鍵，確認伺服器沒有收到聊天；再按摘要才收到一次請求。
10. 驗證到期、拒絕、一次性碼重複及並行兌換、401 撤銷、429 限流、HTML／redirect 等錯誤。

公司 SSO、DNS、實際模型服務與跨應用選字仍需實機驗收；本機模擬測試不能取代這些檢查。
本版新增通知與 Classic Outlook 契約見 [NOTIFICATIONS_AND_OUTLOOK.md](NOTIFICATIONS_AND_OUTLOOK.md)。

桌面圖示已統一採用使用者提供的貓咪圖案，隨 EXE 內嵌，涵蓋視窗、工作列及系統托盤；不需網站提供圖檔，也不變更 API 契約。圖示維護方式見 [assets/README.md](../assets/README.md)。

## 格式參考

介面與串流修正見 [UI_AND_STREAMING.md](UI_AND_STREAMING.md)：支援網站的 `start` → `tool_status` → `delta.text` → `done`，保留 OpenAI 相容格式；斷線保存部分回答並提示不完整，REST 完整結果可原位取代。工具名稱及狀態以原文顯示，不顯示 arguments／result。此修正不新增網站路由。

Outlook 日期查詢預設為「Outlook 目前資料夾及子資料夾」，另可選「預設收件匣及子資料夾」；介面已移除所有已載入信箱與本機資料檔選項，詳見 [Outlook 查詢說明](OUTLOOK_SEARCH.md)。這是桌面介面調整，不新增網站 API；批次基本資訊保留 `folder` 顯示來源資料夾，仍不提供 EntryID／StoreID 或磁碟路徑。本次僅保存變更，未進版或打包。

Outlook 第一輪維持以 `# Outlook 郵件初篩 outlook-triage` 開頭，供網站分流至不額外附帶提示詞／技能的初篩路由。第二輪保留資料與摘要，固定送 `model: "quality"`，列出每份上傳 MSG 對應的完整 `.md` 名稱並允許網站文件工具；不重送初篩 Skill，由網站使用正常提示詞與技能。詳見 [0.6 分流與附件名稱契約](DESKTOP_0_6_CONTRACT.md#初篩與附件分析的分流)。

- [OAuth Device Authorization Grant — RFC 8628](https://www.rfc-editor.org/rfc/rfc8628.html)
- [Windows WinHTTP](https://learn.microsoft.com/en-us/windows/win32/api/winhttp/nf-winhttp-winhttpopen)
- [Windows DPAPI](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)

## 0.7 桌面契約

新版用途旗標、一般／背景模式、SSE、Outlook 容錯與自動更新欄位見 [DESKTOP_0_7_CONTRACT.md](DESKTOP_0_7_CONTRACT.md)；驗證範圍見 [VALIDATION_0_7.md](VALIDATION_0_7.md)。聊天路徑保留 /lm_server/v1/chat/completions。
