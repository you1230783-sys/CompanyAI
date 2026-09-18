# LM_AI 0.8：NSIS 安裝與更新契約

> 0.8.1 起以 [雙格式更新契約](UPDATE_0_8_1_CONTRACT.md) 覆蓋本文只能使用 NSIS 的限制；本文保留給 0.8.0 舊客戶端及 NSIS 安裝細節參考。

本文件取代 0.7 的安裝／更新規格；聊天、SSE、Outlook 與登入路由維持原契約。

## 給網站端的實作要求

更新檔必須放 **完整的 `dist/LM_AI_Setup.exe`**，不要放主程式 `LM_AI.exe`。同一個 Setup 用於首次安裝與後續更新，含主程式、原生解除安裝程式及 WebView2 x64 離線安裝檔。電腦已有 WebView2 時略過 Runtime 安裝。

### 1. 版本查詢，路由不變

`GET http://lp2-en-server/lm_server/api/desktop/version`

請回 `200 application/json; charset=utf-8`，例如下一次發布 0.8.1：

```json
{
  "latest_version": "0.8.1",
  "minimum_version": "0.8.0",
  "message": "新版已發布"
}
```

版本是三段數字；`minimum_version <= latest_version`。不要因為有新版就提高最低版本。只有確定不再支援舊版時才提高 minimum_version。此路由不需要回傳下載資訊；0.8 改為獨立查詢下載路由。

### 2. 原本的下載頁改為回傳 JSON

`GET http://lp2-en-server/lm_server/desktop/download`

桌面送出 `Accept: application/json`、`X-Client-Version: 0.8.0`，不帶 Token、Cookie。可依 Accept 區分 HTML 下載頁與桌面 JSON；若不需要保留網頁，直接回 JSON 即可。

請回 `200 application/json; charset=utf-8`、`Cache-Control: no-store`。**使用建置產生的 `dist/update-manifest.json`；以下只示意格式，省略號不能用於正式回應。**

```json
{
  "schema_version": 1,
  "version": "0.8.1",
  "platform": "windows-x86_64",
  "kind": "nsis",
  "url": "/lm_server/desktop/releases/0.8.1/LM_AI_Setup.exe",
  "size": 213000000,
  "sha256": "64 個小寫十六進位字元",
  "signature": "768 個十六進位字元"
}
```

| 欄位 | 規則 |
| --- | --- |
| schema_version | 固定數字 1 |
| version | 必須等於版本服務 latest_version，且高於目前程式版本 |
| platform / kind | 固定 `windows-x86_64` / `nsis` |
| url | 同來源的最終下載位置，可為根相對路徑或完整網址；主機、協定與埠須與公司主機相同 |
| size | 完整 Setup 的位元組長度，100,000 至 1,073,741,824；不是 MB 數 |
| sha256 | 完整 Setup 的 SHA256，64 個小寫 hex 字元 |
| signature | 發行工具產生的 RSA-3072 / SHA256 簽章；網站不自行生成、修改或替換公鑰 |

除 url 可改成實際同來源下載位置外，其餘欄位請原封不動使用發行檔。不要加額外 JSON 外層、不要回 HTML 或 302、不要要求瀏覽器登入。欄位格式錯誤會停止更新並顯示原因。版本查詢與下載清單須一起切換；下載期間若版本改變，桌面會要求重新下載。

### 3. 實際安裝包的下載回應

桌面 GET 上面的 url，不帶 Token／Cookie，不跟隨任何重新導向。

```http
HTTP/1.1 200 OK
Content-Type: application/octet-stream
Content-Length: <Setup 的實際位元組長度>
Cache-Control: no-transform
```

本文直接是該次發行的 Setup 二進位內容。支援分塊傳輸，但最終實際長度仍須與 size 相符；請勿套壓縮、HTML 包裝或中途置換檔案。建議每個版本使用獨立網址，先放完整檔與清單，最後才提高版本服務的版本。404、HTML、轉址、長度／SHA256／簽章不符均不執行；使用者可重試。

## 桌面流程

