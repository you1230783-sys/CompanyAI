# 0.9.26 登入恢復與公鑰建置檢查

日期：2026-11-07。依使用者後續審查修正並進版 0.9.26。允許本機編譯、打包驗證及推送 GitHub 原始碼；不建立標籤或發布 Release，不部署公司更新伺服器。0.9.25 已完成的其他修正與限制見前版驗證文件，手冊原文保留。

## 登入恢復

- `device` 或 `token` 階段收到通過公鑰驗證的 `transport_v2_unavailable` 後，新的 HTTP 登入資料連同 `verified_v1_fallback` 證據一起以 DPAPI 保存。證據不含額外 Token 或帳號資料。
- 重開程式時，只有防降級紀錄有效、回退證據成立、Session 尚未過期且路由／Header 綁定一致，才使用 HTTP 設定恢復登入。可信 test_accounts／off 回退不再造成每次重開重新授權。
- 新登入仍重讀 Windows 設定檔層級的 v2 紀錄並先協商 v2；HTTP Session 的恢復不解除這項限制。連不上、TLS 錯誤及釘選不符仍不得冒充可信回退。required 的 HTTP 426 升級回應仍使對應 Session 失效並要求重新登入。
- 審查建議假設「建立 v2 紀錄前一定已覆蓋 Session」，但舊版實際先寫紀錄再保存 Session。0.9.26 改為先寫 Session、再寫紀錄，並採明確證據，避免把舊版寫入失敗留下的 HTTP Token 當成允許回退。**舊版沒有回退證據的 HTTP 資料可能需要重新登入一次**；新版取得可信回退後可跨重啟恢復。
- 紀錄損壞仍開啟介面並顯示原有警告，保留 v2 限制，不自動清除紀錄。Token 或 DPAPI 資料本身損壞的既有錯誤處理不屬於這次擴充。

## 公鑰與產物界線

本版 `PRIMARY_SPKI`／`BACKUP_SPKI` **均為佔位值（placeholder）**，沒有換上正式金鑰。HTTPS loopback 驗證使用臨時測試金鑰；不代表已連線公司正式伺服器。發行收據記錄相同狀態與本輪成品雜湊。

- `Build.ps1 -RequireHttpsPins` 在編譯前拒絕佔位值。即使不帶開關，格式錯誤、重複雜湊或一個佔位值搭配一個已設定值也會拒絕。
- `pins::configured()` 專用單元測試直接檢查實際常數，只接受 `Ok(None)` 或 `Ok(Some(_))`，不可為錯誤。Rust 與 PowerShell 均要求兩個佔位值或兩個有效且不同的雜湊。
- BuildOnly、基本／完整建置、CI 與本版安裝包收據記錄 `https_pins`。獨立封裝核對 `.build/https-pins-build.json` 的 EXE／設定來源雜湊，防止把新來源狀態標在舊 EXE 上。
- `configured` 只表示雜湊格式已配置，不自動證明其為公司正式金鑰；`server_verified` 明列 false，直到公司實測另行記錄。

## 本輪驗證

| 項目 | 本機結果 |
| --- | --- |
| Rust 程式庫 | `cargo test --lib --frozen -j 2 -- --test-threads=1`：614 通過、0 失敗、3 個需外部 fixture 的案例由一般測試略過；其中 TLS fixture 另行執行如下。 |
| 登入回歸 | 真 DPAPI 模擬先 v2 登入、登出、可信 device／token 停用後 HTTP 登入，再走正式啟動恢復函式；兩條回退路徑均保留 Session。舊版無證據、未驗證、過期、路由不符、紀錄損壞及 Session 寫入失敗分支通過。HTTP 426 只使對應 Session 失效的既有測試通過。 |
| 公鑰設定 | 實際 Rust 常數測試通過；PowerShell 8 個合成案例通過。實際 `Build.ps1 -BuildOnly -RequireHttpsPins` 與 `Build-Installer.ps1 -RequireHttpsPins` 都在佔位值時如預期拒絕；舊 EXE 與新建置收據不符時亦拒絕封裝。 |
| 格式／靜態檢查 | `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --frozen -j 2 -- -D warnings` 通過。 |
| 本機編譯 | `Build.ps1 -BuildOnly -EmptyCargoCache` 通過；Rust 1.98.1、MSVC 14.29.30133、實際 `_MSC_FULL_VER=192930159` x64、SDK 10.0.19041.0、預設全功能。BuildOnly 收據本身不宣稱執行過測試，測試證據分開保存。 |
| TLS | `Test-HttpsPins.py` 四項通過：主／備用測試金鑰可用，錯誤公鑰與錯誤主機名稱均收到 0 HTTP bytes，沒有 Authorization 或本文外送。未替換公司常數。 |
| 前端 | `Test-Settings.cjs` 的 14 組檢查通過，含 58 份教學、設定版面與公告禁用詞／0.9.26 版本唯一性。 |
| 封裝 | 完整 Python／CAD runtime 與 NSIS 建立通過；兩份更新清單內容相同，原生更新器接受簽章及精確套件雜湊。 |
| 實際安裝包 | `Test-ReleaseInstaller.ps1` 通過：固定路徑、公司資訊、捷徑與註冊、缺 WebView2 的子程序提示、安裝後 WebView2／Python／CAD 自檢、同版更新保留 runtime 身分及使用者檔案、更新後自檢、解除安裝保留未知檔案。測試後安裝目錄清理完成。 |

本輪安裝驗證使用實際 0.9.26 候選 Setup，EXE 與建置收據一致。未變動的完整安裝失敗／PID／修復矩陣沿用 `offline/installer-verification-0.9.19.json`，不宣稱這次重跑完整矩陣，也不宣稱完成公司端帳號驗收。先前本機 dist 產物另留於 `.build/before-0.9.26-*`；這次成品只存本機，不上傳 Release。

公開收據：[來源檢查](../../offline/source-verification-0.9.26.json)、[安裝驗證](../../offline/installer-verification-0.9.26.json)、[本機發行候選與公鑰狀態](../../offline/release-verification-0.9.26.json)。最後一份記錄 EXE、Setup、兩份更新清單及 SHA256SUMS 的完整 SHA-256；`https_pins.mode=placeholder`、`production_pins_replaced=false`、`release_published=false`。

本機主要成品：EXE `f9650b5c1ef29441500e2c5d5f95d6cfe51fe07bc8b68cd03b12fe678ac5d6ad`；Setup `671335df49974fc2347621189a44f73d6c4f150afa2a53e13afb9d6dc33fab54`。更換正式公鑰後，這兩個雜湊不可作為新產物的驗證證據。

## 正式公鑰提供後

保留這次未受影響的功能與安裝生命週期驗證；替換兩個正式雜湊後，以 `-RequireHttpsPins` 重新編譯／封裝，重跑公鑰常數測試，核對新包與兩份簽署清單、SHA-256，並由原生更新器驗證。新成品另產收據，不沿用佔位版 EXE／Setup 雜湊。Release 必須等待使用者後續正式金鑰與發布指示。

待公司實機：同一 Windows 帳號先以測試帳號 v2 登入，登出後用非測試帳號回退，完整離開再啟動不重複登入；伺服器切 off 的恢復；required 模式要求升級；正式／備用公鑰的連線及輪替。0.9.25 的 Windows 實際登出／關機等待實機項目不因本輪合成測試自動視為通過。
