# 0.8.69 CAD 批次讀取整合與交付驗證

狀態：0.8.69 整合、兩版本實機／Luna GUI、v142 完整建置及 NSIS 安裝生命週期全部通過。本次交付原始碼、EXE、完整 NSIS 與兩份簽署更新清單至既有 Git main。

## 實作範圍

- 固定 LISP 匯出器與 Python 轉換器納入逐檔雜湊 runtime；模型不能指定 LISP／COM 方法或輸出腳本。
- Line／Circle／Arc／Text／MText／直線折線／圖塊基本資訊轉成現有索引；保留折線子圖元與圖塊屬性。傾斜 OCS、擬合折線、未知類型及必要屬性使用原有 COM 詳讀，不猜測幾何。
- 完整圖元保留本機，查詢／候選預覽／已讀 Handle 授權沿用；同數量手動修改仍在寫入前完整重建 revision 並拒絕舊計畫。
- 修改前、存檔前與重開後都使用批次讀取，COM 物件按需取得。未指定圖元另比較原生資料雜湊；不減少圖層鎖、文件身分、原件／副本及可讀屬性核對。
- 新建繪圖的前後 Handle 診斷不再逐次全圖建立 COM 包裝；只詳讀本操作新增的圖元。全圖縮放／維護指令的模型空間核對也使用批次讀取，圖紙空間沿用原有詳讀。
- 明確數量不符時才使用完整 COM 備援；逾時／無完整結尾／解碼錯誤／文件切換不自動重送或交付部分索引。
- 讀取／繪圖／修改含重開核對最多十分鐘；inventory／probe 維持原期限。讀圖提示不搶焦點，可取消任務，進入 idle／結束即收起。
- ProgID 大小寫沿用 Windows 語意；成功建立同來源快照會取代舊 coverage 錯誤。編輯方法加入正確 radius 範例，第一次修改前提示先讀方法。

## 原生實機測試

測試透過正式 Python worker／COM／批次匯出及修改引擎，沒有模型代替工具結果。來源與成果均為 .build 內合成圖，不修改使用者原有圖檔。

| 案例 | 2008 | 2022 |
| --- | --- | --- |
| 同一份 20051 圖元 DWG：完整副本讀取 | 50.90 秒 | 21.61 秒 |
| 單圓修改、存檔前／重開後核對 | 152.67 秒，通過 | 60.12 秒，通過 |
| 完整 645 圖元法蘭：新建、樣式／幾何核對、存檔重開 | 417.13 秒，通過 | 240.64 秒，通過 |
| 法蘭完整副本讀取 | 13.33 秒 | 7.06 秒 |
| 法蘭單孔修改、存檔重開 | 49.26 秒，通過 | 38.24 秒，通過 |

上述為單次實測，部分階段同時有建置工作，不作固定倍率承諾。新建 600 段曲線的逐段寫入仍有 COM 成本；本版沒有將模型繪圖操作改成任意 LISP。

20051 圖元案例均獨立讀回目標圓半徑為 2.5，來源檔雜湊仍為 `d81deb289b625ab6b5bc691f2cafb20981fd599ee12c4c52cd0898a03321b4ec`。法蘭修改也獨立讀回半徑，來源各自保持不變；尺寸、引線、剖面及其他未指定圖元通過原生資料保護。

原生紀錄：

- `.build/integrated-edit-5b7a8a64180b4a948b0c3bef6b74cd82/result.json`：2008 大圖。
- `.build/integrated-edit-3874e8247cbe448699bfd7201b9e6858/result.json`：2022 大圖。
- `.build/native-flange-069-478a6558aa2346c7b3226bb0e18841a1/report.json`：2008 法蘭建立。
- `.build/native-flange-069-1042267da5474bf8b719aaa1cbf7c823/report.json`：2022 法蘭建立。
- `.build/integrated-edit-9af4740ffa7941b1b9bf2b7ae9476cbb/result.json`：2008 法蘭修改。
- `.build/integrated-edit-2953a460a1e9478a99b7944628a7dacf/result.json`：2022 法蘭修改。

第一輪 2022 新建測試因未儲存文件的相對 FullName 被誤判而停止，只留下空白測試文件；已修正相對／絕對位置核對後另建新文件通過。原失敗文件保留，沒有自動接續部分圖面。

