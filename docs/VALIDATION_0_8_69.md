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

## 公司回報後補測：目前圖面 revision 與 Undo（2026-10-17）

使用者在公司 iCAD 2008 的未儲存 Drawing1（250 圖元）要求中心圓 Ø60→Ø80：第一次被「文件身分或內容已變更」拒絕；重新建立快照後，在 StartUndoMark 前置檢查停止。兩個錯誤須分開處理，不能由這句合併錯誤推定使用者手動改過圖。

本輪使用已發行的鎖定 runtime，不修改產品來源、不建置、不進版、不更新安裝包。僅在新建文件／私有副本重現同一份法蘭需求；合成資料包含原生尺寸、引線、剖面、文字、線型與曲線。公司實際 DWG 與兩次完整原始快照尚未取得，因此不能把本機合成案例當成公司誤報已修復。

| 案例 | iCAD 2008 | CAD 2022 |
| --- | --- | --- |
| 已存檔 645 圖元法蘭：連續三次快照、ZoomExtents 後第四次快照 | 文件身分、全部圖元列與 revision 相同 | 相同 |
| 新建未儲存 250 圖元法蘭：同樣四次核對 | 相同，未重現版本拒絕 | 相同，未重現版本拒絕 |
| 以全新正式 worker 分別讀取同一副本，共三次 | revision 與文件身分相同 | 相同 |
| 目前圖面中心圓 Ø60→Ø80，不儲存 | StartUndoMark 缺少，寫入前停止，半徑仍30 | EDITED_UNSAVED，半徑40；645與250圖元案例均通過 |
| 正式 COM Undo 群組：同一輪改半徑與顏色，結束群組、重繪，再原生 Undo 一次 | 缺少介面，未放行 | 兩個屬性同時還原，通過 |

上表的縮放只是在測試私有副本時主動加入的診斷步驟，不改寫使用者原本「繪圖時不加入 ZOOM」的需求。沒有自動化實際滑鼠移動測試；原始碼的 revision 未納入滑鼠位置或視野倍率。所有本輪測試文件已不儲存關閉，來源 DWG 與私有副本的磁碟雜湊一致；沒有關閉其他使用者文件。

原生 Undo 備援也做了獨立實驗，不能僅以指令送出成功視為可用：2008 以固定 LISP 載入 UNDO Begin 後，未觀察到跨呼叫的活動群組旗標，因而在寫圖前停止；2022 同樣以分次載入 Begin／End 搭配 COM 寫入，再由載入腳本呼叫一次 Undo，實際沒有還原兩個屬性。此備援未通過、未接入產品。2022 正式 COM 群組則另以實際還原驗證通過，不能將前述備援失敗混為正式群組也失敗。

測試助手亦曾發生兩次準備錯誤：第一次傳入錯誤的 2022 Handle，在取物件時停止；第二次把 UNDOCTL 的命令群組旗標誤當成 COM StartUndoMark 的必要條件，在寫圖前停止。修正助手後以半徑、顏色實際還原為準通過，錯誤紀錄保留，不算產品失敗或通過證據。

本機證據（皆不隨 Git／安裝包交付）：

- 2008 已存檔／未儲存：`.build/revision-diagnosis-238fc6ac7077475c8b5a103923175b39/result.json`、`.build/revision-diagnosis-53cebbf0c8cb493cb929cd23f13ea82c/result.json`。
- 2022 已存檔／未儲存：`.build/revision-diagnosis-9ea098b74b0241289b8703c157b708b3/result.json`、`.build/revision-diagnosis-b6026bc24056430fa36ce54bbbd09e24/result.json`。
- 不同程序：`.build/revision-process-4a39f1637eea450485f597cb87eec263/result.json`（2008）、`.build/revision-process-218dd0042b2a496f84517298cac9f582/result.json`（2022）。
- 原生指令備援失敗：`.build/native-undo-diagnosis-5fc363e415544c9b846c8a0c08751757/result.json`（2008）、`.build/native-undo-diagnosis-5fc71c661f514675bc5f283042b316ce/result.json`（2022）。
- 2022 正式 COM 群組實際還原通過：`.build/native-undo-diagnosis-7569466836a04452bfaa70ba0408608d/result.json`。

