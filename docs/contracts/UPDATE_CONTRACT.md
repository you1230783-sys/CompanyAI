# EXE／NSIS 更新契約

本文件彙整現行更新規則，取代舊版分散的發行說明。實際清單由`scripts/Update-Signing.ps1`產生；目前完整交付使用NSIS，兩個清單端點均指向同一個Setup。

## 路由與格式

版本API為`GET /lm_server/api/desktop/version`，回傳`latest_version`、`minimum_version`與`message`；一般發行不應任意提高minimum_version。

桌面收集同來源`/lm_server/desktop/download`、`/lm_server/desktop/releases/update-manifest-exe.json`及`/lm_server/desktop/releases/update-manifest.json`。使用`application/json; charset=utf-8`、`Cache-Control: no-store`；未發布端點可回404，單一來源失敗不能遮蔽其他合法清單。

清單欄位為`schema_version:1`、`version`、`platform`、`kind`、`url`、`size`、`sha256`、`signature`。平台為`windows-x86_64`，kind接受`exe`或`nsis`。採數字版本排序，同版優先NSIS；較新版本優先於格式。使用者確認時重新核對版本，不能回退到較舊成品。

## 簽章與下載

簽章採RSA-3072／PKCS#1 v1.5／SHA256。以下UTF-8內容以LF分行，末尾亦有LF：

```text
LM_AI_UPDATE_V1
<version>
windows-x86_64
<exe 或 nsis>
<檔案 byte 長度>
<SHA256 小寫 hex>
```

內建公鑰驗證版本、格式、長度及雜湊；網站只可調整為實際同來源url，不手改其他已簽欄位。實體檔以200 application/octet-stream傳送，不要求登入、不重新導向、不變更位元內容。下載完成再次核對長度與SHA256；失敗不安裝。

## 安裝及權限

原生確認涵蓋指定版本的下載與安裝。NSIS使用PID交接並重新啟動LM_AI，不重新啟動電腦。單獨EXE保留手動替換流程；下載完成不解除已知最低版本門檻。

安裝路徑固定`C:\largan\LM_AI`，一般安裝不自動啟動；Setup內含Python與CAD runtime，不下載／安裝WebView2。缺少WebView2由主程式啟動時提示另行取得。保留使用者檔案與VNC設定，不執行使用者端PowerShell、CMD或BAT。

私鑰不交付。修改安裝器或更新流程後須依變更實測安裝、更新、重啟、失敗回復與移除；公司防毒及內網環境另行驗收。
