# NSIS 獨立安裝驗證

- 此專案只測試最小 Win32 工具的安裝、啟動與解除安裝，不修改 CompanyAI。
- 工具使用 C++ / Win32，必須以 MSVC v142 x64、靜態 CRT 編譯。
- NSIS 使用官方 3.12 免安裝發行包；編譯工具只放在本專案 .tools。
- 交付給使用者的工具及安裝／解除安裝流程不得啟動 PowerShell、CMD、BAT、VBS 或下載其他元件。
- scripts 下的 PowerShell／CMD 僅供開發機編譯與驗證，不放入使用者測試包。
- 安裝位置及登錄項目使用獨立名稱 LARGAN_NSIS_Probe，不操作 LM_AI 或 CompanyAI 資料。
- 不把本機測試通過描述成公司防毒驗收通過，不停用或調整防毒設定。