待處理：版本拒絕目前只顯示合併訊息與整體雜湊，未列身分／單位／圖層鎖／圖元屬性的差異；需先增加可核對的差異診斷，再判斷是否為尺寸計算屬性首次可讀、浮點差異或真正改圖，不應直接放寬內容核對。2008 的作用中圖元修改仍需另外實作並驗證可復原的相容方案；0.8.64 的例外僅限純縮放，先前副本修改成功不代表已支援此路徑。公司紀錄另顯示 INSUNITS=1（英吋），與要求毫米不符；本機新建250圖元使用正式 Drawing 初始化為4（毫米），尚不能解釋公司圖面的單位來源。最後的工具原始結果校驗／記憶未保存是另一條流程，本輪未重現或修復。

## Strata 128K 真實模型／GUI 補測（2026-10-18）

使用者要求先確認調整為128K的本機Strata，連線成功後送先前CAD驗證。這輪測的是**已存在的645圖元合成法蘭板：讀取目前圖面、定位中心圓、Ø60改Ø80且不儲存**；沒有讓Strata重新產生完整法蘭板或第三項控制盒底板。未修改產品來源、編譯、進版、更新安裝包或推送。

- MCP stdio實際呼叫strata_status／strata_chat成功；health回報`qwen3.8-flash-next-iq3_s`、loaded=true、max_context=131072。單次短問答約1.1秒；以獨立請求詢問先前隨機代碼回答不知道，這只是行為抽測，不證明服務所有內部狀態。
- 既有正式0.8.69 GUI → 127.0.0.1:8771測試轉接 → 127.0.0.1:8085 Strata原生Chat Completions。reasoning_effort=none、每輪最多8192輸出tokens。完整轉送CompanyAI提供的messages／tools，不裁切或補入先前會話，不替模型產生工具參數；每輪輸入與原生回覆保留本機。金鑰由使用者既有接口助手讀入記憶體，不複製到CompanyAI設定或紀錄。
- 2008與2022使用不同專案與對話。兩個正式流程的首次請求均為空白專案上下文；模型端收到的只有各輪明確快照。快速模式沒有圖片輸入或analyze_image工具。Strata回報的cached_tokens是同前綴計算重用，不能單憑這個數字推定附帶了未提供的聊天歷史。
- GUI沿用原生修改確認。確認畫面明列本輪私有DWG路徑，測試操作才按「修改此圖面，不儲存」；沒有同意修改使用者其他文件。

| 實測 | iCAD 2008 | CAD 2022 |
| --- | --- | --- |
| 工具流程 | 載入技能、inventory、一次snapshot、一次圓查詢、編輯指南、active_edit、finish | 載入技能、snapshot、圓查詢、兩次指南、active_edit、修改後snapshot／圓查詢、finish |
| 中心圓提交 | Handle162，radius40 | Handle26C，radius40 |
| 原生結果 | prepare_undo_group：StartUndoMark缺少；drawing_started=false、completed_operations=0 | EDITED_UNSAVED、saved=false，僅modified_handles=[26C] |
| 獨立核對 | 半徑30、645圖元列全部相同 | 半徑40、645圖元中只有26C不同；圓心、顏色與其他屬性未變 |
| 模型輪數／模型總耗時 | 8輪／156.207秒 | 9輪／189.945秒 |
| 生成速度（服務原生timings加權） | 27.75 tokens/s | 28.70 tokens/s |
| 最後finish回覆 | 1512 tokens／59.770秒 | 1175 tokens／43.587秒 |

