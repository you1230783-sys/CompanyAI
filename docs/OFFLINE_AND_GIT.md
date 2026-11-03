# 離線交付與 Git

現行版本與下載見[首頁](../README.md)，建置方式見[開發說明](DEVELOPMENT.md)。目前發行EXE與NSIS均為0.8.78；舊完整Rust離線ZIP已移至本機[舊版留存](ARCHIVE_POLICY.md)，不再作為目前下載項目。

## 現行檔案

| 位置 | 用途 |
| --- | --- |
| `dist/LM_AI_Setup.exe` | 正式NSIS安裝包，含Python及CAD執行環境 |
| `dist/LM_AI.exe` | 主程式；需搭配完整安裝目錄 |
| `dist/update-manifest.json`、`update-manifest-exe.json` | 本版兩份相同的NSIS簽署清單 |
| `dist/MicrosoftEdgeWebView2RuntimeInstallerX64.exe` | 另行提供的WebView2離線安裝包 |
| `offline/*-verification*.json` | 現行與最近三版驗證，範圍以檔案內容為準 |
| `offline/python-inputs.zip`、`python-lock.json` | 鎖定的Python重建輸入 |
| `offline/GstarCAD-Probe.zip` | CAD重建所需官方輸入，並非新版LM_AI安裝包 |
| `Cargo.lock`、`vendor/`、`.cargo/config.toml` | 固定Rust依賴與離線來源設定 |

vendor與工具鏈不直接提交Git，開發機須另行準備；clone原始碼本身不等於取得完整離線編譯環境。既有vendor與共用`.tools`本次保留。不能將歷史ZIP覆蓋到目前原始碼。

## 發行

收到明確發行指示後，依變更範圍使用`scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`及相關Office／CAD／SMB／VNC選項；確認v142 x64、空Cargo快取frozen與實際驗證結果，再發布成品及簽署清單。

需要重新製作完整Rust離線包時，另行使用`scripts/Prepare-Delivery.ps1`並完成解壓重建驗證。此腳本仍保留，但本次未製作新整包，也未將歷史整包當成0.8.77交付。

## Git與核對

遠端為`https://github.com/you1230783-sys/CompanyAI.git`，分支main。依當次授權提交／推送，不納入`.private`、Token、個人設定、原始模型對話、`.build`或封存區。

EXE／ZIP／字型依`.gitattributes`使用Git LFS；clone後需取得真正內容，不能使用LFS文字指標執行或解壓。提交前核對差異與檔案範圍；推送後核對遠端commit，從固定commit下載成品比對SHA256。

歷史檔案從最新分支移除後仍存在過去commit／LFS中。本輪不改寫歷史或刪除遠端LFS物件。Git交付也不代表公司內網服務已部署。
