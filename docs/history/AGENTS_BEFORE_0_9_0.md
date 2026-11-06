# CompanyAI 專案指引

## 最新授權：VNC與桌寵文字來源調整（2026-11-03，未編譯）

- 使用者要求VNC移回側欄獨立入口；桌寵字級獨立、只有字型共用，固定黑白對比描邊。
- 桌寵font_size保存於pet-settings.json，12～28 px、預設16，舊檔缺欄位沿用預設；不乘主介面字級或縮放，只隨Windows DPI。偏好即時保存，設定頁預覽同步描邊。
- 本輪只修改來源、同步既有測試與文件並做靜態檢查。不進版、編譯、執行原生測試、打包、提交或推送；等使用者下次修改一起驗證。0.8.79已交付成品及其驗證報告保持原狀。
- 下次建置核對VNC設定／登入門檻、桌寵字級保存與舊檔遷移、主介面縮放獨立性，以及各色／透明度／DPI下的原生描邊外觀與裁切。

## 0.8.79必要驗證與封裝完成（2026-11-03）

- R3正式流程退出0；v142 14.29.30133／cl192930159 x64、空Cargo快取frozen、fmt／Clippy、471 Rust、101 CAD契約、原生WebView2基本檢查、44相關UI／57教學／20對話框通過，另行Test-Installer完整生命週期通過。依使用者要求未重跑公司模型、Office或CAD圖面整體驗證。
- CAD清理另以已載入DLL反例補強預檢，7項Rust案例及實際Git歷史環境的成功清理／失敗回復／占用保留通過。原生字型共用既有內嵌資源。
- EXE bc0cde24a49f5ec04e73a7904bc48659ae5d3678458474da9b9c80b07105eea5；Setup 31bdcc5783f2d735fe5be93db9a0033d46f5182965107f31650429eb2121a836。證據offline/*-verification-0.8.79.json；Git交付以實際遠端main及.build/git-delivery-0.8.79.json核對為準。

## 最新授權：0.8.79發行（2026-11-03）

- 使用者已授權本機提醒、桌寵通知共用一般字型、對話框文字欄位改善，完成後進版編譯、必要驗證、NSIS與推送Git；此授權取代下方CAD清理的先不編譯指示，將待測清理一起交付。
- 使用Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller，另核對提醒／字型／清理Rust測試、44項相關UI、登入門檻與Test-Installer.ps1生命週期。跳過Office、CAD實際改圖與真模型整體驗證，不冒稱完整驗收。
- 提醒以DPAPI保存、單次本機時間、200項上限；未登入可用、縮到托盤仍提醒、完全退出後下次啟動補送。0秒／全透明桌寵泡泡改用Windows。提醒不用AI或新增公司資料技能。

## 本輪：CAD舊執行環境清理（2026-11-03，來源修改）

- 使用者回報C:\largan\LM_AI\cad-*累積，確認為安裝包內的CAD Python環境；目前版必需，舊版原本為回復而保留，不是任務反覆輸出的程式碼。
- 已授權在安裝成功後清理已知舊版本；最新明確要求先不編譯，等使用者目前測試結果再決定。本輪不進版、編譯、執行原生測試、打包、Git提交／推送或清理真實安裝目錄，維持已發行0.8.78成品。
- 新增src/cad/cleanup.rs、CLI --cleanup-old-cad-runtimes及NSIS .onInstSuccess掛接；只有新版runtime完整性核對成功，才清理非目前版且manifest雜湊在assets/cad-runtime-history.json中的目錄。歷史19指紋從實際Git發行清單核對產生，工具tools/update_cad_runtime_history.py。
- 所有候選先鎖住目錄與檔案、核對清單及內容；拒絕連結、占用、唯讀、未知／修改／額外項目，以已驗證handle刪除，不遞迴掃刪cad-*。清理放在成功提交後、重啟前，失敗保留並不回復新版本；manifest最後刪除以支援中斷重試。
- 已補原生單元案例及NSIS未知目錄保護案例，僅寫入來源尚未執行。已做rustfmt、PowerShell語法及歷史清單來源核對；下次授權建置時必須跑新增Rust測試、Junction／使用中反例及實際已知舊環境更新／失敗回復／移除驗證，不把靜態檢查當成通過。

## 桌寵插畫追加（本版0.8.78）

- 使用者打包中追加要求精細貓／狗圖畫。已使用內建image_gen生成各一張3×2透明圖集，保存assets/pet-illustrations；Create-PetDefaults.ps1只負責固定分格與縮至256PNG，保留原alpha，不以程式繪製代替AI插畫。
- 本版舊素材建置R1已在release編譯階段停止，未當成正式通過；新12動作及輕量偏好回覆需重新編譯／驗證後再NSIS與Git。精細圖僅在初次、換角色／匯入時傳預覽，滑桿回覆只帶設定，避免放大傳輸成本。

## 0.8.78正式建置完成（2026-11-03 11:04）

- R2正式流程退出0；v142 14.29.30133／cl192930159 x64、空快取frozen、基本原生／WebView2、456 Rust、101 CAD契約、28新UI／57教學／20對話框通過，另行Test-Installer.ps1完整生命週期通過。這不是Office／CAD改圖／公司模型整體重測。
- EXE 243160e18547b127afe5d16affe7c4f3a968dd1efaa83b60f1d309c561904349，Setup 21159f57d3fc332cbe68fc8acb8f1f42e8e246be3213153a54323dff1fa82f72；版本報告offline/*-verification-0.8.78.json。Git交付以實際main及.build/git-delivery-0.8.78.json核對為準，不能僅憑本段假設推送完成。

## 最新發行授權（2026-11-03）

- 使用者要求本批打包後上傳既有Git。進版0.8.78，以Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller完成相關基本建置，另跑全部Rust及Test-Installer.ps1完整NSIS生命週期；UI27項、57教學與20對話框核對依本版再驗證。
- 本輪是桌寵／塔羅／入口UI相關驗證，不重跑Office、CAD改圖或真模型對話，不以基本PASS冒稱整體驗收。封裝成功才提交並推送main，核對遠端commit與固定提交LFS成品雜湊。保留歷史證據與本機封存，不提交私有資料。

## 本輪：桌寵與塔羅體驗調整（2026-11-03）

- 使用者要求實作三款可調顏色／透明度的聊天泡泡、桌寵偏好即時保存、0～5秒通知（0為不顯示）、可編輯／複製的角色生成提示詞對話框。
- 塔羅加長78牌正逆位牌義，同一副牌支援續抽、歷次問題／牌面、不重複牌及同普通聊天續問；洗牌才清空。牌桌於當次應用程式執行期間保留，不宣稱跨重開保存。
- 左側以進階功能整合桌寵／塔羅／Outlook／VNC，保留登入門檻；新增兩項假資料互動教學，總57項。
- 最終R4候選已於11/3 10:23驗證退出0：v142 14.29.30133／cl192930159 x64、空Cargo快取frozen、fmt／Clippy、456 Rust、101 CAD契約、原生WebView2與來源唯讀核對通過；27項新UI／57教學／20對話框門檻通過。證據.build/pet-tarot-followup-verification.json；未重跑真模型、Office或CAD改圖，不冒稱完整驗收。
- 本輪維持0.8.77候選，使用v142空Cargo快取frozen及相關原生／UI回歸；不覆寫dist、不進版或NSIS、不提交／推送。歷史封存規則不限制這次新功能實作。

## 目前狀態與授權

- 上一個發行版為 **0.8.77**，提交 `fa2833f641694554aac25483979becc17cb5c59d`；EXE／NSIS 已推送並以固定提交下載核對。驗證範圍與限制見 docs/validation/VALIDATION_0_8_77.md。
- 2026-11-03 使用者授權整理本機及 Git：0.8.74 及以前的版本專屬成品與歷史紀錄移至 `Y:\Rust\舊版留存\CompanyAI\`，不提交封存內容；保留 0.8.75～0.8.77、現行規格及建置依賴。
- 本次只整理檔案與文件，提交／推送既有 origin/main；不進版、不編譯、不重製 EXE／NSIS，也不操作使用者正在測試的應用程式。Git 歷史不改寫，整理政策見 docs/development/ARCHIVE_POLICY.md。
- 使用者正在實測。後續待討論事項是多成果任務進度、工具識別資訊及 CAD 未存檔成果的完成判定；本次未授權實作這些功能。

## 專案界線與程式碼風格

- `Y:\Rust` 是共用根目錄，本專案是 `Project/CompanyAI` 內的獨立 Cargo workspace。原始碼、Cargo.lock、vendor、.cargo、target、dist 與 scripts 留在專案內。
- 不移動或修改其他專案、`.local-ai`、共用 `.tools`、安裝下載與共用環境腳本；不在共用根目錄建立 Cargo workspace 或 .cargo/config.toml。
- 優先使用明確命名、簡單控制流程與單一責任函式，避免不必要的抽象、泛型及巨集。以繁體中文註解說明用途、限制與設計原因；公開 Rust 介面使用 `///`。
- 錯誤訊息應說明原因與可採取的動作。修改邏輯同步更新註解與文件；不以無理由的 unwrap/expect 取代錯誤處理。

## 建置與驗證

- Windows 11 x64、MSVC **v142 / 14.2x**；已驗證 14.29.30133、`_MSC_FULL_VER=192930159`、SDK 10.0.19041.0。Rust 由 rust-toolchain.toml 固定。
- 使用 scripts/Enter-DevShell.ps1（vcvars64.bat -vcvars_ver=14.2）及 scripts/Build.ps1；確認實際 cl/link 與 x64，不使用新版 VS 預設工具集或 target-cpu=native。
- 離線依賴保留 Cargo.lock、vendor、.cargo/config.toml，以空 Cargo 快取及 --frozen 驗證。不要把歷史離線 ZIP 當成目前依賴。
- BuildOnly 僅編譯；ValidateOnly 不更新發行產物；BasicValidation 不代表完整驗證。Office、VNC、SMB、CAD 與 NSIS 依變更範圍及當次授權實測，缺少環境如實回報。
- 真模型、固定參數原生工具、幾何／內容及畫面核對分開記錄。保留原始參數與失敗，不由測試程式替模型補正；流程見 docs/development/LOCAL_MODEL_VALIDATION.md。
- CAD 使用獨立新圖或副本，保留使用者原圖。修改、Undo、保存重開及文件身分分開核對；工具成功不代表圖面設計核准。
- 0.8.77 完整建置與 NSIS 通過，但 Luna 多檔交付及 CAD 結尾仍有未完成案例；桌寵拖曳／通知點擊、公司模型、真 Outlook 及加密文件仍需現場驗收。不得把原生測試 PASS 當成上述情境通過。

## 持續有效的產品約定

- 公司內資料技能由網頁端提供，應用程式不額外載入。公司主機與路由由 src/config.rs 管理，前端不取得 Token、郵件 EntryID 或任意執行能力。
- 一般聊天與專案保存請求身分；未知提交查原 ID，已確認暫時推論失敗才可依既定退避建立新請求。不可重播成功的本機寫入。
- 原始文件預設唯讀，使用受控副本與版本核對；來源變更、權限與路徑保護不能為了讓任務完成而略過。
- Classic Outlook 經使用者授權讀取，不寄信或改信；資料夾與郵件代號受原生範圍限制。模擬資料不能代替公司信箱實測。
- 設定與私有紀錄依現有 DPAPI／帳號隔離保存；關閉視窗、托盤及快捷鍵遵循現行使用指南，快捷鍵只填草稿、不自動發送。
- 桌寵提供貓／狗及六動作，僅匯入 PNG，建議透明背景，保留原圖並壓縮至每張小於500,000 bytes；角色置前的生成提示可由使用者修改。
- 塔羅離線78牌、單／三張、正逆位；牌面旋轉呈現逆位。AI解析要求問題並使用確切抽牌結果，送一般聊天，不新增塔羅技能。
- Excel公式技能允許合法自訂公式，與常數資料分工，計算後讀回值與錯誤。技能說明不能取代程式的授權及輸入驗證。
- NSIS 固定安裝 C:\largan\LM_AI，內含 Python／CAD runtime；WebView2 分開提供。一般安裝不自動啟動，更新沿用 PID 交接與重新啟動契約，不執行使用者端 PowerShell／BAT。

## 發行與資料留存

- 只有當次授權發行才進版、更新 dist、封裝及推送。正式交付同步 Cargo版本、公告、文件、成品與兩份簽署清單；核對遠端 main 及固定提交 LFS 下載雜湊。
- 更新私鑰、個人設定、Token、模型對話、.build、.codex、dist/onboarding-state.json、icon.png 與本機「舊版留存」不提交。
- 不因檔案建立較早就移走現行原始碼、授權、vendor 或重建輸入。offline/GstarCAD-Probe.zip 與 python-inputs.zip 是目前重建依賴，保留。
- 封存保留原相對路徑、搬移清單與檔案核對結果；不得靜默覆寫已有留存。歷史指示以最新使用者授權為準，不把舊交接的等待／暫停當成當前狀態。
