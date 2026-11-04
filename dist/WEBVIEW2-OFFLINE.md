# 安裝 Microsoft WebView2 Runtime

LM_AI 需要 WebView2 Runtime。從 0.9.1 起，本儲存庫與 GitHub Releases 不再提供 WebView2 安裝包；LM_AI 的 NSIS 也不包含或自動下載它。

1. 開啟 [Microsoft 官方 WebView2 下載頁](https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section)。
2. 在 **Evergreen Standalone Installer** 選擇 **x64**。請使用完整安裝包，不是需要連網下載元件的 Bootstrapper。
3. 公司電腦無法上網時，可在可連網電腦下載，再由 IT 放到公司內網或複製至目標電腦安裝。
4. 完成後重新開啟 LM_AI；不需重裝 LM_AI。安裝權限及更新政策依公司 IT 規範。

Windows 11 通常已有 Runtime；Edge 瀏覽器與 WebView2 Runtime 是不同元件。請以實際啟動結果確認，不能只看有無 Edge。

下載後可檢查檔案的 Microsoft 數位簽章。Evergreen 版本會更新，本文不提供會過期的固定 SHA256。

參考：[Microsoft 官方部署說明](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)。本機介面的字型、公式與圖表資源仍隨 LM_AI 提供，不依賴 CDN。
