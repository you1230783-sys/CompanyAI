# LM_AI 0.8.1：EXE／NSIS 雙格式更新

本文件取代 0.8 的「只能發布 NSIS」限制。登入、聊天、最低版本政策與內建公鑰不變。
0.8.1 先交付 `dist/LM_AI.exe` 與 `dist/update-manifest-exe.json`；不製作新 Setup 或離線 ZIP。
`CompanyAI.exe` 相容檔名已移除。首次從 0.8.0 遷移至 0.8.1，請手動下載、更換主程式；0.8.0 的更新器不接受 EXE 清單。

## 網站需要提供的檔案

版本 API 仍為 `GET /lm_server/api/desktop/version`，維持 `latest_version`、`minimum_version`、`message`。
本次 `latest_version` 設為 `0.8.1`；一般更新不要只因發布新版而提高 `minimum_version`。

桌面會查詢下列三個同來源位置，收集通過驗證的清單：

| 路徑 | 用途 |
| --- | --- |
| `/lm_server/desktop/download` | 相容原本路由；可回一份清單或 JSON 陣列 |
| `/lm_server/desktop/releases/update-manifest-exe.json` | 獨立 EXE 的簽署清單 |
| `/lm_server/desktop/releases/update-manifest.json` | 原本 NSIS 的簽署清單 |

網站直接提供產生的 JSON，不必合併兩檔。未發布的來源可回 404；HTML、錯誤狀態、格式錯誤或簽章錯誤的單一來源不會遮蔽其他合法清單。回傳陣列時上限 16 份，每一份獨立驗證；每個回應上限 65,536 UTF-8 bytes。
所有清單使用 `application/json; charset=utf-8` 與 `Cache-Control: no-store`。程式只查詢清單，取得使用者同意後才下載二進位檔。

本次 EXE 放在 `/lm_server/desktop/releases/LM_AI.exe`，與 `dist/update-manifest-exe.json` 成對發布。
未來 NSIS 繼續使用 `/lm_server/desktop/releases/LM_AI_Setup.exe` 與 `dist/update-manifest.json`。
下載檔回 `200 application/octet-stream`；不要求登入、不傳 Token/Cookie、不重新導向、不變更檔案內容。

## 版本與格式選擇

1. 驗證同來源、三段數字版本、Windows x64、大小範圍、SHA256 格式與固定公鑰簽章。
2. 以數字版本比較，例如 `0.10.0 > 0.9.9`；版本最大者優先，不受清單讀取順序影響。
3. 同版本優先 `exe`，再選 `nsis`。較新 NSIS 會勝過較舊 EXE，保留未來切回安裝包的能力。
4. 版本檢查會將清單中的較高合法版本反映至「可更新至」提示；最低版本依原 API 政策處理，清單不降低門檻。
5. 使用者同意下載時重新查清單；候選必須高於目前程式，且不得低於已顯示的最新版本。若 API 宣告較高版本但尚無合法檔案，提示錯誤，不能偷偷下載較舊版本。
6. 下載期间有更高已簽署版本可接受；若另一次版本檢查已提高門檻至下載檔之上，要求重新下載。

既有 NSIS 0.8.0 JSON 可以暫時與 EXE 0.8.1 JSON 並存，0.8.1 客戶端會選 EXE。
保留舊 Setup 的版本與簽章，不能手動把旧包版本改成 0.8.1。

## JSON 與簽章

清單仍為 `schema_version: 1`，欄位維持 `version`、`platform`、`kind`、`url`、`size`、`sha256`、`signature`。
`platform` 固定 `windows-x86_64`，`kind` 接受 `exe` 或 `nsis`。不要加入教學網址或額外外層物件。
簽章訊息仍為以下 UTF-8 內容（LF 分行，最後有 LF），RSA-3072 / PKCS#1 v1.5 / SHA256：

```text
LM_AI_UPDATE_V1
<version>
windows-x86_64
<exe 或 nsis>
<檔案 byte 長度>
<SHA256 小寫 hex>
```

`kind` 也在簽章內，不能只改舊 JSON 的格式、大小或 hash 而保留舊簽章。只有 `url` 可調整為實際同來源路徑。
下載後再次驗證整檔大小與 SHA256，失敗移除本次檔案。下載目錄及本機檔名由程式決定，不接受伺服器指定任意本機路徑。

## 使用者流程

- EXE：取得下載同意後存入 `%LOCALAPPDATA%\CompanyAI\updates\<本次識別碼>\LM_AI.exe`。完成後按鈕為「開啟下載資料夾」，以 Windows Shell API 選取檔案並顯示手動更換說明。使用者先從托盤離開，再複製新版替換原位置的 EXE、重新開啟；資料仍保存在 CompanyAI。程式不自行執行新版、覆蓋正在使用的檔案或退出。
- NSIS：下載及驗證後按鈕為「安裝並重新啟動」，第二次同意後保留原生 PID 交接、安裝及重啟流程。
- 強制更新：EXE 下載完成或開啟資料夾都不解除功能鎖定，只有啟動滿足最低版本的程式才恢復。
- 本次未加入教學網頁；未新增使用者端 PowerShell、CMD、BAT 或其他腳本。

## 發行方式

一般執行 `scripts/Build.ps1 -EmptyCargoCache`，使用 v142 x64、空 Cargo 快取及 vendor 完成 `--frozen` 建置、測試、WebView2 自我檢查，產生單一主程式與 EXE 簽署清單。
驗證記錄為 `offline/exe-verification.json`。此檔的 PASS 不代表公司防毒或公司端跨版本測試已通過。

未來要發布 NSIS，執行 `scripts/Build.ps1 -IncludeInstaller`；需要完整離線 ZIP 才執行 `scripts/Prepare-Delivery.ps1`。
僅重新簽署時使用 `scripts/Update-Signing.ps1 -Kind exe` 或 `-Kind nsis`；腳本核對實際檔案版本並以主程式驗證簽章，不能把舊 Setup 簽成新版本。