- 支援中的版本：只檢查版本，不自動下載。使用者按「下載更新」，原生對話框取得第一次同意，才查清單及下載 Setup。
- 下載完成：保持主程式開啟，顯示「安裝並重新啟動」。使用者按下並於原生對話框再次確認後，保存草稿、停止本機操作、啟動 NSIS、退出主程式。
- NSIS 等指定 PID 真正退出，取得與主程式相同的單一實例鎖，才替換；完成後自動開啟新版。
- 取消任一確認不會偷偷下載／安裝。關閉視窗仍縮到托盤；真正退出也不會觸發安裝。待安裝檔不會跨重新啟動自動執行。
- 低於最低版本：以不可按 Escape 關閉的更新畫面禁用功能，原生命令入口、選字及本機自動流程也停止。只能更新、重新檢查或退出；拒絕／下載失敗／安裝取消都不能繼續使用。仍保留確認視窗，不在使用者未操作時突然退出。
- 已確認的最低版本保存到 `%LOCALAPPDATA%\CompanyAI\version-policy.json`，斷線、重開舊版或後續降低 minimum_version 都不能解除；安裝達到門檻的新版後才恢復。從未取得有效版本限制且檢查失敗時，延續既有暫用政策。
- 伺服器已接受的背景任務不會因退出而取消；新版可恢復查詢。本機 Outlook 操作停止後需要重新確認授權。

## 安裝／解除安裝

安裝位置固定 `%LOCALAPPDATA%\Programs\LM_AI`，建立目前使用者桌面與開始功能表捷徑，以及 Windows 解除安裝項目。資料繼續保存在 `%LOCALAPPDATA%\CompanyAI`。

可攜版亦可發起 NSIS 更新，但會安裝到上述固定位置；原可攜版檔案不會被任意刪除。之後請使用新捷徑，避免再啟動原先下載資料夾中的舊 EXE。既有 0.7 的首次遷移請手動退出並執行本版 Setup，不能靠舊的腳本式更新器升級。

安裝／更新／解除安裝不執行 PowerShell、CMD、BAT、VBS 或外部腳本。只使用 NSIS 內建指令、Windows 原生 API，及必要時的 Microsoft WebView2 EXE。開發機的 Build／測試／簽署腳本不會放入使用者安裝目錄。

替換前完整解壓；檔案被占用或替換失敗時保留／還原旧主程式並回報。安裝中斷、強制斷電不屬於資料庫式整包交易；必要時重新執行完整 Setup 修復。安裝成功後若 Windows 拒絕啟動新版，會提示從捷徑開啟。

解除安裝需先退出主程式，只移除已知程式、捷徑與登錄；一律保留聊天／設定／登入資料，以及安裝目錄內的未知檔案，不遞迴刪除使用者資料、不移除共用 WebView2。

## 發行者操作與簽章

`scripts/Build.ps1` 使用固定 NSIS 3.12 封裝。工具 ZIP 隨原始碼及離線包提供，SHA256 固定為 `56581f90db321581c5381193d796fffcf2d24b2f8fed2160a6c6a3baa67f2c4f`，保留 ZIP 內授權。正式 Rust 及測試原生 payload 仍使用 MSVC v142 x64。

更新信任公鑰在 `assets/update-public-key.blob`。私鑰在開發機 `.private/update-key.dpapi`，由 Windows DPAPI 加密綁定目前帳號／電腦，**不提交 Git、不放離線 ZIP、不交網站端**。未取得私鑰的電腦可離線重建及測試，但不能生成能被已部署程式接受的新更新簽章。請妥善管理原發行環境；單獨複製 DPAPI 檔不能在另一台電腦解密。若私鑰遺失，必須重新部署信任新金鑰的第一版。

`Build-Installer.ps1` 在原發行環境自動產生清單，並由本次編譯的 Rust 程式驗證其簽章和整份 Setup 雜湊。若後續另加 Authenticode，須在簽完 Setup 後重新執行開發專用 `scripts/Update-Signing.ps1`，再發布新清單。不可使用舊 hash／signature 配新檔。

簽章輸入為下列 UTF-8 字串（LF 分行，最後亦有 LF），以 SHA256 雜湊後使用 RSA PKCS#1 v1.5 簽署，輸出 hex：

```text
LM_AI_UPDATE_V1
<version>
windows-x86_64
nsis
<size 的十進位整數>
<sha256 小寫>
```

這是更新來源驗證，與 Windows Authenticode／SmartScreen 信譽不同；不宣稱可保證所有公司防毒放行。實作參考 [Microsoft BCryptVerifySignature](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptverifysignature) 與 [NSIS 原生操作規範](https://nsis.sourceforge.io/Docs/Chapter4.html)。
