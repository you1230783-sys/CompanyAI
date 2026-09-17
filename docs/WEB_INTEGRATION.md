# LM_AI 0.4.0：網站／API 串接契約

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
| GET | `/lm_server/desktop/download` | 瀏覽器下載頁；提供新版 EXE 與版本資訊 |

另外需提供瀏覽器授權頁，例如 `/lm_server/desktop/activate`（GET 顯示、POST 允許／拒絕）。
這個頁面的 URL 由 device 回應決定，不必固定上述例子，但入口必須與公司主機同來源。
它可沿用網頁登入／SSO，完成後讓使用者核對登入碼並明確允許。

**建議實作順序：固定路由 → models → version／download → 完整驗收。**
選取文字、快捷鍵與翻譯／摘要／潤飾在桌面端完成，不需要新增選字 API；按下按鈕後仍送相同 Chat Completions JSON。

所有桌面 API 請求會帶 `X-Client-Version: 0.4.0` 與 `Accept: application/json`。
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
X-Client-Version: 0.4.0
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
X-Client-Version: 0.4.0
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

- 偏好檔只保存 model、hotkey、font_size、sidebar_collapsed 與 notification_popups；固定連線資訊來自程式，忽略舊 settings.json 裡的 server_url、路由與 Header。
- Token 由 Windows DPAPI 加密，綁定目前 Windows 使用者及實際端點。換端點需重新登入；只換模型不必。
- 對話成功回覆後以 Windows 使用者 DPAPI 加密保存；未送出草稿只在記憶體。登出清除本機 Token，不代表伺服器端撤銷。
- 目前明確採指定內網 HTTP；HTTP 會明文傳輸 Token 和內容。若日後啟用 HTTPS，使用 Windows 信任庫，不略過憑證錯誤。
- 使用 WinHTTP 自動代理；登入／metadata 讀取逾時 15 秒，聊天讀取逾時 120 秒，屬各階段逾時而非整次硬性總時限。
- 無自訂代理帳密／用戶端憑證選取；不需要 CORS。SSO、Cookie 和 CSRF 由瀏覽器授權頁處理。
- 正式模式不開本機 HTTP port，只有 --demo 會在 127.0.0.1 隨機埠啟動模擬服務。

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

## 格式參考

- [OAuth Device Authorization Grant — RFC 8628](https://www.rfc-editor.org/rfc/rfc8628.html)
- [Windows WinHTTP](https://learn.microsoft.com/en-us/windows/win32/api/winhttp/nf-winhttp-winhttpopen)
- [Windows DPAPI](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
