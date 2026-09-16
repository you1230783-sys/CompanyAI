# Company AI 測試版：網頁／API 串接規格 v1

## 1. 明天需要實作的最小範圍

桌面測試版已完成原生 Windows 介面。網站端需要提供下列路由：

| 方法 | 路徑 | 用途 |
| --- | --- | --- |
| POST | `/api/desktop/oauth/device` | 申請短效的一次性登入碼 |
| GET | `/desktop/activate?user_code=...` | 使用既有網頁登入／SSO，讓使用者確認授權 |
| POST | `/desktop/activate` | 接受或拒絕授權；這是網站內部表單，可用既有路由 |
| POST | `/api/desktop/oauth/token` | EXE 輪詢，取得 30 天使用憑證 |
| POST | `/v1/chat/completions` | 驗證憑證後，處理或轉送 OpenAI 相容聊天請求 |

前兩個 `/api/desktop/oauth/...` API 路徑在本版固定。聊天路徑、網站網址、模型名稱與 Header 類型由 EXE 設定。
例如網站填 `https://ai.company.example`，路徑填 `/v1/chat/completions`；**不要把 `/v1` 重複填進網站欄位**。
如果實際路由是 `/api/v1/chat/completions`，只要修改 EXE 的 API 路徑即可。

登入流程採 OAuth 2.0 Device Authorization Grant（RFC 8628）。EXE 不開啟本機回呼 port，使用者以瀏覽器登入，EXE 向網站輪詢結果。
`client_id` 固定為 `company-ai-desktop`，屬於公開客戶端，**不是密碼，不配置 client_secret**。

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
POST /api/desktop/oauth/device HTTP/1.1
Content-Type: application/x-www-form-urlencoded
Accept: application/json

client_id=company-ai-desktop&scope=chat%3Awrite
```

網站成功回應（HTTP 200）：

```json
{
  "device_code": "由伺服器產生的高熵隨機秘密字串",
  "user_code": "ABCD-EFGH",
  "verification_uri": "https://ai.company.example/desktop/activate",
  "verification_uri_complete": "https://ai.company.example/desktop/activate?user_code=ABCD-EFGH",
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
2. 顯示使用者帳號、應用程式名稱 Company AI、授權用途「傳送 AI 聊天請求」、期限 30 天。
3. **顯示 user_code，要求使用者確認與桌面程式相同**。帶 query string 不能視為已同意。
4. 提供「允許登入」與「拒絕」按鈕；使用 POST + 既有 CSRF 保護提交。
5. 伺服器驗證登入碼、期限、目前使用者權限後，原子地把 pending 更新為 approved 或 denied。
6. 網頁顯示「已完成操作，請回到 Company AI」，不把 access_token 或 device_code 放進網址、HTML 或剪貼簿。

短碼已過期、已處理或不存在時，顯示「請回 EXE 重新登入」。
不要因為 GET 到確認頁就自動授權，以免瀏覽器預先載入或誤點造成登入。

## 5. 輪詢並兌換 Token

EXE 會在每次請求前等待 interval 秒：

```http
POST /api/desktop/oauth/token HTTP/1.1
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
本版支援最多 8192 bytes；JWT 或隨機 opaque Token 都可。`token_type` 必須為 Bearer，這也適用 EXE 選擇 X-API-Key Header 的情形。

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
POST /v1/chat/completions HTTP/1.1
Authorization: Bearer desktop_xxxxxxxxx
Content-Type: application/json; charset=utf-8
Accept: application/json
```

EXE 也可選擇 `X-API-Key: desktop_xxxxxxxxx`，此時不加 Bearer 前綴。
憑證只放在選定的 Header，**JSON 本文與預覽區不會出現 Key**。

實際 JSON 格式：

```json
{
  "model": "公司實際提供的模型名称",
  "messages": [
    { "role": "user", "content": "你好，請回覆連線測試成功。" }
  ],
  "stream": false
}
```

連續對話會依序帶入先前的 user / assistant 訊息。此版最多 20 輪；按「清除對話」開始新對話。
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
  "model": "公司實際提供的模型名称",
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
- 429：限流；由使用者稍後重試。
- 5xx：服務問題。

登入與聊天 API 不可回傳 HTML 登入頁或 301/302；EXE 不會跟隨 API redirect。
EXE 不自動重送聊天請求，避免逾時後重複計費／執行。網站錯誤本文不得回顯 Token、Header 或上游密鑰。

## 7. 保存、連線與第一版限制

- EXE 設定：`%LOCALAPPDATA%\CompanyAI\settings.json`。
- EXE 憑證：`%LOCALAPPDATA%\CompanyAI\session.dpapi`，使用 Windows DPAPI 的目前使用者範圍加密。
- 修改網站、聊天路徑或 Header 類型會清除舊登入，防止 Key 被送到新目的地；只改模型不必重新登入。
- 對話僅在視窗記憶體，關閉即清除；不自動讀取剪貼簿、檔案或其他視窗。
- 「清除本機登入」刪除本機憑證，不代表伺服器撤銷。網站應提供個人裝置／憑證撤銷能力，並在聊天 API 即時檢查。
- 使用 WinHTTP／Windows 信任庫與自動代理設定；沒有實作自訂代理帳密或用戶端憑證選取。
- 預設 HTTPS，憑證錯誤不略過。公司私有 CA 應由 IT 安裝至 Windows 信任庫。
- HTTP 只允許本機 loopback，或使用者勾選「允許內網 HTTP」。HTTP 會明文傳送 Key 與訊息，僅限環境確實需要的測試。
- 登入 HTTP 回應讀取逾時 15 秒，聊天讀取逾時 120 秒（為 WinHTTP 各階段／讀取逾時，非整次請求硬性總時限）。
- EXE 直接發 HTTP，不需要 CORS；瀏覽器授權頁仍使用既有網站的 Cookie、CSRF 與登入規則。
- 主程式不開啟本機 HTTP port；僅 `--demo` 模式會在 127.0.0.1 的隨機埠啟動模擬服務。

## 8. 網頁端驗收清單

1. EXE 儲存正確網址、路由、模型；點登入能開啟網站。
2. 網站登入後顯示相同短碼；允許後 EXE 顯示已登入。
3. EXE 送出繁體中文訊息，網站收到預期 Header 與 JSON，EXE 顯示真正模型回覆。
4. 關閉並重新打開 EXE，可在 30 天內沿用登入。
5. 拒絕、短碼過期、重複兌換與同時兌換均正確拒絕。
6. 撤銷／讓 Token 過期後，聊天路由回 401，EXE 要求重新登入。
7. 限流回 429，伺服器錯誤回 5xx，EXE 顯示可理解的訊息。
8. 無效憑證、跨站 verification_uri、API redirect 均停止，不洩漏 Key。

本機 `--demo` 已可模擬以上核心成功流程，但不代表公司 SSO、網路、憑證與實際模型路由已完成驗證。

## 9. 官方格式參考

- [OAuth Device Authorization Grant — RFC 8628](https://www.rfc-editor.org/rfc/rfc8628.html)
- [OpenAI Chat Completions 格式](https://platform.openai.com/docs/api-reference/chat/create)
- [Windows WinHTTP](https://learn.microsoft.com/en-us/windows/win32/api/winhttp/nf-winhttp-winhttpopen)
- [Windows DPAPI CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