兩個來源DWG與私有副本的磁碟SHA256均保持不變，2022修改只在作用中文件、未存檔。2008與2022的修改前完整核對都沒有發生舊revision拒絕；沒有cad_active_read或0/100全圖分頁循環。2022的第二次snapshot在修改完成後用於讀回，不是回到舊圖反覆讀取。

這不是零錯誤自主流程：2008有一輪只回文字沒有工具，runner要求修正後續行；2022先讀cad-copy指南再讀cad-edits，多一輪非必要閱讀。一般決策多為7–22秒，進度筆記常數百tokens；模型總耗時不含CAD操作、輪詢與人工確認。最大實際prompt分別為14027／13974tokens，不能據此宣稱已驗證128K填滿時的速度或正確性。此處沒有量化版本A/B，不能保證換模型會達70tokens/s。

測試準備問題保留：第一個預設專案位於WebView資料目錄內，pin父目錄遇sharing violation，在推論前停止；另建分離的空白專案後繼續。第一輪轉接原樣回傳Strata的實際model名稱，與桌面fast/quality路由契約不符；只修正測試轉接封套、保留原始模型回覆後重試。該失敗輪不計入正式速度。資料夾選擇UI的操作工具曾失去可用索引，重設後改以已目視確認焦點的鍵盤輸入恢復；這段人工操作不計入模型生成速度。

本機證據根目錄`.build/strata-cad-52b90292a28c4086be28dae907776dea/`：`summary.json`、`turn-*.request.json`、`turn-*.strata-request.json`、`turn-*.response.json`、`2008/independent-verification.json`、`2022/independent-verification.json`及兩個專案_AI_Output內的cad-report.json。正式2008是第2–9輪，2022是第10–18輪；第1輪為轉接準備失敗。EXE SHA256仍為`29b6c6ed141328d8e460f670cfc1cbf7e0e76d65aa7d5d2713930573182ee56e`。

截至本輪，測試GUI與兩份私有CAD文件保留供使用者查看；2008中心圓未改，2022中心圓Ø80未存檔。沒有關閉其他使用者文件，也沒有更換Strata模型或修改其專案設定。

## Strata Q2 比較補測（2026-10-18）

使用者自行換成Q2後要求再試。MCP狀態與真實completion均確認`qwen3.8-flash-next-q2_0`，max_context=131072；仍採none、8192輸出上限、快速模式與相同EXE／runtime。短問答連線約0.98秒。沒有修改Strata設定、產品程式或進版建置發布。

### 相同輸入重播

重送先前Q3的第1、6、8、9、18輪完整messages／tools，分別包含載入技能、查詢、修改參數、2008失敗與2022成功後的決策。保留Q3實際原生回覆與Q2回覆，不執行任何重播產生的工具，也不由Codex修正參數。

| 指標 | Q3 | Q2 |
| --- | ---: | ---: |
| 五份請求總推論時間 | 159.090秒 | 105.363秒 |
| 原生生成timings加權 | 27.94 tokens/s | 44.64 tokens/s |

本批約少34%模型時間、生成約快60%；輸出長度不同，不能把生成倍率直接當成整個任務的加速倍率。Q2載入／查詢／修改工具方向正確，Ø80轉radius40與Handle162正確。2008失敗後Q3用finish，Q2用ask_user列出後續選項；2022成功後兩者均用finish。這些重播不代表Q2已實際操作CAD。

### 真實GUI與CAD

另在兩個空白專案／對話操作正式GUI，分別開啟新的645圖元私有法蘭板副本，來源與Q3相同、INSUNITS=4，不沿用Q3已改Ø80的作用中文件。GUI修改確認只核准所顯示的本輪私有路徑。舊GUI視窗因操控工具回報`failed to activate captured window`無法輸入，重連後仍失敗；另開可見的隔離測試GUI後恢復。準備操作不計入模型速度，也不是模型或CAD失敗。

