# 0.8.66 基礎驗證

狀態：PASS_BASIC，2026-10-15。EXE／NSIS 0.8.66.0。

本版新增一般對話「解釋」、設定「教學與更新日誌」、14個獨立主題及重看功能。第三輪CAD公司測試因超時暫停，未視為通過；本版不調整上游推論排隊或CAD行為。

基礎範圍：v142 x64、空Cargo快取與--frozen、選定Rust契約、Python合成契約、正式EXE基本檢查、NSIS及更新清單。另核對教學模擬DOM的登入門檻、依序閱讀／返回、重看、登出與更新日誌。

未涵蓋完整驗證、公司服務實測、CAD實機與NSIS安裝／更新／回復／移除生命週期；卡片外觀由使用者安裝後驗收。

## 已完成

- MSVC v142 14.29.30133，實際_MSC_FULL_VER=192930159 x64；空Cargo快取、vendor、--frozen、fmt與Clippy通過。
- 63項選定Rust契約、57項CAD Python合成契約通過。
- 教學模擬DOM互動與JS語法檢查通過，正式EXE基本檢查、Python runtime及Outlook匿名管線檢查通過。
- NSIS與簽署更新清單核對通過；紀錄見.build/build-0.8.66-release.log與offline/exe-verification.json。

## 產物

| 檔案 | bytes | SHA256 |
|---|---:|---|
| LM_AI.exe | 18660352 | ff1fc687f8a3c7e3ad463167fe99a05de0eaac0cffb3e4675bf95f99f798c5da |
| LM_AI_Setup.exe | 36107816 | 2fe5c188f57131aea5774197ae91b50b880ce4e20b88ed0239dc79442f2ba838 |
| update-manifest.json | 1050 | a569d9aebebe9fa46935c54e453d4cc9b9b501b539cfe1c58c29a7a77789f69b |
| update-manifest-exe.json | 1050 | a569d9aebebe9fa46935c54e453d4cc9b9b501b539cfe1c58c29a7a77789f69b |