原生邊界測試另確認：2008 把附屬 ATTRIB 列入 ModelSpace，而 2022 只列主 INSERT。已依真實子 Handle 補齊 2008 的屬性圖元，保持唯讀授權，輕量 Handle 診斷也保留同樣集合，不把合理差異誤判為缺漏而全圖 COM 重讀。

- 2008 `.build/cad-edge-069-c3fa7d1657c24d4b8cbe776d984b6083/report.json`：輕量／傳統折線、中文圖塊屬性、傾斜 OCS COM 備援、同數量修改拒絕舊 revision、Handle 診斷、來源檔不變均通過。
- 2022 `.build/cad-edge-069-92b1bcdffa214b3eae19854ed23a7af6/report.json`：相同邊界與過期 revision 通過，另確認作用中修改為 EDITED_UNSAVED、沒有儲存或改寫來源檔。
- 2008 原廠介面不提供 StartUndoMark／EndUndoMark，作用中圖元修改仍維持原有限制；本版以副本修改測試，不假稱解除這項限制。
- 最終來源的 2022 邊界重跑：`.build/cad-edge-069-ae108fcba6e34b5bb15c55fb737aeec3/report.json` 全部通過。
- 邊界圖另做副本單圓修改、存檔及重開：2008 為 6 個 COM 圖元（含附屬 ATTRIB），讀取 2.88 秒、修改核對 19.78 秒；2022 為 5 個主圖元，讀取 1.95 秒、修改核對 12.81 秒。兩者獨立讀回目標半徑、來源雜湊不變，傳統折線頂點與中文屬性保護均通過。紀錄為 `.build/integrated-edit-c5c5f82324f344c0b8691beb5c8bafb8/result.json` 與 `.build/integrated-edit-80383b1e9ba947749f176caaa0fe68f6/result.json`。

## 候選版檢查過程

第一個固定 runtime 候選版通過 348 項 Rust 與 68 項 CAD 合成測試；WebView2 自檢第一次在既有圖表畫布檢查失敗，同一 EXE 單獨重跑 `--self-check` 返回 0。正式建置仍須完整重跑，不把這次部分結果當成發行通過。

GUI 工具起初無法切換前景（`foreground window did not report a process id`），重設工具並點選已觀察的應用視窗後恢復。2008 真正 GUI → Luna low 已成功建立 645 圖元索引、查回四個精確孔位及顯示局部預覽，使用者選取 13 號／Handle 166；快速模型的請求未提供 analyze_image。第一輪模型誤把工具的「指認不是寫入授權」解讀成需再次詢問指認，已分開成功選取／關閉兩種說明，明確提示按原要求進入修改工具，由原生端保留既有確認。最終候選版驗證續記於下方。

## Luna low 與 GUI 實測

真實應用介面 → localhost 8770 → `gpt-6-luna`／`low` → 正式 runner／Broker／固定 CAD runtime；不是以預錄工具結果代替模型。快速模式所有 38 輪請求均未提供 `analyze_image`，沒有傳圖。局部預覽只給本機使用者查看。

- **2008**：645 圖元索引 → 四個 Ø12 紅孔精確查詢 → 局部預覽選 13 號／Handle 166 → 修改半徑 7.5 → 存檔重開核對 → 新副本查回直徑 15 → 完成。第一輪模型重問指認，人工補充後接續；其後兩次參數錯誤（radius 欄位格式、cad_query 多帶 revision）均在執行前拒絕，有限修復後成功，不能稱為零錯誤自主流程。來源 SHA256 保持不變，獨立 COM 讀回成果為 645 圖元、目標半徑 7.5。
- **取消**：2008 的 20,051 圖元合成圖，GUI 顯示已讀約 3,250、CAD 回報總數及經過時間，非模態操作提醒可見。按「取消讀取並停止任務」後回報停止、提示收起、未發布部分快照，也沒有重送模型／CAD 讀取。CAD 內部已送出的唯讀匯出可能繼續完成，終止 Python 不等於終止 CAD 指令，這項限制仍保留。
- **2022 最終候選版**：645 圖元索引與四孔查詢通過；同參數 `refresh=false` 回報 `reused=true`，snapshot_id 不變。預覽選 633 號／Handle 270（190,115,0），模型讀編輯方法後進入修改，沒有再問同一個指認問題。三次參數錯誤（詳細頁超過10筆、漏 snapshot_id、錯誤 operations／revision）均在執行前拒絕，模型自行修正後只改半徑為 7.5、存檔重開核對 645 個 Handle 並完成。獨立 COM 確認其餘三孔仍為半徑 6、四孔仍紅色、目標圓心不變，來源檔雜湊不變。
- 所有查詢 offset=0 都是四孔／單孔的條件查詢，沒有 0／100 全圖分頁循環。快取命中沒有再次讀 CAD；圖面資料仍由本機索引按需交付。

