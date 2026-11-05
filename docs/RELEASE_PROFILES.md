# 發行配置量測與選擇

0.9.8採用`lto = "thin"`、`codegen-units = 16`，明確保留`panic = "unwind"`與`strip = "debuginfo"`。Windows v142 x64、靜態CRT及一般x64指令集不變。

## 可重跑方式

從專案根目錄執行`scripts/Measure-ReleaseProfiles.ps1`。腳本依序呼叫正式`scripts/Build.ps1 -BuildOnly -EmptyCargoCache`，每個候選使用新建的獨立target及空Cargo快取，從vendor以`--frozen`編譯。實際cl探針與linker仍由Build.ps1核對；不修改dist、簽署清單或安裝包。每組只量測一次，記錄環境檢查、資源核對及編譯的整體耗時；未清除作業系統檔案快取，不把差異宣稱成穩定編譯速度提升。

量測輸出在`.build/release-profiles-<唯一編號>/`，保留各組原始日誌、來源提交與未提交的相關差異、秒數、EXE大小及SHA256。三組之間不得修改來源；腳本會核對差異。`BuildTargetDirectory`僅供預設功能的BuildOnly，路徑限制於專案`.build`，不能用於正式驗證或封裝。

## 2026-11-05結果

來源為`51bec02978a96333393fcd23b89c4af79f0c3ae1`的0.9.7應用，加上隔離量測輸出所需的Build.ps1介面；三組應用來源一致。完整證據見[量測JSON](../offline/profile-measurements-0.9.8.json)。正式0.9.8成品另含版本及公告更新，大小／雜湊以[本版驗證](VALIDATION_0_9_8.md)為準。

| 配置 | EXE bytes | 整體秒數 | 比原配置縮減 |
|---|---:|---:|---:|
| 原配置：lto=false、16 units | 69,519,360 | 278.663 | — |
| ThinLTO、16 units | 68,926,976 | 226.340 | 592,384 bytes（約0.85%） |
| ThinLTO、1 unit | 68,356,608 | 505.4（四捨五入） | 1,162,752 bytes（約1.67%） |

選16 units：1 unit僅再減少570,368 bytes，單次建置成本卻超過兩倍；保留平行codegen較適合目前維護與CI。沒有量測應用執行效能，不能宣稱使用者操作變快；執行相容性另由正式原生回歸驗證。

全部候選實測MSVC v142 14.29.30133、`_MSC_FULL_VER=192930159` x64與SDK 10.0.19041.0。Rust固定1.98.1。沒有使用新版VS預設工具集、`target-cpu=native`或panic abort。

[Cargo官方profile說明](https://doc.rust-lang.org/cargo/reference/profiles.html)區分`lto=false`的crate內ThinLTO與`lto="thin"`的跨crate最佳化；降低codegen units也會改變平行編譯程度。此處依本專案實測選擇，不將官方描述當成本機效能證據。