**2008未通過自主流程**：首次snapshot明確指定`Icad.Application.1`，正確找到Handle162、圓心(110,70)、Radius30；重複載入技能／指南／查詢後，第一次active_edit在prepare_undo_group因`<unknown>.StartUndoMark`停止，drawing_started=false、completed_operations=0。隨後Q2沒有按要求回報結束，而以`version=latest, refresh=true`取得2022的目前圖面，繼續讀指南並準備修改26C。此時已手動按停止，未核准另一版本的寫入。模型第31輪生成的active_edit回覆仍被測試服務記錄，但取消後未交付執行，project2008只有一份原生修改報告。獨立核對2008全部645圖元列不變、半徑30，來源與副本磁碟SHA256不變。

該中止流程共有13份模型生成回覆，生成加權42.81 tokens/s、共126.196秒（包含取消後仍完成的第31輪生成）；這不是成功完成耗時，不能拿來宣稱比Q3的完整失敗回報更快。沒有繞過2008 Undo限制，也沒有把改別套CAD當成2008通過。

**2022幾何修改通過，回報有數值錯誤**：另一個獨立空白專案中，Q2多次查圓、矩形與polyline，包含重複查相同候選與兩次compact_context，最後正確提交26C radius40。原生回報EDITED_UNSAVED、completed_operations=1、modified_handles=[26C]、saved=false；再重建快照與details查詢確認Ø80。獨立逐圖元比對645列只有26C的半徑／對應native_revision改變，圓心／顏色等不變，來源與副本磁碟SHA256不變。最後答案卻稱「其他5個圓（含直徑15的孔等）」；實際是4個Ø12孔和1個Ø20圓，故文字正確性不能全算通過。

| 2022完整流程 | Q3 | Q2 |
| --- | ---: | ---: |
| 模型輪數 | 9 | 18 |
| cad_query次數 | 2 | 10 |
| 模型總推論時間 | 189.945秒 | 199.790秒 |
| 原生生成加權 | 28.70 tokens/s | 43.38 tokens/s |

Q2本輪生成較快，但多輪查詢抵消收益，完整模型時間約多5%；CAD／輪詢／人工確認不包含在上表。未出現cad_active_read或0/100全圖分頁；2022第二次snapshot是修改後讀回。兩個新流程均無圖片輸入或analyze_image工具，最大輸入分別13343／14122tokens。這是少量實測，不是128K填滿、建築圖、統計性量化品質或公司服務效能驗收；流程差異不能單獨證明必由量化造成。Q2可作較弱模型的流程壓力測試來源，但不能僅由tokens/s認定更適合穩定驗收。

本機證據根目錄`.build/strata-q2-587c4789d7b34db08b7f4366b21d1f7a/`：`health.json`、五份`replay-*.json`、`metadata.json`、`observations.json`、`summary.json`與兩版本`independent-verification.json`。GUI請求與原生completion沿用先前Strata紀錄根目錄，第19–31輪為中止的2008，第32–49輪為獨立2022；原Q3第1–18輪及summary未覆寫。測試GUI與私有文件保留，2008未改，2022Ø80未存檔；沒有關閉其他使用者文件或發布。

## 產物

| 產物 | 位元組 | SHA256 |
| --- | ---: | --- |
| `dist/LM_AI.exe` | 19,427,840 | `29b6c6ed141328d8e460f670cfc1cbf7e0e76d65aa7d5d2713930573182ee56e` |
| `dist/LM_AI_Setup.exe` | 36,261,848 | `5f9ddd4a214764bc154d376d98e91a97a34641726fe5a080249fa9beb26c93a3` |

兩者檔案版本均為 `0.8.69.0`；正式 EXE 與完成 2022 Luna GUI 驗證的候選版相同。歷史整套 Rust 離線 ZIP 未重製，本次交付為原始碼、EXE、完整 NSIS 與簽署更新清單。

公司服務、真正建築圖的複雜外參／代理物件、所有 CAD 視覺外觀不在本機合成驗證的通過範圍。原型速度與限制另見 VALIDATION_LUNA_CAD_INDEX.md 的 E 節。