2008／取消測試使用候選 EXE SHA256 `ed4c740bd1f7449dabd11ae15bc98062f4bc55cfc886bdb05119e9e96aad859f`；2022 使用修正指認說明的最終候選 EXE `29b6c6ed141328d8e460f670cfc1cbf7e0e76d65aa7d5d2713930573182ee56e`，CAD runtime 相同。最終產品以正式建置產物為準。

本機證據 `.build/luna-local-1175b757cd4b42eab444cd0ee689530a/summary.json`（第1–21輪2008；22–23取消；24–38為2022）及 `native-2022-verification.json`。連線授權碼、聊天與本機測試圖均不納入 Git 或安裝包。Luna 的少量參數修復仍有推論成本，不能用本機服務推定公司排隊／token 上限成因。

## 發行建置與驗證

- 最終候選版通過 `Build.ps1 -EmptyCargoCache -ValidateOnly`，退出碼0，紀錄 `.build/build-0.8.69-luna-final.log`。
- 正式 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 完整重跑通過，退出碼0，紀錄 `.build/release-0.8.69-final.log`。機器報告見 `offline/exe-verification.json`、`offline/installer-verification.json` 及本版 Python／批次／程式／LOG 驗證報告。
- NSIS 隔離套件與正式安裝包的安裝／更新／重啟／鎖檔失敗回復／解除安裝全部通過。Python 沿用、缺檔／同大小損壞／清單變更修復、應用回復後保留沿用 Python、使用者檔案保留，以及安裝後 Python／CAD 自檢均通過。
- `update-manifest.json` 與相容端點 `update-manifest-exe.json` 完全相同，版本0.8.69、kind=nsis、指向本版完整安裝包；原生更新器已驗證簽章／套件，另核對實際大小與SHA256。EXE／NSIS 以 Git LFS 保存，測試圖、連線、聊天及個人設定不提交。Git 發行不代表公司內網已部署。

正式建置的完整核心驗證已全部通過，使用 Rust 1.98.1、MSVC v142 14.29.30133（實際探針 `_MSC_FULL_VER=192930159 x64`）及 Windows SDK 10.0.19041.0；專案 Cargo.lock、vendor、.cargo/config.toml、空 Cargo 快取及 `--frozen`，沒有使用 VS 預設新工具集。

- fmt、Clippy、348 項 Rust 與 68 項 CAD 合成契約通過；固定 CAD runtime 為 `cad-e17fc4a0bf3537c6`。
- WebView2 原生自檢與 composer／追蹤、AppContainer 檔案／網路隔離、副本與來源保護、30份約10MiB LOG、26項原生代理、重試恢復、圖片、長任務／checkpoint、記憶及 PDF 整合通過。
- Python 完整流程通過：Big5／大型 LOG、來源證據、並行批次、程式錯誤修正、CSV／XLSX、隔離、取消、真實120秒逾時及成果遭改寫拒絕。
- `node tools/test_onboarding.cjs`、`node scripts/Test-Settings.cjs` 通過；保留教學、本版更新日誌與窄視窗操作。
- 本輪未選用真實 Office／SMB／VNC 選項，歷史報告不冒充本版驗收。公司服務、建築圖及完整外觀仍需實測。

## 產物

| 產物 | 位元組 | SHA256 |
| --- | ---: | --- |
| `dist/LM_AI.exe` | 19,427,840 | `29b6c6ed141328d8e460f670cfc1cbf7e0e76d65aa7d5d2713930573182ee56e` |
| `dist/LM_AI_Setup.exe` | 36,261,848 | `5f9ddd4a214764bc154d376d98e91a97a34641726fe5a080249fa9beb26c93a3` |

兩者檔案版本均為 `0.8.69.0`；正式 EXE 與完成 2022 Luna GUI 驗證的候選版相同。歷史整套 Rust 離線 ZIP 未重製，本次交付為原始碼、EXE、完整 NSIS 與簽署更新清單。

公司服務、真正建築圖的複雜外參／代理物件、所有 CAD 視覺外觀不在本機合成驗證的通過範圍。原型速度與限制另見 VALIDATION_LUNA_CAD_INDEX.md 的 E 節。
