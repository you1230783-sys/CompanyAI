# Cargo選用模組

0.9.6將`cad`、`pet`、`tarot`及`vnc`設為獨立Cargo features。四項預設全部啟用，正式NSIS與公司更新仍發行完整版；使用者不需要調整設定。

## 邊界與相容性

| Feature | 停用後的行為 |
| --- | --- |
| `cad` | 不編入CAD工作程序與runtime管理；模型工具目錄移除CAD操作，Broker拒絕執行及沿用CAD成功結果。舊專案資料、幾何規格與紀錄型別保留，仍可檢視既有紀錄。CAD自檢與runtime清理回報未包含功能，不回報假成功，也不刪除既有環境。 |
| `pet` | 不建立桌寵視窗、不編入桌寵圖片；原桌寵提醒改用Windows通知，舊提醒與偏好保留。截圖預覽所需PNG編碼獨立保留於核心。 |
| `tarot` | 不編入牌局邏輯、牌面與牌背圖片；隱藏主介面及桌寵選單中的塔羅入口。 |
| `vnc` | 不編入VNC管理、同步、遠端連線與專用HTTP處理；停用設定入口，保留原有`vnc_enabled`偏好及machines.json，不讀改或清除機台資料。 |

原生`Command`不接受未編入的Pet／Tarot／VNC命令，CAD也在原生工具層拒絕。網頁的`build_features`僅控制呈現，不能重新啟用原生模組。教學選單依能力篩選；獨立教學預覽仍可展示完整功能。

共用JS／CSS保留以維持離線頁面載入；核心所需Windows綁定與共用依賴不因停用模組而移除。本版不宣稱依賴數或正式完整版EXE體積減少。LTO另於後續版本驗證。

## 建置與驗證

使用專案的v142入口；選用組合目前僅提供開發預覽，不產生可發布的精簡安裝包：

```powershell
# 只有核心：文件、Outlook、Python、截圖與一般對話等仍保留。
.\scripts\Build.ps1 -BuildOnly -NoDefaultFeatures

# 核心加CAD與VNC。
.\scripts\Build.ps1 -BuildOnly -NoDefaultFeatures -Features cad,vnc

# 正式完整版與離線空快取驗證。
.\scripts\Build.ps1 -EmptyCargoCache -IncludeInstaller -TestOffice

# 所有16種組合的Clippy及核心Rust測試。
.\scripts\Test-Features.ps1
node scripts/Test-BuildFeatures.cjs
```

自訂組合輸出位於`.build/feature-preview-target/`，不覆蓋正式target或dist。`Build.ps1`拒絕將自訂features與正式驗證／NSIS合用；未提供獨立更新通道之前，不將精簡版放入公司更新資料夾。公司更新同源、簽章、HTTP與安裝路徑全部維持原約定。

`BuildOnly`只產生EXE，不準備Python或CAD執行環境。若要測試Python工具，使用`Stage-Python.ps1 -ExecutableDirectory <EXE所在目錄>`；啟用CAD的預覽另依`Stage-Cad.ps1`準備環境。兩者仍需要本機鎖定雜湊的離線輸入，不能把單獨EXE當成完整安裝包。

純搬移的PNG helper提交為`3ef6244`，對照見`refactors/0.9.6-png-preview.json`；與後續feature行為變更分開。矩陣編譯、原生測試及正式封裝的實測結果記於[0.9.6驗證紀錄](../validation/VALIDATION_0_9_6.md)。
