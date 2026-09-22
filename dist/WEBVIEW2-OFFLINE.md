# WebView2 離線安裝

LM_AI 使用 WebView2 顯示桌面介面。0.8.8 的最新 NSIS 已改為精簡包，不包含或自動下載 WebView2。Windows 11 通常已安裝；若 Setup 提示缺少執行階段，請從公司另行提供的下載位置取得 `MicrosoftEdgeWebView2RuntimeInstallerX64.exe`，安裝後重新執行 `LM_AI_Setup.exe`。直接使用單檔主程式者則重新開啟 `LM_AI.exe`。

獨立安裝程式仍保存在 [Git 的 dist 目錄](https://github.com/you1230783-sys/CompanyAI/blob/main/dist/MicrosoftEdgeWebView2RuntimeInstallerX64.exe)，使用 GitHub 的 Download raw file 下載實際安裝檔，或以 Git LFS 取得，不能把 LFS 指標文字當成 EXE。公司可將這份檔案放到自己的內網下載區，僅供缺少 Runtime 的電腦安裝一次，不必每次 LM_AI 更新重複下載。
此檔為完整 x64 Evergreen Standalone Installer，安裝時不需下載 WebView2 主程式。公司安裝政策／權限仍由 IT 管理。
不需要 Rust、VS、Node 或 Python 才能使用 App。Classic Outlook 需另外已安裝並開啟。

- 來源：Microsoft 官方 `https://go.microsoft.com/fwlink/?linkid=2124701`
- 取得日期：2026-09-17
- 大小：212745424 bytes
- 安裝包 FileVersion：1.3.265.7（安裝包本身版本，不代表 WebView2 引擎版本）
- Authenticode：Valid，簽署者 Microsoft Corporation
- SHA256：`ebebc5ec130378ff1ab513f3917be791a9cf84f849e970b1695ff01801a9d348`

可用 `Get-FileHash .\MicrosoftEdgeWebView2RuntimeInstallerX64.exe -Algorithm SHA256` 比對。
安裝包同時放入完整 Rust 離線 ZIP。App 不會自動執行安裝包，也不需要從 CDN 載入 Markdown、公式、字型或高亮套件，這些已嵌入 EXE。

部署說明：[Microsoft WebView2 distribution](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)。
