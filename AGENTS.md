## 目前任務：0.8.72 Mermaid、CAD擴充與等待確認通知（2026-10-19）

- 使用者明確要求：Mermaid做在專案功能；原始碼複製、PNG/SVG/MMD保存、下輪讀原碼改圖；可以加入實用相關功能／套件。已同意沿用上版進版、基本／針對性驗證、NSIS與Git main推送。後續又要求合併建議的CAD擴充，並使用已開啟的本機Strata按需求實測，通過才發布；完整長流程／安裝生命週期仍不擅自擴大。
- CAD本輪包含原生polyline/arc、半徑／小角尺寸、新圖層與圖元改層、有界原生圖塊、複製／旋轉／鏡射／矩形陣列、線／圓偏移及兩直線修剪／延伸／圓角。固定JSON與原生LISP模板，不開任意指令；既有2D修改限WCS Z=0、已讀主副目標、無TrueColor／反應器／擴充字典，依allowed_edits；新幾何與改字／半徑批次分開，2008通用新增／刪除仍不開放。新增指南cad-advanced。
- 新增等待用戶操作的背景Windows托盤通知／工作列提示，點擊返回專案對話；相同request只提醒一次，前景不打擾，不代替同意，遵守通知設定。涵蓋CAD確認／檔案等待／Outlook範圍／圖表選擇與偏好問題。
- Mermaid 12.2.0官方standalone已SHA512核對，固定離線dagre/strict並淨化SVG；Create/Inspect/ExportMermaid走真正WebView2 awaitPromise、來源版本與加密checkpoint。實際Strata三輪產生3圖／3 SVG／2 PNG，原碼及based_on、原件未變／舊版保留核對，兩PNG已目視。模型main.py行號引用不準，語意品質未核准。
- 正式Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller於14:09:52退出0，RELEASE_0_8_72_BASIC_AND_NSIS_PASS。v142 14.29.30133、cl192930159 x64／link14.29，空快取frozen、fmt、Clippy、相關Rust篩選、99 CAD契約、原生UI與來源整合、NSIS及簽署清單核對完成；完整長流程／安裝生命週期／公司服務未重驗。
- 最後runtime SHA256為4ff7453eb569f48a547b9b007389fee66a02548a35f7b825e1191d596fc0f6fc；EXE為4dd36ed85e86266d66070581fc4129665a4b7bec3e5e4a50b419429b41c5243f，Setup為4d83efc1626fbafb9a670c8975417c65d3db1871eb4319c83e057d129823da6a。finalize072.py已退出0並更新公開摘要／文件；Git交付以提交紀錄及.build/remote-delivery-072.json為準。排除.codex/、dist/onboarding-state.json、icon.png、.build/與私人連線／聊天。
- 最後雙版原生新增重開根native072-new-4ef80e0a39034bb6bc6f817522b2f9e3，修改及一次Undo根native072-edit-afb6e9aabeac4e3f94bd1b30930a5be8，native072-final.json對應最後runtime。Strata實測找到並修正2008圖塊子圖元HandleToObject、順時針角度端點交換、圖塊名稱大寫讀回的相容性；皆有固定回歸，失敗未存檔圖與診斷保留。
- 最後雙版Strata CAD根f974656eb457460dbbe8d4be7fdb57cd已完成並獨立原生讀回，結果忠實符合原始模型參數、DWG雜湊未變。兩模型把半徑標註點取近似值，R20意圖核對失敗：2008為19.996979771955555、2022為19.9998081990803。嚴格失敗保留，未放寬容差改判；公開結果PASS_BASIC_FOCUSED_FUNCTIONS_WITH_MODEL_CONTENT_ISSUES，不是設計通過。2022 INSERT容器0層，內部孔／中心線在指定層。CAD未做真窗口目視。
- Mermaid證據另在9c64b7343c4044d1af5871fe20e52b36；255份Rust／UI／指南／Cargo／build來源核對未變，最後僅改CAD Python／runtime，雙版CAD以最後EXE測。公開摘要記不同EXE雜湊，不混稱同一二進位。正式App localhost隔離入口已實測，必要確認不代答，原始模型參數未改寫，Strata金鑰不落公開文件。
- 使用者明確要求：先推送0.8.72及docs/COMPANY_TEST_PROMPTS_0_8_72.txt七組提示詞，再實作docs/NEXT_VERSION_NOTES.txt的1–7項（modal、快速模型確認、資料夾捷徑、VNC JSON、縮放、四字體、未知副檔名清單與文字副本編輯）。後續修改暫不進版、不編譯、不重新打包或擅自二次推送，等公司測試。字體預設霞鶩文楷繁中非等寬，三本機開源字型已與官方固定tag blob核對一致並取得授權，待推送後才加入來源。0.8.72交付前不混入後續功能。

## 歷史：0.8.71 基本建置與NSIS完成（2026-10-19）

- 使用者明確要求本輪實作：ICAD2008 文字／直徑標註更新、程式解析並回傳確切 CAD 版本、輸出額度耗盡後最多自動續接 5 次。完成後進版、v142 x64 編譯、NSIS 打包並推送既有 GitHub main（來源、EXE／Setup／更新清單及文件）；本輪僅小部分／針對性驗證，不跑完整驗證。
- 延續既有排除清單與共享目錄界線。不得重播已成功 CAD 工具、執行截斷參數、擅自改用其他 CAD、修改使用者目前圖面進行實驗；實機使用私有新圖驗證修改及 Undo，保留既有圖面。
- 原生定向驗證已通過：最後 runtime SHA256 `7c5c36c5f2d67c09dd8dcffc19318e038236897ee384e95be7508dddff4adba1`，`.build/native-071-313b060c556549e6a9ae5d27e6efe673/result.json` 為 PASS；文字／引線附屬文字原位更新，直徑尺寸同圖安全替換並回傳新Handle，一次Undo全圖原生revision完全還原，磁碟未存檔。直接改端點的早期候選Undo後DXF42快取未還原，失敗保留，不接受為通過。
- 25項progress、6項cad_plan、Broker還原、88項CAD及技能長度回歸通過。新版指南曾超5000bytes，已精簡，沒有放寬上限。`.build/release071.ps1`正式基本流程10:25:30退出0（`.build/release-071-exit.json`），`.build/release-071-r2.log`有`RELEASE_0_8_71_BASIC_AND_NSIS_PASS`。實際v142 14.29.30133／cl192930159 x64／link14.29，空快取frozen、release、基本契約／來源整合與NSIS封裝簽署驗證通過；完整及NSIS安裝生命週期未重驗。首次`.build/release-071.log`為指南長度失敗歷史。
- `.build/finalize071.py`已退出0，核對PASS_BASIC、0.8.71.0、兩份NSIS清單及最後原生proof，更新README／驗證文件與`offline/cad-annotation-verification-0.8.71.json`。EXE SHA256 `84a38d7d1509f7d734cc64ea5a5faa35f1f73a7926ff490e37b86bcc05ffbbfa`，Setup SHA256 `c5d8de1ca495329968e441d9eb753960cd79b8b8bbea7710a58ce10093f6f648`。Git交付白名單沿用既有main，遠端提交以Git紀錄為準；排除個人檔／.codex／.build／icon.png，不更新歷史ZIP，不沿用0.8.70的完整PASS。

## 歷史：0.8.70 正式建置與恢復核對完成（2026-10-19 09:00）

- 使用者要求查看斷線前進度並繼續。原 pwsh23376 仍在執行，沒有重啟建置；08:59:37 完成完整 Build.ps1 -EmptyCargoCache -IncludeInstaller，日誌 `.build/cad-segment-release.log` 已有 `CAD_SEGMENT_RELEASE_BUILD_AND_INSTALLER_VALIDATED`。
- 實際 v142 14.29.30133、cl `_MSC_FULL_VER=192930159` x64、link14.29.30159；377 Rust／87 CAD、專案與 Python 隔離長流程、完整 NSIS 安裝／更新／回復／修復／移除全部通過。`offline/exe-verification.json` 與 `offline/installer-verification.json` 均為 0.8.70.0 PASS。空 Cargo 快取只有 registry/CACHEDIR.TAG，124 份 vendor 套件沿用，Cargo.lock 只有本專案版本改動。
- 原建置程序退出碼無法重新取得（觀察器得到 null，不當成 0）。恢復後 `.build/finalize_segment_release.py` 獨立核對報告、雜湊及兩份更新清單，實際退出 0；`.build/resumed-release-artifact-verification.json` 保存結果。README、docs 索引及驗證文件已更新 0.8.70。
- EXE SHA256 `75a11583da9d8643eccfebc9ce0669f432fc66af3cb312ce72aaea2195186f4a`；Setup SHA256 `458e6f3956757b846374cc94ab52cdedf55de8aa00e847e413258bf6aaed8525`。雙版 ZIP 仍為 43879 bytes／SHA256 `5ee41c6bfaa83ba823fff19289e29916e42f557447228d7007203725ed0c732c`。
- Git 交付採白名單提交與既有 main，並核對遠端 OID／LFS；實際提交以 Git 紀錄為準。排除 `.codex/`、`dist/onboarding-state.json`、`icon.png`、`.build/`；歷史完整離線 ZIP 不重製。兩份住宅的設計品質仍 ISSUES，不能改稱設計通過。

## 歷史：雙版已完成，正式發行執行中（2026-10-19 08:34）

- 使用者新增授權：測試後將2008／2022 DWG包一個ZIP推Git；已明示本輪分段最後雙版樣本，放examples/cad。.build/package_cad_examples.py已成功；xamples/cad/cad-segmented-0.8.70.zip為43879bytes，SHA5ee41c6bfaa83ba823fff19289e29916e42f557447228d7007203725ed0c732c，含2008/兩層樓住宅平面圖.dwg、2022/兩層樓概念平面圖.dwg、README及公開manifest；CRC／逐位元核對通過，無聊天／Token／絕對路徑。旁邊README也要commit。ZIP小檔可直接Git，不需新增LFS規則。
- 最終details838候選**雙版完整結束，controller22009退出0，progress complete2**。2008九段160操作160圖元，27輪1615.936模型秒／67579輸出／峰12183；2022七段116操作116圖元，12輪630.439秒／24256輸出／峰6097。均0 length、同一DWG原生存檔重開／獨立資料及真窗口覆核，兩案都超100原生入口，不跑固定120fixture。
- 新.build/check_segment_feature.py已完整PASS_STAGED_PROTOCOL，根內feature-proof.json可用；summary已重跑。接受／拒絕以實際tool_call_id結果區分，200821項超量失敗後自行拆段；首次commit因CP950建圖前拒絕，只修兩段後第二次成功。2022一次commit，無inspect。
- **兩案設計仍ISSUES**。2008缺尺寸／門線未斷／500mm廚門／衣柜?，2022缺尺寸／封閉房間無門窗／楼梯顯示?梯／陽台實際上側但答覆說下側；詳各review及重寫後docs/VALIDATION_0_8_70.md。2022沒有ask_user，角色的8×12與一樓廁所補充未送，不能視為已知要求；自行假設12×10。DWG SHA2008 444a1f02776b16c6e381f3672cf87d7daab24f4d343c00d1b21bff0670324b24，2022 5b52ac74eaabaae6e486ff67083c25a8847a3b4ecfdcf9920d687c350514bfcd。真窗口調視野後SHA均未變，文件留開。
- **正式發行在跑exec43511**：.build/release_cad_segment.ps1→完整Build.ps1 -EmptyCargoCache -IncludeInstaller，log .build/cad-segment-release.log。實際v142 cl192930159/link14.29、377workspace及87CAD已過，正往Python／project長流程；尚未到NSIS，不能稱發行完成。必須exit0及CAD_SEGMENT_RELEASE_BUILD_AND_INSTALLER_VALIDATED。後續README/current版本0.8.70、docs索引／驗證最終buildhashes、同版dist／offline報告、AGENTS收尾、白名單git add（含CAD ZIP）、commit/push main並驗remote/LFS。
- Git最後查main仍f43d9fcaffc33d214abf5328ec11c814a886b736，LFS3.5.1。排除.codex、dist/onboarding-state.json、icon.png及.build。無Git提交／推送完成，README仍0.8.69。
- 使用者公司輸出已重設20000。已解釋分段兩例零length不保證永不超限；caps可看default_max_completion_tokens／limits.max_completion_tokens（公告未必實效），DEBUG原始finish_reason／用量可判斷。空length目前保存進度暫停，不保證自動2–3重試成功。不新增公司API測試或中途改凍結來源。

## 即時進度：最終候選雙版回歸（優先於下方歷史）

- 最新使用者詢問進度，已回答約70%、剩45–90分鐘估算，**仍繼續原任務，未暫停**。不要結束在進度回覆；必須完成正式發行與Git推送。
- 最終details838根2008已完整：27模型輪1615.936秒，9段160項→160圖元，一份DWG存檔重開；同段替換成功，超20拒絕後拆段，commit先CP950拒絕且未建圖、只修兩段再commit成功。0 length。真窗口全圖／近看已完成，visual-review ISSUES＋staged_protocol_pass true；缺尺寸、大門原線未斷、廚房僅500mm開口、衣柜顯示問號，不能稱設計PASS。SHA444a1f02776b16c6e381f3672cf87d7daab24f4d343c00d1b21bff0670324b24，圖保留未改／補存。
- 同controller22009正跑2022，8793，產品來源仍凍結，segment-current不變。t028 load_skill已完成、029推論中。等待await_visual後獨立資料＋真窗口覆核；用inspect_segment_progress.py。
- .build/check_segment_feature.py已補按真tool_call_id的實際ok結果區分接受與拒絕，僅接受段才計入總數，另保存rejected_part_attempts；不可把有效超限復原樣本判為全部參數接受或擅自刪除原模型錯誤。兩案完成後正式執行proof。
- 2008已真模型走超100原生入口，不需額外固定120操作fixture。兩案完成→summary／feature-proof→release_cad_segment.ps1（完整v142空快取frozen＋NSIS安裝生命週期）→同版README／文件／Git LFS commit push main及遠端核對。尚未改dist、尚未推送。

## 最新任務：0.8.70分段計畫與發行（2026-10-19，優先於下方歷史）

- **07:43最優先現況：新版已開始真模型重測，不能停工**。最終wire候選完整通過，exec30576退出0＋CAD_SEGMENT_CANDIDATE_VALIDATED；`.build/cad-segment-details-wire-build.log`為377 Rust／87 CAD／2角色／15harness及v142 cl192930159 x64／link14.29、空快取frozen。runtime仍完整SHA b9c37d239820b4f1924457c04d7e4551693f91045bc9432e9c55041122492c33；新model EXE SHA `c6542823c7e95d1fb32763ed58502f04f088087f3b0c336128598bfde5b890c8`，built.json已更新確切來源hash。
- **目前controller exec22009**，`.build/cad-segment-details-model-controller.log`；segment-current→`.build/cad-user-segment-details-8381802ad695433dbba43d443e536d91`／8793／Q2 low20k128K，住宅2008→2022。剛t001推論，尚無CAD產物。用`.build/inspect_segment_progress.py`看最新4輪／pending，進await_visual或await_failure_review必須獨立資料＋真CAD窗口覆核。此新根與舊b6中斷根分開保存；產品來源現在凍結，避免再中途修改。
- 最近4段32KB精確工作集／inspect生命週期已整合；另Part.model_value還原預設line_style:null供模型查回，與公開schema一致，**原生計畫仍省略可選null**，不改座標／線型。這是針對舊案例照抄有效區段卻缺line_style的錯誤，非測試程式代改模型參數。新377測試已含wire斷言。
- 後續依下方準備腳本：雙版逐案覆核→summarize及check_segment_feature產生feature-proof→必要120操作native邊界（僅若兩案未超100；腳本guard要求complete2，不是模型測試）→正式release_cad_segment.ps1執行完整Build -EmptyCargoCache -IncludeInstaller→同版README／CHANGELOG／驗證文件→Git LFS commit/push main驗遠端。**使用者已授權完成後發行推送，不能只回報部分實測就結束。** 尚未更新dist或推送，README仍0.8.69，CHANGELOG已加0.8.70來源功能。

- **07:35最新，不能發行舊候選**：第一候選2008在75項後反覆inspect floor2_walls／floor2_furniture_a；實際每輪只保留最近一個tool body，缺另一段精確資料，與原0/100問題同類。已主動取消turn041，再核對PID後停止runner14024／controller16304，session38336退出1為除錯中斷；2008 **0 cad-report／0新DWG**，CAD程序及原圖未動。舊根b6c0ac42… progress=stopped_for_plan_detail_retention_fix，interruption.json保存，041晚到仍inspect、取消後未執行。2022先前真窗口ISSUES不覆蓋，不稱完整雙版通過。
- **修正已實作**：Plan.recent存ID，最近新增或查回4段完整精確資料以recent_details常駐，合計32KB，超額明列recent_details_omitted，原計畫不刪。替換後投影取新座標；checkpoint及compact_context不清工作集。全部所需段已完整常駐時不公告cad_plan_inspect；有舊段未列才公告，part_id enum只列缺少的ID及null，避免已知段互查。新增2項回歸＋既有Broker還原測試補斷言，5項cad_plan定向已過；workspace新版預計377。guide/catalog已同步。
- **正在新版完整候選建置**：exec92237，`.build/cad-segment-details-build.log`。v142 release及87 CAD已過，仍待原生自檢／workspace／example收尾，只有exit0＋CAD_SEGMENT_CANDIDATE_VALIDATED才能算完成。runtime仍b9c37d239820b4f1…（Python未改），新版EXE hash以完成後`.build/cad-segment-built.json`為準，不能拿舊11d394…當新版。
- 下一步執行`.build/start_cad_segment_details_model.ps1`（已準備**尚未執行**），8793／新`cad-user-segment-details-*`根、更新segment-current；先住宅2008再2022。腳本要求舊根已停止且所有原推論回覆已落盤（041已落盤）。真模型不能代填參數，每案獨立資料與真CAD窗口覆核後寫review；尤其檢查不再牆線家具互查、每段≤20、無length、同DWG commit。模型品質問題仍分開標。
- 已準備`.build/check_segment_feature.py`：完成2案後核對真模型分段／常駐數量／單次DWG／原生完成／磁碟SHA／真窗口review，產生本根feature-proof.json。`.build/run_staged_native_limit.py`可在雙案完成後做私有2008固定120操作原生邊界（不是模型測試），若兩案仍未超過100可補測，執行前guard要求progress complete2；結果指標`.build/cad-segment-native-limit-current.txt`，有失敗先保留不重畫。
- **必須繼續做到發行推送**：`.build/release_cad_segment.ps1`已準備，要求雙版feature-proof後執行正式Build.ps1 -EmptyCargoCache -IncludeInstaller（完整workspace／專案Python隔離／NSIS生命週期，無Office／SMB選項），確認0.8.70.0及CAD_SEGMENT_RELEASE_BUILD_AND_INSTALLER_VALIDATED。尚未執行。然後README／CHANGELOG／0.8.70驗證文件／AGENTS更新同版，檢查私有資料、Git LFS後commit/push main，驗遠端OID。Git遠端仍f43d9fc，未改dist／未推。user已明確授權，不再問批准、不停在候選實測。

- **07:07實測進度**：同controller38336／8792／segment根b6c0ac42…，2022住宅已完整資料＋真窗口覆核、visual-review=ISSUES，將進2008。產品來源仍凍結。2022確實5段12/12/10/16/8，常駐計畫累積12/24/34/50/58，模型另inspect floor2_walls取精確座標，單次commit無operations；58操作→78圖元同一DWG存檔重開核對通過。14模型輪539.444秒／24430輸出tokens，**0 length**；最大單輪6845（額外compact），實際part最大3566。原2022住宅3×20k失敗，本次分段傳輸功能確實走通，但不可當設計通過。
- 2022設計品質問題是真實：floor2牆y14000–26000，floor2_labels八文字仍y2500–12500，模型漏+14000而標到一樓；無原生尺寸，independent DATA_ISSUES非Oracle誤判；樓梯只有區域／文字、門口動線未驗。地面家具是5個hatch矩形占位，不能說完全無圖；二樓床/衛浴用矩形及圓。真窗口已全圖與局部看完，中文可見但小字簡繁混用。DWG SHA bf2e181159addd2551a0602cf0cd22331db5f53a1a4947ef722f928e7ea03161，仍開不補存／關閉。
- 目前功能修復目標是同圖分段與避免單輪額度，**不是宣稱全面改善CAD設計品質**。後續2008仍須跑完原生／獨立資料／真窗口，結果記錄後正式v142 Build -EmptyCargoCache -IncludeInstaller與同版文件、Git推送；不要停在此樣本。不把本輪已觀察的模型錯座標說成程式漏存資料。常駐目錄不額外複製幾何，但最近工具歷史仍含當輪參數；先前已向用戶精確補充。

- **06:55進度**：最終候選完整通過，exec84304退出0＋CAD_SEGMENT_CANDIDATE_VALIDATED；375 Rust／87 CAD／2角色／15harness，實際v142 cl192930159 x64／link14.29，空快取frozen。runtime完整SHA `b9c37d239820b4f1924457c04d7e4551693f91045bc9432e9c55041122492c33`；model EXE SHA `11d3940702372a3b67979c3641b2ff041970e7b1121a0561964a6596ae8382fe`。built.json記確切來源hash。
- **真Strata分段實測已啟動，未暫停**：controller exec38336，`.build/cad-segment-model-controller.log`；`.build/cad-user-segment-current.txt`→`.build/cad-user-segment-b6c0ac42f26a4f1fbc15b133bc3bd17a`，8792／Q2 low20k128K。先two_floor_house-2022，再2008；目前2022 t001 load_skill10.773秒，t002推論中。監看`.build/inspect_segment_progress.py`。產品來源凍結，勿中途修改；必要原生問題才另候選。
- 每案必須等cases/progress的await_visual或await_failure_review，獨立資料＋**實際CAD窗口**覆核後寫visual-review／failure-review，不用資料成功代替外觀。核對模型確實cad_plan_start→多段part≤20→commit只送ID版本→一份DWG，記length輪數及模型時間。`tools/summarize_cad_user_cases.py`已增staged_parts／plan_commits／output_limit_rounds／staged_limits_observed，完成後跑本根總結，不能拿之前原14／浴室2案當本版模型通過。

- **06:54即時狀態**：最終候選建置在exec84304／`.build/cad-segment-build-final.log`，v142 cl192930159／實際link14.29、87 CAD已通過，正在workspace／example收尾，必須exit0＋CAD_SEGMENT_CANDIDATE_VALIDATED。新runtime `b9c37d239820b4f1…`。第一完整候選曾通過374 Rust／87 CAD／2角色／15harness，但其後加入commit成功結果有界摘要與1項新測試，不能拿374當最新版完整結果。最新預計375。原生完整報告仍存本機、模型只收交付事實／量測不可用計數／最多8條400字警告，避免400操作成果填滿128K。
- 完成後執行`.build/start_cad_segment_model.ps1`（已準備，**尚未開始**），8792／新的segment-current／Q2 low20k128K，先two_floor_house-2022→2008。腳本核對built.json所有來源hash／EXE／runtime再凍結；監控`.build/inspect_segment_progress.py`。原主current及targeted-current不覆蓋。本機Strata仍可用，舊模型均完成。
- 最新2個既有缺漏已修：新增4工具讓舊文字目錄破64KB，改同一份幾何規格文字JSON Pointer參照（原生完整Schema不改），並測試不對無operations的工具憑空加欄；工具樣本加4、引線契約份數改4。第一次build log `cad-segment-build.log`因此舊3份斷言失敗；rerun完整通過，final是摘要補強後重驗。
- 發行仍未開始。C:/largan/LM_AI與LM_AI_Installer_Test都不存在，標準Test-Installer可用；Git main遠端仍f43d9fc、LFS3.5.1可用，未提交或推送。README／CHANGELOG待模型結果後更新。`docs/VALIDATION_0_8_70.md`已建尚待實測結果；來源說明已補CAD_DRAWING／WEB_INTEGRATION。最終必須Build.ps1 -EmptyCargoCache -IncludeInstaller成功、更新版本與驗證文件、提交白名單來源及同版產物並推送，別停在候選測試。

- 使用者最新明確要求：先實作同圖分段累積，真Strata測是否能逐段交工具避免單輪20k，完成後**進版、編譯、NSIS打包及推送既有Git main**，供公司實測。此條取代本輪下方不發行限制；不重製歷史整套離線ZIP。
- 已新增本機cad_plan_start／part／inspect／commit，fresh cad-new短方案後公告分段入口，不公告需一次送整圖的cad_draw。每段1–20、總400項／40段／500KB；checkpoint保存精確資料，每輪只帶目錄。相同part_id按最新plan_revision整段替換、過期拒絕、完全相同重送不追加。commit才合併同一張DWG，native整份預驗後繪製，既有CAD不修改。舊直接cad_draw維持100，原生staged_plan固定400、4000圖元與600秒仍有界。版本已改0.8.70，尚未發行。
- 正在v142編譯單元／Clippy：exec9687，`.build/cad-segment-unit.log`、`cad-segment-clippy.log`。初版2項新測試抓到Broker原始Schema oneOf未轉anyOf而漏查必填欄位，已修成只轉branch別名保留內部optional語意，2項定向重跑通過；新增Broker checkpoint／工具可見／取消未繪圖測試待本次完整結果。不可稱整體通過。
- 下一步完成新runtime、v142空快取frozen驗證及release model example，再以**新隔離根、端口**跑原兩層住宅模糊需求（先2022以前撞20k，再2008）。保留資料、模型原始往返與真CAD畫面覆核；檢查每段≤20及commit無operations且一份DWG。必要時據失敗修正再新候選，不替模型補參數。
- 模型實測後正式Build.ps1 -EmptyCargoCache -IncludeInstaller及必要安裝生命週期驗證，更新README／0.8.70驗證記錄與使用者公告；Git提交所有本輪應用程式來源／同版產物與文件，排除.build、.codex、icon.png及dist/onboarding-state.json等個人檔。不納入其他專案或改遠端／強推。測試紀錄不放應用教學。

## 即時交接（2026-10-19 06:20，優先於下方歷史）

- **本輪矩陣及修正回歸收尾完成**：原14案（11 VISUAL_ISSUES、2 CAD_FAILED、1 PAUSED_OUTPUT_LIMIT）保持原結果；V2原生兩修正與真窗口回歸已通過；新Strata浴室雙版代表回歸亦逐案存檔重開、獨立資料及真窗口覆核完成。controller56255自然退出0，targeted progress complete／cases2；沒有任何執行中CAD寫入或待模型回覆。
- 新候選 `.build/cad-user-native-fixes-targeted-f4c61e145380488ebacecee76f99436a`／8791／targeted-current，runtime c389…、EXE9b76…；2008 5輪262.956秒／10880 tokens／28圖元，2022 6輪360.040秒／12798 tokens／29圖元。兩版第一次finish []、無格式重送或length，總深200／180正確。**兩案仍為VISUAL_ISSUES**：尺寸細小暗藍、支架／固定點／圓角細節與承載說明不足。2022中文標題可辨、小字密集，不能說全缺字；2008全英文不算中文驗證。兩份DWG視野調整後磁碟SHA未變，post-visual-file-check.json保存；Get-FileHash曾共享拒絕，Python唯讀檢核成功，未解除占用。
- docs/VALIDATION_CAD_USER_SCENARIOS.md、docs/PENDING_CAD_Q2_OPTIMIZATION.md及新候選model-summary.json已記完整收尾。完整建置為v142 x64／空快取frozen基本整合／371 Rust、86 CAD、2角色、15驗證流程及release example通過（V2 log及built.json）。沒有進版、dist、安裝包、Git提交／推送。
- 下一輪待辦僅規劃：標註比例、同圖結構化分段計畫、局部參數修正、幾何語意檢核。公司首次revision誤報仍未以原圖重現；此次新拒絕診斷只有契約測試，本次模型未觸發拒絕，不稱模型自我修正已測過。全應用其他功能的完整驗證仍未執行。原未存檔Drawing11及各ISSUES圖保留開啟，別補存／關閉。

## 即時交接（2026-10-19 06:13，優先於下方歷史）

- V2原生回歸與真窗口檢查均完成：新proof `cad-native-read-fixes-proof-efcaf77088f840138c805903fa2b31e8`，2008新私有住宅86圖元存檔重開且全圖視野正常，原Drawing11全圖未變；2022原花架30圖元268屬性一致／Unicode檔名與SHA未變。visual-review只接受原生修正，原14案不改判。
- **新Strata代表回歸正在跑，未暫停**：exec56255，8791，`.build/cad-user-targeted-current.txt`→`.build/cad-user-native-fixes-targeted-f4c61e145380488ebacecee76f99436a`，凍結V2 runtime c389…、EXE9b76…，low20k128K。主current仍原14。controller log `.build/cad-native-fixes-targeted-controller.log`。
- 2008浴室案已真窗口最大化覆核並寫ISSUES，開始2022：5模型輪262.956秒／10880輸出tokens、28操作28圖元（18線4尺寸6英文文字）存檔重開及獨立資料PASS，SHA9e283d0b488e3f191991aa6784b5d6950d00999a1c1167b7dafe721df1e7c0b5。總深800–1000正確200、淨距280；finish第一次[]成功，沒有格式重試或length。尺寸字很小且暗藍、支架／R3僅概念、20mm不鏽鋼假設待確認，答覆仍冗長且未量瓶高就稱足夠。圖留開未補存／關閉。不能稱全案PASS。
- 下一步：等2022模型／原生存檔重開完成，依targeted cases/progress關卡做獨立資料與**實際CAD窗口**覆核後寫review，讓controller自然結束。最後整理本回歸model-summary與docs；不進版／dist／打包／Git。其他源碼不要在凍結候選中途改。

## 即時交接（2026-10-19 06:05，優先於下方歷史）

- 使用者要求繼續，未暫停。原14案已完整覆核（11視覺問題、2原生失敗、1輸出上限，沒有全案PASS），不得改判原失敗。
- 新來源完整v142離線建置已成功：`.build/cad-native-read-fixes-build-v2.log`、exec32744退出0、CAD_NATIVE_READ_FIXES_VALIDATION_COMPLETE；實際cl192930159 x64／link14.29、空快取frozen基本整合、371 Rust／86 CAD／2角色／15驗證流程測試及release example通過。`.build/cad-native-read-fixes-built.json`保存來源與EXE雜湊，runtime `c389e34d901fc0134269dc13c2dd64062dd1e29784ec4a27b36b21c28963b5c8`。
- V1原生回歸雖存檔重開86圖元，但真窗口揭露視野0.1錯誤，不能算修復：新建2008圖元未Regen時GetBoundingBox全零。私有兩線探針證明Regen後座標正確且資料不變。V2只在罕見縮放備援先Regen，再取全圖原生範圍；拒絕合併後仍為零的XY範圍，一般批次讀圖不受影響。
- V2原生回歸執行中：exec75868，`.build/cad-native-read-fixes-proof-v2.log`，指標`.build/cad-native-read-fixes-proof-current.txt`目前指向`cad-native-read-fixes-proof-efcaf77088f840138c805903fa2b31e8`。原Drawing11完整86圖元先存唯讀快照，僅在新私有DWG重播原87操作；接著2022讀原花架Unicode檔名。完成後必須實際看兩版窗口，寫新proof的visual-review，不能沿用V1錯視野覆核。
- 之後執行`.build/start_native_fixes_model_test.ps1`，8791／獨立targeted-current，新凍結EXE與runtime做浴室架2008／2022真Strata代表回歸，維持原模糊用戶需求及真窗口關卡。尚未開始，不能稱新版模型通過。不進版、dist、打包或Git。

## 即時交接（2026-10-19 05:42，優先於下方歷史）

- **原凍結候選14／14已完整覆核完成**，current仍`.build/cad-user-explicit-version-99d651b60a2c404ea6317d873c5c59eb`／8790／Q2 low20k128K／runtime7018…。controller20035自然退出0，progress complete/cases14。11 VISUAL_ISSUES、2 CAD_FAILED、1 PAUSED_OUTPUT_LIMIT，沒有PASS。94模型輪9824.672秒／409793輸出tokens／峰值輸入41963／無圖片，不代表128K填滿。model-summary.json與docs/VALIDATION_CAD_USER_SCENARIOS.md最新完整收尾。使用者仍要求繼續修正驗證，工作未完成，不暫停。
- 最後浴室2022已真窗口局部／全圖看完：58圖元6尺寸、native80秒存檔重開，7輪459.911秒。總深200含支撐正確，但圖註淨高250/200與實際下230、最後答覆230不一致，字小簡繁混用、工程細節待核實，ISSUES；SHA3cd5cdc548dcdc708f341496554302fe0aa7fbcb04d1bf33143aafe14ae5cedd。**operations[48]原錯誤是leader漏height，不是ZOOM E；ZOOM E是合法別名，已向用戶更正**。前13案詳下方。
- **新來源改動**：cad_bulk.py三個helper分開核對DWGNAME/DWGPREFIX，接受COM原值的ANSI LISP BMP表示法，不一般解碼；cad_worker.py全原生GetBoundingBox範圍備援；cad-new.md補失敗保留／幾何占位誠實／總外廓含支撐墊片／finish.artifacts=[]。指南原4145加common超合併5000檢查，現縮2918bytes保留規則，未放寬門檻。85 Python契約曾通過（縮指南後仍需完整建置再跑）。
- 另新增src/projects/agent/schema.rs拒絕後診斷：已明示kind/action且各候選單一常數、唯一命中才深入具體錯欄位；接受條件matches不改，未知／模糊保持泛用；新增2 Rust回歸，workspace預計371。**9項schema定向已以v142編譯通過**，含leader缺height、command多餘color、合法ZOOM E保留、錯線型及未知／模糊tag。第一版新測試誤假設ZOOM E不合法，第二次fullbuild因此失敗，已更正；不是產品接受條件被改寬。
- **現執行建置：exec session69110**，.build/build_cad_native_read_fixes.ps1重新完整跑，log `.build/cad-native-read-fixes-build-final.log`。前兩次失敗log `.build/cad-native-read-fixes-build.log`（指南合併大小）、`...-rerun.log`（新測試假設錯）。目前仍未回報建置完成；必須exit0＋CAD_NATIVE_READ_FIXES_VALIDATION_COMPLETE。流程含實際v142 cl192930159/x64、link14.29.30159、空快取frozen Basic、全部workspace、example2角色、harness15、release example；不進版／dist／打包／Git。
- **下一步具體**：建置完成後跑 `.build/prove_cad_native_read_fixes.py`（已準備未執行，guard final.log完整標記）。它載入target/release/cad-<新資產hash>新runtime：2008以turn073原87操作在新私有DWG回歸，原Drawing11前後完整snapshot不變、不另存／關閉；記native_bounds_fallback是否真命中，獨立重開86圖元；2022讀原花架Unicode檔名檔案30圖元比原獨立快照、磁碟SHA不變。結果位置會寫`.build/cad-native-read-fixes-proof-current.txt`。若失敗先保留／覆核，不自動重畫。通過後仍需真CAD窗口看新回歸DWG並寫review，原14案失敗不重分類。
- 之後以新凍結EXE/runtime做必要Strata模型代表性回歸，至少浴室架雙版（驗總外廓／finish指引）及必要參數拒絕後修正；沿用原一般用戶模糊要求，不餵答案。可另用targeted-current指標／8791，保留主current原14案供對照，不重新跑全部14。所有原失敗未存檔Drawing11、各ISSUES檔仍開，別關／補存。公司原圖revision誤報仍未重現，不宣稱本次兩修正解決它。
## 即時交接（2026-10-19 05:20，優先於下方歷史）

- **13／14已覆核，目前bathroom_shelf-2022模型中**。同controller exec20035、current `.build/cad-user-explicit-version-99d651b60a2c404ea6317d873c5c59eb`／8790／Q2 low20k128K／凍結runtime7018c7d37970aeaa…不變。使用者未暫停，不跳過最後案原生、獨立資料及真CAD畫面。不進版／dist／打包／Git。
- 浴室2008：6輪400.901秒／13724輸出tokens，36圖元／36操作，native37秒存檔重開與獨立資料PASS，SHA5a3cd3ee28e030023dfd0aee24b95c1b6e6c0b50368708aa96c51d56e6a8766f。真窗口最大化已看：側支撐x700–720＋層板720–920，**總突出220超已送200限制**，200尺寸只量板、答覆誤稱符合。兩層／四瓶／600寬／500高／280淨距可辨，尺寸小、20mm SS304假設需核實、英文不算中文；visual-review=ISSUES已寫，圖留開不改。第一次finish DWG清單不符、第二次[]成功，無重畫。詳docs最新段。
- **新來源待完整驗證**：cad_bulk.py ANSI LISP Unicode檔名/目錄分開核對；cad_worker.py native_model_bounds在系統EXTMIN/MAX無效時全圖GetBoundingBox備援，完整Count/Handle/DBMOD/文件/時間及合法範圍檢查；cad-new.md失敗保留未存檔圖面、明示簡化占位/未完成、整體外廓含支撐背板墊片、finish.artifacts填[]。85 Python契約最新通過，log .build/cad-name-and-native-bounds-unit.log。舊模型矩陣仍用凍結原80項候選，不能說本85或新提示已實測過。
- `.build/build_cad_native_read_fixes.ps1`已準備**尚未執行**：先要求cases/progress.stage=complete且cases14、全request有response，才Enter-DevShell（vcvars64 -vcvars_ver14.2／VCTools14.29）、build_cad_runtime、Build.ps1 -ValidateOnly -EmptyCargoCache -BasicValidation（實際cl1920–1929/x64探針）、workspace frozen tests、example2角色、harness15測試、release example。完成標記CAD_NATIVE_READ_FIXES_VALIDATION_COMPLETE。不IncludeInstaller、不改dist/進版。矩陣14完成後需實際執行並讀exit0＋標記，再新來源兩版native回歸及新候選必要模型定向測試；腳本存在不等於完成。
- 目前已定位原生缺陷兩項：花架2022已存檔但簡體檔名LISP反斜線U+路徑誤拒絕；住宅2008未存檔Drawing11／86圖元原生EXTMIN/MAX初始哨兵值，資料727屬性相符且視野手動調整前後全86相同。native-bounds-proof.json證明全部86原生BoundingBox合法，沒有改圖或存檔。詳下方04:43/05:12及docs。公司首次revision誤拒絕原圖仍未重現，不宣稱本兩修正已解決。
## 即時交接（2026-10-19 05:12，優先於下方歷史）

- **12／14已覆核，目前bathroom_shelf-2008模型中**。同controller exec20035、current `.build/cad-user-explicit-version-99d651b60a2c404ea6317d873c5c59eb`／8790／Q2 low20k128K／凍結runtime7018c7d37970aeaa…不變。使用者未暫停。後續浴室2022，仍須原生存檔重開、獨立資料、真CAD畫面；不進版／dist／打包／Git。
- 住宅2022完整走完兩次同檢查點續接，**PAUSED_OUTPUT_LIMIT、沒有cad-report／沒有新圖**。7輪1715.574模型秒／71834輸出tokens，t79/80/81各20k無工具，488.991/461.918/460.672秒。runner exit0不是成功，已自然進下一案，不能再重送本案。詳docs/VALIDATION_CAD_USER_SCENARIOS.md最新段。住宅2008為未存檔Drawing11／86圖元縮放範圍異常CAD_FAILED，完整證據見下方04:43，文件保持開啟不改／補存。
- **來源已有待驗證修正，舊候選未重建**：src/cad_bulk.py三個helper處理ANSI LISP BMP文件名表示法及獨立目錄核對；src/cad_worker.py native_model_bounds在世界XY模型空間、系統EXTMIN/MAX無效時逐全部圖元GetBoundingBox合併，唯一Handle／Count／DBMOD／目前文件／570秒都須通過，任何缺失停止。正常批次讀圖保留。cad-new.md補未存檔失敗解釋／簡化占位誠實及不保證工程結論。
- **85 Python契約通過**，.build/run_unicode_path_unit.py以src/cad_worker.py別名worker載入最新來源，log .build/cad-name-and-native-bounds-unit.log。尚未新版runtime、v142空快取frozen完整建置、新來源兩版native回歸或新版模型定向測試，不能稱修復完成。原型 .build/probe_house_native_bounds.py已唯讀確認Drawing11全部86圖元原生範圍合法，low[-804,-800.5,0] high[18000,12900,0]，Count／DBMOD不變，證據本案native-failure-review/native-bounds-proof.json。後續先完成14案，然後做兩修正定向實機／v142建置／必要新模型回歸。
- 先前10案與原生缺陷、驗收Oracle共邊/圓形限制見下方與docs；大廠2008真60共邊格但0間隙、2022真60設備圓+12座位圓但輪廓/通道不符；不能說只畫標籤。首次公司revision誤報仍未以原圖重現，不宣稱本次兩修正已解決它。
## 即時交接（2026-10-19 04:43，優先於下方歷史）

- **11／14已覆核，目前two_floor_house-2022模型中**。同controller exec20035、current `.build/cad-user-explicit-version-99d651b60a2c404ea6317d873c5c59eb`／8790／Q2 low20k128K／runtime7018c7d37970aeaa…不變。使用者未暫停。後續浴室雙版，必須原生存檔重開、獨立資料、真CAD畫面，不提前結束／跳過；不進版／dist／打包／Git。
- 住宅2008本案CAD_FAILED未存檔Drawing11：7輪876.403秒／39993tokens，t72 417.17秒20k無工具後continuation1的t73 355.567秒提交87操作；86已繪，最後ZOOM EXTENTS因原生範圍初始異常而停，無檔，不重畫／補存／關閉。VIEWSIZE2.04e20、EXTMIN全正1e20／EXTMAX全負1e20，worldXY。獨立COM86圖元727可比屬性相符；第一尺寸1A8原報告只有expected_entities、enumeration_error為CAD匯出期間圖面已變更，現COM可讀8000，四原生尺寸8000/12000各2，不能說缺尺寸或圖太大。
- `.build/inspect_house_zoom_failure.py`已完成，native-failure-review/snapshot.json、result.json及view-only-proof.json保存。前三次診斷因command無entities、1A8僅expected_entities、COM Point JSON而停，均沒調視野；修正後已確認全部已登記Handle、727屬性，再固定本案ZoomWindow(-2000,-2000)-(24000,14000)，視野高16000且86圖元前後完全相同。真2008最大化窗口已看兩層、相對樓梯對位、房名及家具；部分房間無門、尺寸太小、標籤跨線，英文不算中文字形驗證。failure-review已寫，controller進2022。
- 模型有正確停止，但回覆誤說超出CAD處理範圍，建議關閉殘圖重試／分樓繪製／新版無根據，未照做。此為待修錯誤解釋＋原生範圍備援，原CAD_FAILED不改判。花架2022 Unicode檔名誤拒絕亦仍待定向修正；先完成凍結版剩3案，不拿新runtime冒充本候選。
- 大廠雙版與先前案例細節及Oracle共邊／圓形辨識限制見下方04:21、04:08及docs/VALIDATION_CAD_USER_SCENARIOS.md最新段。不要把2008共邊60格或2022設備60圓說成只有標籤；兩案實際問題為間隙、輪廓尺寸、通行／標註，原始與人工校正證據分開保留。
## 即時交接（2026-10-19 04:21，優先於下方歷史）

- **10／14已覆核，目前two_floor_house-2008模型中**，同controller exec20035、current `.build/cad-user-explicit-version-99d651b60a2c404ea6317d873c5c59eb`／8790／Q2 low20k128K／runtime7018c7d37970aeaa…不變。使用者未暫停。後續住宅2022、浴室雙版，必須原生、獨立資料及真CAD畫面覆核；不進版／dist／打包／Git。
- 大廠2022已完整看實際畫面，visual-review=ISSUES：100操作／100圖元（72圓14線14字），native101秒存檔重開，7輪模型689.136秒、無length，SHA4704b79dcbad56b80582013b16cf1036ba833fc7ead06ccf284b6d4a8a06fd83。快照r1000/1500/2000各20設備圓、r500共12座位圓，**不是沒有設備或只有標籤**；Oracle只查矩形故0，需以人工辨識補充。圈不代表原矩形長寬、最小淨距2m/1m/0m；模型最後主動承認不足、需矩形與尺寸，但開頭仍稱完成。無原生尺寸DATA_ISSUES，封閉外框／底部條帶只有門標籤、無實際出入口，文字低對比與跨線。文件仍開不改／補存。詳docs/VALIDATION_CAD_USER_SCENARIOS.md最新段。
- 大廠2008同樣Oracle漏共邊，但shared-grid-review.json已逐邊證明60格尺寸各對；內部0間隙與宣稱3m矛盾、文字嚴重重疊、不應把60格說成無圖。其兩個原生尺寸存在，只2008Measurement不可讀，Oracle未找到不是缺尺寸。前所有證據與9案詳下方04:08。
- 花架2022Unicode檔名路徑誤拒絕尚未修，待此凍結版矩陣完成後定向修正；不能忽略身分檢查／自動改檔名。其他品質及工具粒度待辦照舊，不因每案問題重新開批次。現在住宅2008剛t68 load_skill，仍工作中，不結束或暫停。
## 即時交接（2026-10-19 04:08，優先於下方歷史）

- **9／14已覆核，目前large_factory-2022模型中**，同controller exec20035、current `.build/cad-user-explicit-version-99d651b60a2c404ea6317d873c5c59eb`／8790／low20k128K／runtime7018c7d37970aeaa…不變。使用者未暫停。接著住宅雙版、浴室雙版，逐案真CAD畫面／獨立資料，不跳過，不重啟已完成案；不進版／dist／Git。
- 大廠2008：10輪1659.631模型秒／76402輸出tokens，兩次20k無工具後同檢查點continuation2的t57提交79操作，原生88秒存檔重開。79圖元58線19字2原生尺寸，SHA196dd1a695930910ed0769db589775a2bea6e9783312405bf07245a0c04cc2c6，本案040043/factory_layout.dwg。先前宣稱只標籤是自動辨識漏共邊，已向用戶更正。**真窗口與逐格線覆蓋分析確認A/B/C各20格共60，尺寸各對；但內部0間隙，模型答覆機間3m通道錯誤**。辦公12格位非經確認座位動線、文字嚴重重疊、外框無門口缺口，只紅線標門，不能保證堆高機通行。visual-review=ISSUES已寫，文件留開不改。shared-grid-review.json人工證據，原Oracle不覆寫。兩個原生外廓尺寸確實存在，2008Measurement不可讀導致Oracle誤報未找到，不要誤稱缺尺寸。第二預存角色補充未送，4m/1.2m不當硬性已送條件。完整追加docs/VALIDATION_CAD_USER_SCENARIOS.md最新段。
- 其餘8案、Unicode簡體檔名誤拒絕尚未修、源碼凍結、harness15項、v142建置證據及computer-use規則見下方03:35與歷史。後續修正需定向驗證，不改現候選冒充同版。現在2022大廠已load／ask_user，模型待續，不停止。
## 即時測試交接（2026-10-19 03:35，優先於下方歷史）

- 使用者要求繼續，未暫停。current `.build/cad-user-explicit-version-99d651b60a2c404ea6317d873c5c59eb`／8790／Q2 low20k128K／runtime7018c7d37970aeaa…／controller session20035。**完成8／14，現在large_factory-2008**。後續大廠雙版→住宅雙版→浴室雙版。沒有stop-after-case，不進版／dist／打包／Git。每案await_visual或failure-review必須真CAD畫面覆核，不跳過。
- 前6案：門架雙版、桌下架雙版、花架2008資料通過但畫面／內容ISSUES；花架2022存檔後reopen身分誤拒絕，原FAILED保留。新缺陷尚未修：簡體檔名由LISP回傳反斜線U+9633／89C6／56FE，COM FullName是原字，直接比較路徑誤拒絕；COM30圖元268屬性相符、DBMOD0、SHA855fd3…。native-failure-review保存原始export／HEADER、全圖快照；真畫面已看，僅調整本案視野、幾何磁碟不變，未另存／關閉。先完成本凍結產品覆蓋，再針對原檔與兩版修正回歸，不跳過身分檢查／改檔名繞過。
- 小廠雙版原生存檔重開成功，但都無原生尺寸，獨立data.status=DATA_ISSUES。controller總分類VISUAL_ISSUES會覆蓋data字樣，內嵌data仍完整，不能讀成資料通過。2008 97圖元／6輪422.742秒，12機台6桌但無編號、封閉隔間無門，native約100秒，SHA54767b…。2022 98圖元／7輪802.102秒，先零長線建圖前拒絕再成功，12機台M1–12但沒有六席桌幾何，分區封閉、簡體貨／門問號、大文字跨框、DASHED過密；native98秒，SHA92110b…。兩份真窗口最大化全圖覆核完成，visual-review已寫，圖都留開不改。詳docs/VALIDATION_CAD_USER_SCENARIOS.md最新段。
- 工具與GUI：computer-use skill及guidance/api/confirmations已讀，使用node_repl的global sky。sky.drag不支援middle按鈕，不再給未文件化參數；曾選取框已Escape取消無改圖。窗口2022 id722826，2008 id3345444（需fresh list→唯一選取）。native verifier會restore窗口，需title雙擊最大化；2022 resize後視野裁切需wheel縮小後再判缺圖。Screenshot直接看，不存／重emit payload。COM program API可做原生唯讀診斷及本案view fit，不能Shell UIAutomation。
## 目前進度快覽（2026-10-19 03:02，優先於下方歷史）

- 使用者要求繼續，沒有暫停；current仍`.build/cad-user-explicit-version-99d651b60a2c404ea6317d873c5c59eb`／8790／Strata Q2 low20k128K／frozen runtime7018c7d37970aeaa…。controller exec20035仍跑，**完成5／14，現在花架2022**。門架、桌下架雙版與花架2008皆存檔重開＋獨立資料PASS，但實際畫面或內容有問題，均VISUAL_ISSUES。沒有進版、dist、打包或Git。
- 最新花架2008：先CP950兩處拒絕（≤、簡體現場確認），再100操作中最後ZOOM E帶color欄位Schema不符，皆未建圖。模型第三次93操作成功，93圖元（71線6尺寸9多行字7圓），正確Icad1；native約105秒，模型7輪828.148秒，無length。DWG本案025741/balcony_plant_stand_900x350.dwg，SHA `eda4cf4cda6b40a2fc7f67ef1f7dd74d9f3c7d0bda64d292312d28fed27a42fb`。最大化真CAD三視圖與下方中文逐項覆核，兩層六盆、900×350／上板560加假設盆高200總760可對照；尺寸字太小、三盆中心距220與直徑220相切無餘量、清掃區斜撐、答覆結構高780卻含盆760矛盾、材料足夠與自重估計未驗證。VISUAL_ISSUES，圖留開、不改／補存。第二預存角色補充未送達，不作硬性失敗條件。一次不支援middle的拖曳只啟動選取已Escape取消，沒有改圖；後續不給sky.drag未文件化參數。
- 桌下架2022詳下方02:45追加：49圖元／9尺寸，SHA962a46…，6輪450.552秒，native54秒。全圖與字放大已看，長中文擠疊、散熱腿部保證超過證據、深350等於主機350無前後餘量。4案比較已追加docs/VALIDATION_CAD_USER_SCENARIOS.md。
- 後續順序：花架2022→小廠雙版→大廠雙版→住宅雙版→浴室雙版。繼續原生、獨立資料、真實畫面，不重啟已完成案件。產品凍結擴大覆蓋，品質待辦不立即中斷整批；實際流程致命缺陷另診斷。await_visual逐案看後寫visual-review；多產物每份都看，未知部分失敗不重畫／另存／關閉。
## 最新有效交接：確切CAD版本候選已啟動（2026-10-19 01:44）

- **02:12最新**：current仍`.build/cad-user-explicit-version-99d651b60a2c404ea6317d873c5c59eb`／8790／low20k／128K／runtime7018c7d37970aeaa…。controller已由原80152自然結束後，**exec session20035續跑**，現在**門架2022模型階段**；2008已完整逐份原生與真CAD畫面覆核，沒有重畫已完成案例。新候選目前1/14，未全完成，不進版／dist／Git。
- **02:22更新**：同一session20035繼續，已完成門架兩版**2/14**，現在**桌下架2008**。2022門架49操作（含1指令）、48圖元（36line、8text、4dimension），正確GStarCAD.Application.22，存檔重開與獨立資料PASS；SHA `b160da13ef0609dd1a92f7378cb5968cf134d3625ce080cda68f77f0f24b2dd6`，位於本案021542/door_hook_rack.dwg。實際2022最大化畫面已看：四L型鉤、300寬／80間距及未知門縫不保證關門有表達；部分簡體字如「现场确认」顯示問號，但COM原文仍完整；原生線265為(40,3)→(0,3)，跨門幅40未加門體40兩側各3的墊片，不能確認裝配。**VISUAL_ISSUES**，未改圖或重畫，DWG仍開。6模型輪326.529秒，完整計畫136.491秒／5411tokens，native約54秒，沒有length；主要剩餘為字型／幾何與註記品質，先凍結產品擴大圖型，不因此重新開整批。
- **02:32更新**：完成**3/14**，現在**桌下架2022**，session仍20035。2008桌下架65操作67圖元（38line、13dimension、4circle、3text、7mtext、2leader），正確Icad.Application.1，無重試／length，存檔重開及獨立資料PASS；SHA `fcd9fbdc53c8f63893ca4b1b3e815acf56229f248ee4b5d511bfb8ecffbd0d2c`，本案022635/desk_shelf_woodworker.dwg。實際最大化2008三視圖已看，主機180×350×320右置、600×400層板、延長線分區及50前／上餘量可對照，中文可辨；**VISUAL_ISSUES**為尺寸／引線可讀性、沒有另確認地面及腿部位置即說腿不受影響、把6kg平均分成每支M6約1.5kg未驗證偏載／板重／固定強度，吊掛方式需木工確認。文件保留開著，不改圖。6模型輪355.272秒（先load10.507、ask22.445、compact30.977、load5.811、draw233.827／10916tokens、finish51.705），native約96秒；這輪先compact再load正常沿用方案，沒有再發生方案不可見。
- 2008這案正確使用Icad.Application.1：第一次55操作因3處簡體視／圖／間／較在建圖前拒絕（0操作），模型自行補正後**57操作、67圖元**存檔成功；再自行發現v1註記43與實際46不符，另建v2 **59操作、70圖元**，補46原生尺寸並將4圓移回板邊緣內。這是已知成功後的語意自我修正，**不是未知部分失敗重畫**。最後明示v2為交付版並保留v1，native檔案與原參數未被我們改動。模型7輪／646.068秒，兩次成功native約122／131秒，沒有length；詳原始bridge／events。
- 兩份DWG都已在2008獨立重開、單位／圖元／原生尺寸／雜湊核對，且真實窗口最大化縮放、切tab逐份查看。v1在015057／SHA `8a3394d41a04c93a2e35f0ad44712c106ef56549fdbcbc5cd229118f49478d3c`，v2在015532／SHA `8d31a87678a2334b8b5ec7725a17170b2dfcab2b6eea8c54ede7845e37e3a727`。v2四鉤／300×120／70等距／門40加兩側各3形成46可對照，中文與未知門縫不保證關門有表達；仍屬概念、卡爪安裝與材料待確認。整體**VISUAL_ISSUES**：v1已作廢但仍自動交付、視野初始很小／字與引線較小；nested review詳列v2概念改善，不當作整案CAD失敗。
- 驗收器原本只接受一份成功報告，這案原自動VERIFICATION_FAILED已保存在case-result.automatic.json／summary.before-multiple-artifact-review.json。現在tools/verify_cad_user_case.py逐份串行子程序讀取，存artifact-verifications/<時間>/獨立快照與報告，再聚合；不能只挑最後一份，任一錯版／資料問題都不被另一成功蓋過。兩份實機原生覆核已完成，root independent-verification=DATA_PASS_VISUAL_PENDING、artifact_count2，visual-review=ISSUES。已補充case-result／summary且保留自動原始結果，不覆寫native報告。
- **僅測試工具增補，產品二進位不變**：run_cad_user_cases.py新增--resume，只從已覆核案例邊界、相同EXE SHA／runtime manifest SHA／完整需求與版本續跑；已完成案例必須有對應逐份資料、畫面（部分失敗另需failure-review）及未變DWG SHA。未知中斷／未完成資料夾不重送。成功產物若另有部分寫入失敗也不能判PASS，需人工檢查。15項純資料／檔案生命週期測試已在本機通過（首沙箱暫存ACL WinError5已用require_escalated重跑通過，非CAD錯誤），見.build/cad-user-verification-unit.log。原v142建置仍369 Rust／80 CAD／4舊版本驗收／2角色；不要宣稱原build log含這15項，這是後續驗收工具測試。
- `.build/resume_explicit_cad_tests.py`核對凍結candidate的EXE、runtime全部檔案與已完成推論後，才補舊cases/manifest的兩個hash欄位；舊manifest保存在manifest.before-resume-hashes.json，候選原manifest未改，新工具SHA與理由在validation-tools-amendment.json。這次同產品續跑，不開新候選，不略過未測圖型；後續仍桌下架、花架、小廠、大廠、住宅、浴室架各兩版。若再停邊界可用正式--resume，勿直接覆寫既有案例。

- 使用者仍要求繼續測試，未暫停。**current為`.build/cad-user-explicit-version-99d651b60a2c404ea6317d873c5c59eb`／localhost8790／low／20k／128K**，controller exec**80152**。先門架2008→2022，再桌下架、花架、小廠、大廠、住宅、浴室架各雙版，共14案尚未完成；目前門架2008模型階段。不要重啟舊批次或拿舊版runtime冒充。維持不進版、dist、打包或Git。
- 新候選`.build/cad-user-explicit-version-build.log`已完整退出0，有`EXPLICIT_CAD_VERSION_CANDIDATE_VALIDATION_COMPLETE`：v142探針192930159 x64、空快取frozen基本整合、**369 Rust／80 CAD／4版本驗收器純資料／2角色流程測試**及release example。runtime完整SHA `7018c7d37970aeaa02d9b066a35d1c58645ce9dd9cb6971134d938733b13d7b4`。`.build/start_explicit_version_cad_test.ps1`已執行。
- 新發現／修正：variety門架要求2008，模型卻送`latest`，實際native selected為GStarCAD.Application.22，58操作72圖元存檔成功，但2008實際開啟顯示Invalid Dwg Version。模型最後承認錯版，不能算2008通過。現在**新圖cad_draw需確切ProgID，Broker及worker皆建圖前拒絕latest**，不自動改參數；技能／Schema已說明，未指定先inventory。其他連線／唯讀latest原規則保留。兩版實機`.build/explicit-cad-version-native-proof/result.json`通過，拒絕前沒有selected／connection，文件、目前圖、數量不變，沒有建立輸出。
- 驗收器`version_review`核對原生selected.progid；錯版只在實際建立它的CAD做獨立唯讀檢查，標記**WRONG_CAD_VERSION**，避免舊CAD被不支援版本模態框阻擋。controller保留此分類，不因畫面可讀改判成功；4項純資料測試於tools/test_cad_user_verification.py。未知／缺證據不猜版本。每案仍須存檔重開、獨立資料及真CAD畫面，不能以模型文字代替。
- 上一variety根`.build/cad-user-variety-f840e9a35fae45c98767cff32d1b7124`已於3案自然邊界停止，exec32810退出0：浴室架兩版VISUAL_ISSUES；門架2008**WRONG_CAD_VERSION**（舊自動結果為VERIFICATION_FAILED，case-result.automatic.json及summary.automatic-before-version-review.json保留，原報告未改）。新驗收器已實際在2022獨立讀72圖元／10原生尺寸，SHA `7a56f521dd21451251bbf80c91bbd0d6e519b25b075df7c572aa39109fbe346d`。已真實最大化2022視窗覆核：四掛鉤位置及門縫待確認有表達；註記75間距實為75/75/70；門體x400–440厚40，上方折鉤x395–420只跨25且未越另一側，不能視為可安裝。圖與畫面有註記擁擠；visual-review.json詳列。2008版本錯誤通知僅按確定關閉，沒有轉檔、另存、改圖或重畫。這張2022產物仍開著供診斷。
- 先凍結此候選擴大圖型測試，不為每個尺寸／標籤品質問題反覆重開同兩案；**流程／原生致命缺陷才另外修正**。後續品質待辦仍為註解比例、文字與實際幾何一致、承載／防刮不得保證、局部參數草稿／修正、成功工具結果有界摘要及高階幾何。未實作rectangle、分段計畫或提高100上限。

## 先前交接：2022混合字碼修正／無思考對照（2026-10-19 00:32，以上方為準）

- **01:06最新current**：`.build/cad-user-variety-f840e9a35fae45c98767cff32d1b7124`／localhost8789／**low／20k／128K**，controller exec**32810**，runtime仍0d22dcf81b7453d1…。先浴室架2008→2022，再門架、桌下架、花架、小廠、大廠、住宅各雙版，14案尚未完成；第一案正在模型階段。`.build/start_variety_cad_test.ps1`已啟動，首次因複製腳本改到buildLog路徑而在送模型前停止，修正為真實mixed-ansi-build.log後才啟動。不要將啟動失敗當模型案例。
- **01:17更新**：variety的浴室架2008已完成，**VISUAL_ISSUES**；正在浴室架2022。2008原生46操作（模型筆記誤寫45，實際46）、61圖元／6原生尺寸，存檔重開及獨立資料PASS，SHA `2e50da919dc7a1bc88598a35ac7ea3b75d0abc842353d9e7c502b61f5f4507fa`，DWG在本案project/_AI_Output/20261019_011148/bathroom_shelf_design.dwg。真實最大化ICAD2008窗口已覆核：600×180×263三視圖對照、兩層及257淨距可辨，尺寸字太小／四位小數；R3只在註記，沒有圓弧；全英文，不能算繁體字型測過。模型最終「3mm板承重5kg足夠／不刮人」沒有工程驗證支持，visual-review有記錄，不當加工核准。此文件因ISSUES仍開著，不改幾何／補存；後續2022使用另一CAD程序。t1 ask21.456s、t2 load10.047、t3短方案41.895、t4完整46操作303.039s／13620tokens；原生約82秒；兩次finish47.748／36.08秒，第一次artifacts清單不符，第二次CAD路徑由桌面交付／artifacts=[]才通過，無CAD重畫。共有6模型輪，全部無length。完整矩陣尚未完成。
- **01:27更新**：浴室架2022也完成資料及真實CAD畫面覆核，**VISUAL_ISSUES**；現在跑門背掛架2008，controller仍32810、current仍variety根。2022共45操作46圖元（27line、4circle、6dimension、6text、1mtext、2leader），毫米／獨立資料PASS，SHA `346982349269d6e7b5c32a18a25d2d85f059e97a869e67ce4c20b0f8c9f50fb1`；DWG在本案project/_AI_Output/20261019_012109/浴室壁挂架_討論稿_v1.dwg。CAD窗口最大化並wheel縮小確認完整三視圖、中文、原生DASHED投影、四固定點及擋條；尺寸字仍過小、引線附近註記擁擠。**圖上Mtext淨高240，原生層間250、扣3板厚247；最終回答說247但圖上未改，交付矛盾。** R5僅加工註記未畫圓弧，承載／防刮不視為已驗證；最終有明示概念非施工核准。5模型輪308.425秒，完整計畫138.041秒／5480tokens，native約62秒，沒有length或重畫。兩個不同方案不能當CAD版本純速度比較。兩版有問題的DWG仍開著、不修改；批次僅完成2/14，後續多種類型繼續，不為每個品質問題重新開整批。
- none住宅2022比較已完整走完既定續接、於自然邊界停止：exec86671退出0，根`.build/cad-user-mixed-ansi-055aae084d384b5691fb33244b0b8eab`，**18輪／1785.911秒模型時間／88139輸出tokens**，t12/17/18各20k無工具（353.245／368.944／482.385秒），只有t15的162操作完整但超100被建圖前拒絕，**無cad-report／無新圖，PAUSED_OUTPUT_LIMIT**。没有2008或其餘13案完成；24秒短診斷不能代表此完整任務。`model-summary.json`／cases/summary.json保存；current已換上方low新批次。暫不採none作主要矩陣模式，不能稱none改善完整CAD效率。

- **00:35最新**：mixed-ansi候選建置exec62794已退出0，v142探針192930159 x64、空快取frozen基本驗證、368 Rust／79 CAD／2角色測試與release example全部通過，日誌有`MIXED_ANSI_CANDIDATE_VALIDATION_COMPLETE`。runtime完整SHA `0d22dcf81b7453d1f287692d853e8af2f5a540bbd7458090c5634d9578d15efd`。
- 新矩陣已啟動，**current現在為`.build/cad-user-mixed-ansi-055aae084d384b5691fb33244b0b8eab`／localhost8788／none／20k／128K**，controller exec86671；先住宅2022→2008，再小廠、大廠、浴室架、門架、桌下架、花架各雙版，14案尚待完成。`.build/start_mixed_ansi_cad_test.ps1`已執行；原8786在自然邊界停止、無待完成推論。新none不改模型權重或全域設定；實機寫入、獨立核對與實際CAD畫面關卡不變。
- **00:53實測**：none住宅2022尚未建圖。t1追問18.474秒／506tokens；t2先compact11.134／326、t3載入4.433／101、t4再compact17.145／443；後續兩次ask_preference及多次compact，t9/10/11/13/14有tokens但原生API content=null且無tool_calls，t12用滿20k／353.245秒（約56.6 tokens/s）仍無工具。正式continuation1的t15終於回cad_draw、151.229秒／8187tokens，**162操作**被Broker建圖前拒絕，沒有原生報告。t16compact14.321／298，t17待修正。工具與design_planning實際都有送，不是再次遺失筆記；none短回覆快不等於完整任務快。已設`cases/stop-after-case`，**讓當前案完成既定有限續接／若有圖照常實機覆核，再於案例邊界評估後續模式**，不讓完整none矩陣默默跑下去，也不把它提前算成功。下一步宜較廣類型，不能一直只測住宅。162操作只用8187tokens，提示大型圖的操作100限制本身也需後續評估；尚未提高上限／新增rectangle或草稿工具。

- 使用者要求繼續，未暫停。8786住宅2022已自然完成並覆核失敗部分圖，在一案邊界停止，controller81122退出0；current仍`.build/cad-user-handoff-b78e05da0bb6463abae8a3f70c34b751`。只做完1案，不是14案完成；不進版、dist、封裝或Git。
- 第4輪low耗496.555秒／20k且無工具；正式同checkpoint續接後第5輪424.155秒／16284tokens送105操作，在建圖前拒絕；第6輪219.576秒／9539tokens重送97項（prepare後94操作），實際102圖元，在verify_before_save因cp950單一B2無法解碼而未存檔。第7輪52.083秒／1459tokens正確區分未存檔與已繪製，但不應將未驗證布局宣稱無問題。
- 實際2022視窗已覆核，兩層8×12m／家具／對位樓梯可見，文字擁擠重疊、尺寸過小、通行未核實；`failure-review.json`及`native-failure-review`保存102圖元326屬性全相符與原始匯出。原Drawing2.dwg仍未存檔，**不重畫、不補存、不關閉**。此案CAD_FAILED，不因後續讀回修好而改判成功。
- 新錯誤為LISP把m²中的U+00B2寫成單一B2，混在Big5資料內；不是先前餐字尾5C跳脫問題。修正src/cad_bulk.py以surrogateescape保留不可解碼位元組，僅含這種資料的主／子圖元改用既有COM描述器；native_revision以ASCII JSON無損雜湊原始值。JSON結構、文件標頭／結尾、nonce／Handle仍嚴格；不猜字碼、不replacement、不重讀整張圖。這不宣稱所有ANSI混碼均已解決；其他格式或身分錯誤仍停止。
- 新唯讀原生證據`.build/mixed-ansi-native-proof/result.json`：2022全102圖元898屬性全相符、13文字圖元備援、2.019秒；2008既有97圖元824屬性全相符、無該備援、2.317秒。兩版重複snapshot完全相同，DBMOD／數量不變、不寫幾何／存檔。原生資料通過不代替新模型存檔／重開驗收。
- 同turn004保留請求的none串流診斷完成：`.build/cad-stream-diag-86cd7e056731465ca89b6da3d0c953f5`，24.377秒／832tokens，只有compact_context、無CAD工具執行。**不能說none畫圖只需24秒／CAD通過**；串流且單次隨機推論不作純速度因果證據。
- 新79th CAD契約已加；目前v142／空快取frozen完整候選建置在exec62794，`.build/cad-user-mixed-ansi-build.log`，以退出0和`MIXED_ANSI_CANDIDATE_VALIDATION_COMPLETE`為準。不要拿上一runtime401a或前368/78結果冒充新完整驗證。`.build/start_mixed_ansi_cad_test.ps1`已準備但**尚未執行**，完成後可啟8788／新的none、20k、128K矩陣，原7類模糊需求不變，先住宅2022→2008，再其餘各雙版本。程式與推論模式同時改動須註明，不把所有差異歸於none。只改本次API effort，不改Strata全域設定／權重。

## 目前有效交接：真實CAD矩陣繼續（2026-10-18 23:59，以上方更新為準）

- 使用者已要求繼續，沒有暫停。**最新current**為`.build/cad-user-handoff-b78e05da0bb6463abae8a3f70c34b751`／localhost8786／Strata Q2／low／20k／128K，controller exec session81122仍執行；先住宅2022→2008，再小廠、大廠、浴室架、門架、桌下架、花架各兩版，共14案待驗。維持逐案原生存檔重開／獨立資料／真CAD畫面，不省略；不進版、dist、封裝或Git。
- 目前候選已通過v142實際探針192930159 x64、空快取frozen基本整合、**368 Rust／78 CAD／2角色測試**及release example；`.build/cad-user-handoff-render-build.log`退出0、有HANDOFF_RENDER_CANDIDATE_VALIDATION_COMPLETE。Runtime仍`401a5d15bacf1186d31d0f4a23395cb160aadeb2e4e274d3f4e4ba14ba1e5a16`，新EXE與runtime已凍結在候選bin。
- 修正後正式runner受控回放PASS：`.build/handoff-replay-fe54ed3296694600bb73e65a87bd64b0/bridge/replay-check.json`確認先compact_context後load_skill時，下一輪同時有cad_draw及原方案。**此回放固定回覆，不用模型、不畫圖，不能算Strata通過**。前367項候選回放曾抓到runner只在已有DWG時送方案的缺漏；368版已修，規劃與建圖前錯誤也會送。先存方案後載入可沿用同一Broker筆記，其他技能筆記不覆蓋已啟用CAD方案，新設計仍清除，授權未改。
- 既有8785住宅2008：94操作／97圖元存檔重開及獨立資料PASS，實際畫面為VISUAL_ISSUES（尺寸太小、樓梯文字重疊），模型總1008.765秒，沒有length；DWG、SHA、完整差異見下方歷史及該案visual-review.json。8785住宅2022在模型-only回歸除錯時中斷，**不算完成或自然CAD失敗**；pending13晚到已保存、未執行。現新批次要重新測，不能拿舊資料冒充。
- 後續改善僅記錄、尚未實作：大圖可讀尺寸比例（目前DIMTXT/DIMASZ3、DIMSCALE1）、本機計畫局部修正避免100項全部重送、成功結果的有界模型摘要（之前單次56,901字／41,155輸入tokens）。原生Big5尾0x5C／CP950彙總已修且兩版原型通過，主矩陣仍需新模型驗收。先凍結本候選做較廣測試；流程缺陷或原生致命錯誤才另診斷。
- 用`.build/inspect_staged_progress.py`（`C:/Python313/python.exe -X utf8`）看最新輪次；controller的`cases/progress.json`為await_visual或await_failure_review時必須實際檢查後寫對應review，不能只判資料成功。角色未交付的8×12／1F WC等隱藏條件不可作失敗依據。技術細節與早期嘗試保留在下方／docs/VALIDATION_CAD_USER_SCENARIOS.md。
- 2026-10-19 00:04實測進度：8786住宅2022首次ask_user42.923秒／1196tokens，角色已提供8×12m／1F WC等資料；同一新task再compact_context29.122秒／792、load_skill5.753秒／129。真實turn004請求已確認cad_draw可見、原筆記及design_planning都送出，見候選根`handoff-live-request-check.json`，至此沒有read_skill_guide。現在turn004生成完整計畫，未有本案原生CAD報告；不能提前算通過。請保持此凍結候選繼續觀察。

## 恢復測試歷史（2026-10-18 22:33，以上方最新交接為準）

- **23:55最新**：8785的住宅2022回歸嘗試已在確認cad_draw仍未公告、該案無任何cad-report後取消pending turn013，停止專屬controller／runner／worker（session27619已退出1，這是主動除錯中斷，不是自然CAD失敗）。`gate-interruption.json`及cases/progress.json=stopped_for_gate_regression。原生CAD／Strata未關閉；turn013晚到為compact_context、439.829秒／17438tokens，已保留但取消後未執行，不可續用。這不是完整2022通過，也不宣稱永遠死循環。
- 回歸原因／修正：收到必要補充的新task先compact_context（turn009，28.986秒）再load_skill(cad-new)（turn010），原本程式忽略未載入技能前的同任務筆記，令cad_draw仍不可見而多讀cad-commands／cad-geometry。已修正同Broker的通用交接可在首次載入CAD時沿用；其他技能後續筆記不覆蓋已開始的CAD方案，真正切入下一新設計仍清除。LoadSkill結果明示planning_note_saved及next_action，舊checkpoint／寫入授權不改。
- 第一輪handoff建置367 Rust／78 CAD／2角色及v142空快取frozen已過，但**正式runner固定回覆回放**發現另一缺漏：cad_draw已公告，方案雖在Broker內，runner只在已有artifacts或blocked時送cad_artifact_index，導致無DWG時方案沒送給模型（不是helper誤判，回放第三請求無design_planning）。已修runner.has_cad_context，方案／last_execution存在也要送，並加368th回歸。`.build/replay_handoff_flow.py`只用固定回覆，**不推論、不繪圖、不算Strata通過**；失敗第一份根由`.build/handoff-replay-current.txt`指向，原始請求保存。必須新版EXE重跑此回放至PASS，確認同時有cad_draw及原筆記，才開新Strata矩陣。
- 現正建置`.build/cad-user-handoff-render-build.log`（exec session86139），預期368 Rust／78 CAD／2角色，runtime仍401a5d15…；以HANDOFF_RENDER_CANDIDATE_VALIDATION_COMPLETE和退出0為準，不沿用上一handoff完成。完成後執行`C:/Python313/python.exe -X utf8 .build/replay_handoff_flow.py`（需本機loopback授權），成功後`.build/start_handoff_cad_test.ps1`驗證回放binary hash與新release example一致、前推論已完成才啟8786／全新cad-user-handoff根，先2022再2008各7類14案。**尚未啟動8786，current仍舊8785停止根。** 不進版、dist、打包或Git。
- **23:36目前**：8785凍結候選仍執行，住宅2008已完成資料／實際畫面覆核，case-result=VISUAL_ISSUES；已開始住宅2022，session27619仍運行。2008第2輪短方案62.931秒／1950tokens（前8784為423.554／16212）；第3輪282.310秒／12103tokens送101項，Broker建圖前拒絕；第4輪400.171秒／16574tokens改94項，原生一次列出78／88／90三處楼而建圖前拒絕；第5輪139.437秒／5473tokens修正TC後94操作完成，存檔重開97圖元（69line、7circle、17text、3dimension、1unknown/hatch），mm、SHA256 bddaec1eaec8558e06af56e9e7a6f46e3d7bebee73e2dc73c13baaffbe8e693d。DWG在本案project/_AI_Output/20261018_232608/home_two_floor_plan.dwg。第6輪finish111.806秒／3667tokens，模型總1008.765秒，沒有length。曾在commentary誤說100操作，已明確更正最終94。
- 2008真實maxCAD畫面可看懂兩層／樓梯對位、三房兩衛、陽台、門洞、家具及假設／非施工註記；但原生尺寸字高太小、樓梯文字重疊，外部入口／實際通行未確認，visual-review.json=ISSUES。初始使用者沒給基地尺寸／1F WC，模型未詢問角色資料，**不能用隱藏persona的8×12m及1F WC當失敗依據**；12×8m已明示假設。檔案資料通過不等於可施工，也非整套14案通過。
- 新發現效率／能力待辦先記錄：成功後的工具結果仍有56,901字元完整native診斷，最後回覆輸入41,155tokens；原始結果已另存，後續宜僅給有界摘要並保留原始worklog。101項／修3字仍需重送全計畫，宜評估本機計畫局部修正；原生尺寸樣式目前硬設DIMTXT／DIMASZ3及DIMSCALE1，大圖缺可讀比例設定。**這些尚未實作，先以同一401a／8785凍結候選完成較廣矩陣，不為每個品質問題重啟全部測試；原生致命錯誤才另診斷。** 局部方案暫存／rectangle／annotation_scale都只是後續方向，不可宣稱已有。
- **23:11最新執行**：新候選`.build/cad-user-gated-c15365ac18134f5d835a34df1bed9ebb`／localhost8785／low／20k／128K，current已更新，controller exec session27619。runtime401a5d15bacf1186…。gated-build.log完整退出0，v142實際探針／空快取frozen基本整合／365 Rust／78 CAD／2角色測試及release example均通過，已開始住宅2008，7類×兩版14案仍待實測。舊8784一案在安全邊界停止，88圖元未存檔部分圖仍保留，不由新候選修改或補存。新批次包含真正規劃工具門檻、Big5修正、CP950彙總及失敗回覆指南，原生原型證據不能代替整套新模型測試。
- **23:06最新交接**：8784文字分階段候選的住宅2008已結束，原根`.build/cad-user-staged-471c3ecc21fd4d1da1c2b83718968f0f`停於stopped_at_case_boundary／completed1，controller session27088退出0。第2輪compact_context423.554秒／16212tokens，第3輪完整88操作198.214秒／10376tokens，無length；第一次CP950拒絕楼，模型修正TC後第4輪105.145秒／4783tokens重送88操作。實際75線13文字，全部畫出，但verify_before_save JSON Invalid escape，未存檔；不是全案通過、未測2022或其餘13案。新Drawing2仍未存檔，88Handle／279項原生屬性獨立核對全相符，已實際max窗口／縮放覆核。沒有原生尺寸、門洞／可見動線；基地12×10m自行假設。模型最後誤稱「建圖前失敗、未建立」並建議ASCII／換CAD，原錯誤與failure-review.json保留，不採用該建議。
- 查明真正匯出錯誤：Big5餐=C0 5C，ANSI LISP逐byte跳脫把尾5C再加一份，產生客餐\\廳的非法JSON。兩版唯讀字串探針證實strlen按bytes、LISPSYS不存在；修正src/cad_export.lsp依Windows GetCPInfo的DBCS lead ranges成對輸出，真反斜線／引號／控制碼仍跳脫，Unicode LISPSYS1／2不套用bytes模式。固定8組原生roundtrip兩版PASS，原88／58圖元及DBMOD不變；完整新bulk原型讀2008 0.510秒279項、2022 2.556秒499項全部相符，證據`.build/dbcs-json-native-proof/`及`.build/dbcs-bulk-native-proof/`。這是修正原型實機證據，後續新EXE＋模型仍待驗。
- 新對照候選原始碼已加入：cad_design文字方案狀態／保存恢復（不含授權），fresh cad-new規劃時不公告cad_draw完整Schema，compact_context成功後才公告；重載同技能不清方案，舊checkpoint缺欄位維持相容，直接猜工具也在Broker建圖前擋。簡單新圖只需1–2句方案。另聚合最多20項／每項8種CP950問題字元並標省略，不自動替換；finish失敗說明強調drawing_started與saved不同，禁止據未存檔推論未建圖。
- 正在`.build/cad-user-gated-build.log`建立新的v142／空快取frozen候選（exec session13405），runtime401a5d15bacf1186…；365 Rust、78 CAD、2角色測試及完整release example以最後GATED_CANDIDATE_VALIDATION_COMPLETE和退出0為準，尚未看完不能稱全部完成。`.build/start_gated_cad_test.ps1`準備8785／新隔離根，等前批安全邊界、無舊執行器／未結束推論、新建置標記才啟動；尚未執行，current仍8784。恢復後先查建置結果，不能拿舊76ad runtime做新功能冒充。
- 22:37新候選已啟動：`.build/cad-user-staged-471c3ecc21fd4d1da1c2b83718968f0f`／localhost8784／low／20k，runtime76adf657…。current指標已更新；controller exec session27088，先住宅2008，7類×2008／2022共14案待驗。正式v142探針、空快取frozen基本整合、361 Rust／75 CAD／2角色測試及release example全部完成，resume-build.log有STAGED_CANDIDATE_VALIDATION_COMPLETE且退出0。不能把建置通過當模型案例通過；分階段提示的實際效果仍在觀察。
- 使用者已明確表示完成電腦使用、要求繼續測試，解除下方暫停；維持本輪不進版、不更新dist、不打包、不推送。
- 已確認Strata loaded、Q2、131072上下文，三份重開機備份SHA256一致，正式連線流程可連2008／2022，見`.build/cad-user-staged-environment.log`。沒有恢復原取消任務或執行晚到回覆。
- 以MSVC v142、空Cargo快取frozen重新完整驗證候選，進度見`.build/cad-user-staged-resume-build.log`；75 CAD契約已通過，其餘及新版測試EXE以完成標記為準，未完成前不啟動新批次。
- `.build/start_staged_cad_test.ps1`已補paused_by_user取消證據、舊推論完成／無舊執行器及新版建置完成標記檢查。待建置全通過，啟動8784獨立新批次，先測分階段住宅案例，再按實測結果調整；原始模型／CAD／資料／畫面驗證關卡均保留。

## 使用者暫停交接（2026-10-18 19:06，已由上方恢復）

- **重開機前備份已完成（19:20）**：使用者另行授權保存後續需要的圖面，僅三張本輪失敗診斷圖另存到`.build/reboot-preserved-20261018-191620/`：bathroom-shelf-2008-partial.dwg（31圖元）、balcony-plant-stand-2008-partial.dwg（54）、door-hanger-2022-partial.dwg（58）。各自開啟獨立磁碟副本、全Handle及可讀描述屬性與存檔前記憶體一致，驗證副本Close(False)、原文件未關閉，備份SHA256穩定；詳saved-and-reopened.json，指標`.build/reboot-preserved-current.txt`。這是使用者授權的重開機備份，原模型／原生報告仍FAILED，不改成通過。2008花架SaveAs寫出檔案但來源COM FullName仍殘留先前失敗路徑，因此以獨立開檔資料證明備份有效，不重送SaveAs。初版保存腳本的FullName斷言／比較函式錯誤及修正版日誌均留.build。
- 其他既有645圖元法蘭／Undo圖都有原檔、來源及診斷證據，可從原檔重新建立測試，不需要保留記憶體修改或Undo堆疊。空白Drawing1不需要保存；沒有關閉CAD、沒有重新開始模型矩陣。重開機後從上述三份DWG恢復診斷，不再依賴舊Drawing編號／COM工作階段。
- 8783取消後的第7輪已保留晚到回覆：465.254秒、20,000 tokens、沒有工具；不能執行該取消結果。下方「可能仍在推論」是暫停當時紀錄，這一輪現在已完成，整體仍由使用者暫停。
- 使用者要先使用電腦，明確要求儲存進度、晚點再繼續。**目前停止，不自行恢復測試、編譯或UI操作。** 沒有進版、更新dist、打包或推送。
- current仍是`.build/cad-user-model-20k-matrix-7d8a0fd3600541d28c0da417ce2ff80e`／8783；住宅2008到第6輪已有三次20,000 tokens且無工具。第7輪模型等待時已正式POST取消，確認cancelled；核對命令列後停止本輪controller12516、runner24044、worker29200。沒有cad-report、沒有本案CAD繪圖；CAD應用及先前圖面保留。證據`user-pause.json`，cases/progress.json為paused_by_user。
- 已送出的第7輪Strata推論可能自行結束，取消後不會執行工具；晚到回覆由仍開的本輪bridge保留。恢復前先查`bridge/turn-007.response.json`及原始日誌，不把取消當成功／CAD失敗，不與未結束GPU推論重疊測速。
- 使用者建議分次完成以避免20k耗盡。已修改agent/skill.md、legacy skill.md與cad-new.md：必要條件→compact_context保存200–400字配置→下一輪完整原子cad_draw；不假裝多次cad_draw可追加同圖，100操作限制仍在。**新提示尚未送Strata實測。** 流程已記入docs/LOCAL_MODEL_VALIDATION.md。
- 分階段候選首次建置因CAD技能上下文超過5000 bytes測試失敗，已縮短提示，未放寬門檻。第二次實際v142探針192930159 x64、Clippy及基本Rust測試通過，到75項Python CAD契約時4個WinError5暫存目錄權限錯誤停止；**建置未完成，沒有workspace361／example2／新版測試EXE的本輪完成證據**。原始`.build/cad-user-staged-build.log`與`...-rerun.log`保留；執行session22192已退出1，不存在背景編譯。
- 恢復後先以允許暫存檔操作的相同v142環境完整重跑Build.ps1 -ValidateOnly -EmptyCargoCache -BasicValidation，再workspace／example測試及release example；不要省略失敗或沿用舊EXE冒充新提示。`.build/start_staged_cad_test.ps1`是尚未執行的隔離啟動草稿，目前只接受舊批次stopped_at_case_boundary，需依user-pause證據調整安全檢查才可啟動；預定8784、新根目錄、先住宅再其餘7類×2版本。未建立新候選、未啟動8784。
- 前面的桌下架2008真實存檔／資料／畫面通過紀錄仍有效，整個14案矩陣沒有完成。後續繼續保留原生存檔重開、獨立資料與真正畫面覆核，不因分階段提示而跳過驗證。

## 本機模型長期驗證與CAD多情境（2026-10-18，已按上方要求暫停）

- **目前最新8783**，current指標為.build/cad-user-model-20k-matrix-7d8a0fd3600541d28c0da417ce2ff80e；low／20k，runtime76adf657…；從住宅→小廠→大廠→浴室→門架→桌下→花架，各2008／2022，14案仍執行中。v142實際探針／空快取frozen／75 CAD／361 Rust／2角色測試及隔離EXE建置退出0（.build/cad-user-20k-path-build.log）；不進版、打包、更新dist或推送。
- 8782花架2008只畫54項幾何，模型誤以為下一輪可追加尺寸；cad_draw說明已明示每次獨立新圖、完整必要尺寸／註記一起規劃。檔名自取阳台（U+9633）造成2008 SaveAs原生錯誤通知阻塞，僅用UI關閉兩個錯誤通知，沒有另存／重畫；實際無DWG，Drawing4保留。54圖元／166項獨立COM屬性及真正畫面覆核完成。新完整存檔路徑CP950預檢已實機證明建圖前拒絕、文件10→10及54個Handle不變；NativeSave錯誤不是模型排隊或output限額。
- **最新為8782**，current指標指向.build/cad-user-model-20k-suite-d6683e6764a448adbe4e3ca22f0383fc；low／20k、runtime e39648a5…，從花架→住宅→小廠→大廠→浴室→門架→桌下，各兩版本，全14案仍執行中。nullable提示修正建置退出0：v142實際探針192930159 x64、空快取frozen、74 CAD／361 Rust／2角色測試；不進版、打包或推送。
- 8781桌下2008已完成63圖元存檔重開／獨立資料／真正CAD三視圖覆核，僅木工討論概念圖PASS，實物承載／桌板厚度未驗證；共享唯讀SHA256確認未變，Get-FileHash分享鎖錯誤不可當內容變更。首計畫省略line_style失敗是指南與Schema矛盾：required nullable欄位須填null，restore_optional才還原省略；已對齊通用及線型說明並加入strict true／false回歸，沒有放寬必填驗證。該批在一案完成邊界停止，舊圖與證據保留，最新14案仍包含桌下重跑。
- 最新候選為localhost8781、Q2／128K／low／20,000輸出，current指標在.build/cad-user-current.txt。新runtime e39648a5…，v142實際探針192930159 x64、空Cargo快取frozen、74 CAD契約、360 Rust、2角色測試及隔離EXE建置退出0（.build/cad-user-20k-unicode-build.log）。從桌下收納→花架→住宅→小廠→大廠→浴室→門架，各2008／2022，14案仍執行中，不能稱全通過。
- 8780門架2022實際52操作／58圖元，bulk TextString把CAD Unicode表示法當成原文字而誤判未存檔；獨立COM204項屬性全部相符。已實際看全圖及標題，部分簡體字形問號仍是外觀問題。修正只對含Unicode表示法的文字沿用COM描述器，其他保留bulk；原私有圖重讀3.260秒、11筆文字均與COM一致。三份leader Schema補三點順序及後兩點y相同；原生限制未放寬。原失敗圖沒有儲存或自動重畫，已完成failure-review後在案例邊界換候選。
- 最新使用者要求Strata每輪輸出上限提高至20,000，128K模型上下文不變；隔離轉接新增--max-tokens，同步公告最大／預設並轉送實際額度。新批次暫存指標.build/cad-user-20k-next.txt；8777舊8192門架案在無cad_draw／原生報告的模型修復等待時安全取消並停止所屬隔離程序，保留interruption.json及晚到生成、不執行工具。不是CAD失敗，也非完整案例通過；尚未取得20k推論結果時不得稱改善。
- 最新候選補齊正式Inbox普通用戶偏好回覆及每輪有界偏好狀態；修正本方錯誤polyline建議，cad_draw目前只有line等既有種類，矩形需4項line。最新v142空快取frozen基本流程、72 CAD契約、360 Rust workspace、2 example角色測試及測試EXE通過，見.build/cad-user-persona-final-build.log；不代表14案已成功。
- 同一舊請求串流重播：none回完整100條line但內容錯誤，low耗盡8192思考tokens無工具。僅為無工具執行的診斷，不能當CAD通過或穩定速度。8776偏好無人回答的55輪案最終為回覆修復上限，已更正原本輸出額度的誤分類。
- 20k low同題重播以18,422 tokens／429.029秒回完整73項cad_draw，CP950仍有5項文字不支援，未執行；只能說較大額度讓這一次完成生成。8778轉接誤改task封套導致HTTP400，已修正並補回歸，不把14次失敗啟動算模型矩陣。實際候選改8779，.build/cad-user-current.txt指向新根目錄，已確認正式請求實際max_tokens=20000；完整CAD結果仍待驗證。
- 8779 none＋20k門架2008以18輪／1298.966秒結束：82操作引線末段不水平、修正版106操作超限，皆未建圖；兩輪20k耗盡無有效內容，最後暫停，assessment.json保留。不是COM寫入失敗或14案已測。已在自然案例邊界切到8780獨立low＋20k批次，current指標更新，產品EXE／runtime／角色不變，仍保留全部資料與CAD畫面驗證關卡。

- 使用者要求保留完整／代表性／重點三級實測流程，先完整測CAD；不趕時間，不省略存檔重開、獨立資料及實際畫面覆核。案例採一般用戶模糊需求，包含日常架具、住宅及工廠，不只調同一法蘭。
- 長期流程在docs/LOCAL_MODEL_VALIDATION.md；案例與角色在tools/cad_user_cases.json；正式runner批次入口examples/cad_model_suite.rs及tools/run_cad_user_cases.py。命令列流程與真實GUI驗證分開，不自動同意既有文件寫入或失敗後重畫。
- 前兩批發現新設計誤讀既有圖、角色未回答另開選擇、2008字碼頁丟字、異常視野、模型超出100操作及輸出額度暫停；保留全部失敗並修正。詳見docs/VALIDATION_CAD_USER_SCENARIOS.md，不把尚未完成矩陣寫成全部通過。
- 目前完整候選根目錄由.build/cad-user-current.txt指向；每案2008／2022獨立專案，具體思考模式及額度看該批manifest，localhost轉送原始當輪messages／tools。暫停最多以同一正式檢查點續接兩次，依實際pause_reason區分輸出額度與回覆修復上限，不新建任務代替續接。
- 新增操作數明示及read_work_log有界結果摘要，避免大型參數蓋住錯誤；最新EXE已納入此修正、最近工具失敗與偏好狀態。360 Rust workspace通過，真實模型效果仍須按各案證據判定。
- Outlook帳號與企業加密環境不足項只能標模擬／待公司實測，不能當真實完整驗收。這輪不進版、不更新dist、不製作安裝包、不推送Git；保留使用者既有icon、onboarding資料及本機連線設定。

## CAD Q2 問題修正與再測（2026-10-18）

- 使用者要求依Q2實測修正、持續送模型，並以上下文／提示詞優化。授權必要的隔離測試建置及2008／2022私有副本測試；本輪不進版、更新dist、封裝或推送。
- 修正方向：程式固定目前文件與版本、保留停止原因／原生修改證據／少量精確查詢，避免上下文整理引導重讀或跨CAD；總覽識別有界的封閉直線輪廓，基本編輯格式常駐，不再強制先讀指南。
- 2008找到可行原型：RunCommand原生Undo群組＋固定受限LISP半徑／ACI修改，等待完成回條，再完整核對。原生修改後一次Undo已能完整還原645列；不能採用COM屬性外包Begin／End或單次LoadLISP包Begin／End的失敗方案。
- 完整引擎已通過兩版本兩圖元一批修改、全圖核對、一次Undo還原645列。群組必須涵蓋Regen與批次核對；2022用原生Begin／End，2008用RunCommand＋固定有限LISP。2008額外讀回後一次Undo也通過；2022額外刷新可能留下空Undo項目，仍有限制，不能自動Undo清除。
- 先前診斷助手漏啟BULK_ENABLED而走慢速COM，在寫入前停止；另有無回條等待、群組過早結束的失敗紀錄，均保留.build，不算通過。建置使用Build.ps1 -ValidateOnly -EmptyCargoCache -BasicValidation及額外定向回歸，v142實際探針不省略。
- 實際GUI／Strata Q2的2008及2022各兩次中心圓Ø60→80均完成，獨立645列比對僅目標半徑改變、來源與副本磁碟雜湊不變。v2為6輪57.772秒／5輪50.895秒；v3為5輪71.255秒／5輪75.049秒，皆為模型耗時，不含工具及人工確認。先前Q2的2022是18輪199.790秒；2008先前失敗，不能比較成已完成任務。
- 第一候選版誤認缺少CAD能力，補強技能啟動；v2的2008成功後誤查失效快照，v3依生命週期移除無效工具後未再發生。v3的2008最後答案誤把輪廓編號當Handle並猜類型；再補完成回覆提示及同上下文重播，保留原錯誤，不稱零幻覺。
- 70項CAD契約與6項新增Rust定向回歸、基本整合、自檢及空快取frozen的v142候選建置通過，沒有本輪全部354項Rust或安裝包生命週期。最後只改完成回覆提示，做最後一輪模型重播及建置，未再完整GUI重跑；詳見docs/PENDING_CAD_Q2_OPTIMIZATION.md與.build/strata-q2-opt-44798bfccc4d455aab821b4652585a09/summary.json。公司原圖首次revision拒絕仍未重現，未放寬revision檢查。

## Strata Q2 比較補測（2026-10-18）

- 使用者自行換成 qwen3.8-flash-next-q2_0，授權再測；128K、none、8192輸出上限與既有0.8.69 EXE不變。本輪未改產品、模型設定、編譯或發布。
- 五份相同Q3輸入重播：原生生成加權27.94→44.64 tokens/s，總推論159.090→105.363秒。只是固定請求推論，不能當成完整流程或工具執行。
- 正式GUI的2008副本：正確定位後一次修改仍因StartUndoMark在寫入前停止；Q2之後以version=latest重讀到2022並準備重試，已手動停止，未授權跨版本寫入。第31輪生成在取消後保留但未執行；原生報告僅一次edit，2008獨立核對645圖元全未變。
- 獨立2022專案：實際僅26C半徑30→40且未存檔，全部645列比對／來源與磁碟雜湊通過；但18輪／199.790秒（Q3為9輪／189.945秒），生成43.38 tokens/s。模型重複查詢且最後誤稱其他孔含Ø15（實際4×Ø12及1×Ø20），不可稱零錯誤通過。
- 沒有0/100全圖分頁；Q2實際最大輸入約1.4萬tokens，不能證明128K填滿、公司模型或量化品質；一次比較也不能證明差異必然由量化造成。證據.build/strata-q2-587c4789d7b34db08b7f4366b21d1f7a/summary.json、observations.json；真實GUI模型原始紀錄沿用Q3根目錄第19–49輪，Q3舊summary保留。

## Strata 128K 真實 GUI／CAD 測試（2026-10-18）

- 使用者授權連線到本機 Strata，成功後送先前 CAD 驗證；本輪未改產品或進版建置發布。用既有0.8.69 EXE、正式runner/Broker/worker，在兩個獨立空白專案測已存在645圖元法蘭的目前圖面中心圓Ø60→80，不是由Strata重新畫完整法蘭。
- Strata狀態max_context=131072，model=qwen3.8-flash-next-iq3_s、reasoning_effort=none。MCP連線成功；正式GUI透過127.0.0.1:8771直接轉送每輪messages/tools，API key只由既有Y:/python接口助手讀入記憶體。沒有Codex模型代為推論／修正工具參數。
- 2008：一次快照／圓查詢／修改提交後，StartUndoMark缺少，drawing_started=false；獨立核對645圖元完全不變、半徑仍30。2022：EDITED_UNSAVED後Strata重建快照查回Ø80；獨立逐圖元核對僅26C改半徑40，其餘不變、來源與副本磁碟雜湊不變。兩者都未重現過期revision或0/100迴圈；不代表公司原圖誤報已修復。
- 生成加權約27.75／28.70 tokens/s，模型總耗時156.207／189.945秒；2008曾只回文字，由runner要求修正；2022多讀一次cad-copy指南再讀cad-edits。完整最大輸入約1.4萬tokens，未做128K填滿驗證。
- 本機證據與助手在.build/strata-cad-52b90292a28c4086be28dae907776dea/、.build/strata_local_server.py、.build/strata-current.txt；新版轉接保留原始回覆，只對桌面fast/quality路由封套做映射。首輪原始封套不符、測試專案放在WebView資料目錄內造成sharing violation，皆已另記並排除正式流程速度。測試GUI和兩份私有圖面保留供看結果；2008未改、2022未存檔，不自動關閉。
- .codex/config.toml是使用者要求的本機MCP連線設定（未追蹤），不放進產品交付或Git；icon.png、dist/onboarding-state.json照舊保留。正式UI與量化比較細節見docs/VALIDATION_0_8_69.md的Strata補測節。未更換Strata模型或修改另一專案設定。

## 0.8.69 公司回報後診斷（2026-10-17）

- 使用者授權以法蘭需求補測本機2008／2022目前圖面 revision 與 Undo；本輪只測試及紀錄，未改產品、建置、進版或發布。
- 已存檔645／未儲存250圖元法蘭，連續三次與縮放後第四次快照都一致；另每次新啟正式worker三次也一致。公司首次版本拒絕未重現，合併錯誤不足以認定使用者動圖；需補差異診斷，不直接忽略revision。
- 2008仍缺StartUndoMark，兩種圖面都在寫入前停止；0.8.64只放行純縮放，不能把副本成功當作用中修改支援。原生LISP跨呼叫Undo備援未通過，不可直接採用。2022兩種圖面作用中Ø60→80成功，正式COM群組另以一次Undo實際還原半徑與顏色通過。詳見docs/VALIDATION_0_8_69.md補測節；公司DWG／原始快照、單位1與記憶校驗問題仍待查。

## 0.8.69：CAD 批次讀取整合與發行（2026-10-17 最新授權）

- 使用者已要求整合、測試成功後進版／v142 編譯／NSIS 打包並發布 Git main，取代下方原型及暫停限制。保留 icon.png、使用者 onboarding-state.json 與歷史離線 ZIP；不提交 .build、連線或聊天資料。
- 正式 worker 以固定且逐檔雜湊鎖定的 cad_export.lsp／cad_bulk.py 取得全圖資料。已確認類型直接轉成原有索引，傾斜／未知類型用 COM 詳讀；圖元數量不符時完整 COM 備援，不因逾時／格式錯誤重送。COM 物件依 Handle 延後取得。
- 首次讀取、修改前 revision、存檔前／重開後核對，以及新建圖面的診斷均整合批次流程；未指定圖元的原生資料雜湊另核對，避免只加速第一次讀取。修改仍需原本確認、圖層鎖及 Undo／副本規則。
- CAD 讀取／繪圖／修改最多 600 秒，輕量 inventory／probe 保留 240 秒；非模態讀取提示、實際計數、取消及 idle 清除保留。快速模型不傳圖片。
- 已實機通過兩版本 20051 圖元副本單圓修改與存檔重開，並獨立核對圓徑及來源雜湊。完整 645 圖元法蘭新建與單孔修改也通過：2008 讀取約13.3秒／修改49.3秒；2022 約7.1秒／38.2秒。圖元寫入本身仍較慢，不能把讀取改善當成整個繪圖時間。
- 使用 Build.ps1 -EmptyCargoCache -ValidateOnly 建立候選版做 Luna／GUI；實機／GUI 成功後 Build.ps1 -EmptyCargoCache -IncludeInstaller 完成完整核心與 NSIS 生命週期。待驗及最終結果見 docs/VALIDATION_0_8_69.md；不得沿用過期 runtime 或把未完成檢查說成通過。
- Luna low 真正 GUI 已通過 2008／2022 的 645 圖元查詢、預覽選孔、單孔副本修改與存檔重開；獨立 COM 核對孔徑與來源雜湊。2008 大圖讀取取消提示及 2022 快取 reused=true 均確認；模型仍曾填錯參數，修復紀錄須如實保留。非空 selected_candidate 表示已指認，不重問同一個選擇；寫入仍走既有確認，不因預覽跳過。
- 0.8.69 正式 Build.ps1 -EmptyCargoCache -IncludeInstaller 已以退出碼0完成；348 Rust、68 CAD、完整專案／Python、NSIS 安裝生命週期及安裝後 CAD／Python 自檢均通過。兩份更新清單一致且指向本版完整 NSIS；Office／SMB／VNC 選用實測未執行，歷史 ZIP 不重製。產物雜湊及範圍見本版驗證文件。

## CAD 程序內批次匯出原型（2026-10-17 最新授權與結果）

- 使用者恢復並授權先做固定內部匯出原型／效能測試，取代下方不得恢復原型測試的暫停限制。本輪未接入正式 Broker／runtime，未編譯、進版、更新 dist 或推送；原先提示／600 秒改動與未完成驗證仍保留。
- 新增 tools/cad_bulk/export.lsp、cad_bulk_probe.py 及大型合成圖建立助手。2008 使用已實測的 Application.LoadLISP；Document.SendCommand 不存在、EvaluateLispExpression 明確 E_NOTIMPL，不得誤用。2022 使用固定 Document.SendCommand 載入匯出器；兩者都以 nonce／完整結尾判定完成。
- 2008 Drawing5 實際 644 圖元（先前停止時只讀到 643），內部匯出約 1.28–1.38 秒；2022 的 645 圖元約 0.29 秒。各自與 COM 全部 Handle 集合一致，另抽樣核對屬性。
- 同一份新建合成 DWG（20000 線＋50 圓＋1 中文文字）：2008 匯出 36.90 秒、解析 0.24 秒；2022 匯出 7.63 秒、解析 0.52 秒。兩者全部幾何／中文逐一通過、原 DWG 雜湊不變。
- 這是指定 DXF 群組的唯讀原型，不能宣稱完整 CAD 屬性／真實建築圖／圖塊外參／OCS／正式修改流程已驗收。正式整合須同時處理首次快照與修改前 revision／驗證，不能只加速首次讀取就宣稱整個修改流程變快。
- 6 項匯出協定回歸通過；完整證據、限制及原型命令見 docs/VALIDATION_LUNA_CAD_INDEX.md 的 E 節。原型合成圖留在兩套 CAD 供查看，未關閉使用者原有文件。

## 0.8.68 新 CAD 流程 Luna 實測（2026-10-17）

- 使用已發行相同 EXE，真實 GUI → localhost → GPT-6 Luna low → CAD；此次只測試，未更動產品程式、編譯、封裝或推送。
- 2022 的 645 圖元查詢／預覽／選孔後查回通過，未出現 0／100 全圖分頁循環。快速模式未提供圖片工具與說明。
- 2022 自主修改測試因四次錯誤編輯參數及重複指認而未完成；成功後舊快照失敗仍留在 analysis 概況，是待修正的狀態不一致。
- 2008 的 643 圖元完整快照實際 240 秒逾時；九圖元五圓案例在要求先讀編輯方法後，Ø25→Ø15 的選取、另存與重開核對成功。另以原生唯讀比對確認只變更 Handle 15C 半徑、來源雜湊不變。
- 本機補齊缺少的 2008 HKCU COM 登錄；大型合成圖 Drawing5 未存檔，保留現場。細節、紀錄與限制見 docs/VALIDATION_LUNA_CAD_INDEX.md，不把小圖成功當成 2008 大圖驗收。

## 0.8.68 恢復發行（2026-10-16 最新授權）

- 使用者要求完成編譯、打包與推送，取代下方暫停限制。使用 Build.ps1 -EmptyCargoCache -IncludeInstaller，完成 v142 x64、離線依賴及必要驗證後交付 EXE、NSIS、更新清單與原始碼至既有 main。
- 已重建新 CAD runtime；保留 icon.png、使用者 onboarding-state.json 與歷史離線 ZIP，不提交測試連線或聊天資料。最終結果見 docs/VALIDATION_0_8_68.md，不沿用舊驗證冒充新功能驗收。
- 本次完整建置結束碼 0：實際 v142 14.29 x64、空快取／--frozen、345 Rust、61 CAD 合成、專案／Python 整合及 NSIS 安裝生命週期通過。已更新 EXE／NSIS／兩份簽署清單；新 CAD 的公司實機效率與模型核對仍待驗收。

## 0.8.68 CAD 本機索引與局部預覽（2026-10-16 實作階段）

- 使用者同意實作一次建立快照、按幾何條件查詢、穩定候選編號、局部預覽與修改前核對。維持暫停編譯／打包／推送，這輪只改原始碼與文件。
- 新增 cad_snapshot／cad_query／cad_preview，作用中圖面與專案 DWG 副本共用本機索引。完整快照不送模型、不恢復作用中文件寫入授權；查詢結果才按需交付，details=true 才加入已讀 Handle。
- 品質模型可取得局部 JPG 並另走 analyze_image；快速模型移除圖片工具／方法說明且執行端拒絕。兩者均可顯示使用者候選預覽；指認不等於寫入同意。
- 預覽為 WCS XY 幾何示意，不是真實 CAD 截圖；僅線、圓、可讀直線折線。矩形辨識、距邊計算與未知屬性限制見 docs/CAD_SNAPSHOT_QUERY.md。
- 本批僅格式整理與人工程式檢閱，未新增／執行測試，未編譯或實機驗證。先前 345 項測試與 Luna 實測不涵蓋本批。恢復建置時先 python tools/build_cad_runtime.py 更新鎖定 runtime，再按 v142 指引完整驗證。

## 0.8.68 暫停發行，先討論 CAD 按需查詢（2026-10-16，後續已授權實作）

- 使用者要求停止編譯，先討論一次讀取建立本機索引、依區域／圖元條件查詢，再決定是否併入本版；目前不實作這項新設計，不繼續打包或推送。
- 本次建置已停止；Rust 345 項、release 主程式、CAD 61 項與專案整合通過，Python 全套及 NSIS 尚未完成。dist／offline 產物未更新，未提交／推送。
- 原始碼暫定版本已改為 0.8.68；保留現有修改與 Git 暫存內容，最終驗證待設計決定後重跑。詳見 docs/VALIDATION_0_8_68.md。

## 0.8.68 發行（2026-10-16 先前授權，已由上方暫停要求取代）

- 使用者要求編譯、進版並推送 Git，取代先前暫不發行限制；進版 0.8.68，交付 EXE、NSIS、簽署清單與原始碼至既有 main。
- 使用 Build.ps1 -EmptyCargoCache -IncludeInstaller，以 v142 x64／--frozen 做完整核心與 NSIS 驗證。保留歷史離線 ZIP、icon.png 及使用者 onboarding-state.json；不納入測試連線檔／聊天資料。
- 本版 CAD 分頁實測無重讀，但每頁掃描成本、模型誤填版本及剖面屬性過度確認仍是已知限制。發行結果以 docs/VALIDATION_0_8_68.md 為準，不宣稱公司模型已完整驗收。

## localhost Luna 實測（2026-10-16 最新授權）

- 使用者授權 localhost 轉接到 Codex GPT-6 Luna low 並實際操作應用程式核對 CAD 分頁；允許為測試建立隔離 EXE，仍不進版、封裝或推送。
- 新增 Start-LocalLuna.ps1、原生 dynamicTools 轉接與 --local-ai-test／--local-cad-probe 隔離入口；只綁 127.0.0.1，使用獨立設定與合成資料。
- v142 x64／空快取 --frozen 基礎建置與 26 項 progress 測試通過；target 已更新，dist 未動。645 圖元法蘭圖已存檔重開核對。
- 兩次 GUI 真正模型／CAD 測試均成功讀完 645 圖元、offset 0→100→…→600，無分頁重讀。另補齊四個 CAD 工具漏掉的 progress_note，6 項 schema 測試及第二次完整基礎建置通過。
- 第二次需人工更正模型誤填的年份 version 為 ProgID；最後雖交付核對表，仍把未讀出的 Hatch 屬性稱為符合，答案正確性不通過。僅驗證分頁／筆記流程，詳見 docs/LOCAL_LUNA_TEST.md、docs/VALIDATION_LOCAL_LUNA.md；不宣稱完整 CAD 驗收。

## 待發行：CAD 0／100 分頁循環（2026-10-16）

- 使用者明確觀察 offset 在 0／100 間反覆；補上獨立 CAD 閱讀區間與固定提供模型的 cad_paging，重讀同頁不算新進展。
- 同文件同 revision 重讀首頁保留累積 Handle；文件／內容變更仍重設。詳見 docs/PENDING_CAD_PAGING.md。
- 沿用先改、不編譯不進版安排；新增 Rust 回歸未執行，未封裝推送，實機與模型效果待驗證。每頁全圖掃描效能問題本輪未修改。

## 待發行：只保留本版更新日誌與教學（2026-10-16 最新要求）

- 取代下方跨版本摘要修正：取消更新後自動公告、版本區間追蹤與 NSIS 舊版本轉換紀錄，只保留設定按鈕開啟的本版更新日誌對話框。
- 移除設定下方重複日誌及內嵌 0.8.65／0.8.66 公告；往後 release-notes.json 每次發行替換為本版內容，不累積歷史。完整開發紀錄留在文件。
- 保留全部教學與已讀紀錄，舊 JSON 的版本欄位忽略。先改、不編譯、不進版、不封裝推送；教學 DOM 測試通過，Rust／原生／NSIS 尚未驗證。見 docs/PENDING_ONBOARDING_UPDATE_RANGE.md。

## 待發行：教學公告與更新版本起點（2026-10-16，已由上方簡化方案取代）

- 最新要求先修改、不編譯不進版；本輪不接續打包或推送。
- 檢查 0.8.65～0.8.67 使用者摘要，移除開發用離線測試與內部技能拆分公告；14 張教學卡保留 ID／revision 與已讀紀錄。
- 升級重新設定公告起點，避免舊 pending_from 導致 0.8.65 → 0.8.67 顯示為 0.8.64；同版本已讀不重現。新增 Rust 案例未執行，詳見 docs/PENDING_ONBOARDING_UPDATE_RANGE.md。

## 0.8.67 完整補測進行中（2026-10-16）

- 使用者最新要求：CAD 測完後補跑之前略過的完整驗證，全部通過才打包及推送 main。沿用版本 0.8.67；本輪目前只修改測試，不改產品功能。
- 已修正工具數量、技能拆分、教學保存目錄隔離與四分頁測試；342 Rust、61 CAD 合成、完整專案／Python／前端／本機 VNC 通過。完整核心命令 `Build.ps1 -EmptyCargoCache -ValidateOnly -TestVnc` 結束碼 0；另一次 Python 全套含 Excel COM 通過。
- 尚有使用者的「文件1 - Word」開啟，已透過非同步問題請使用者儲存並關閉 Office，未收到答覆；不代為關閉。Test-Office.ps1 要求 Office 全部關閉，剩下全格式 Office／圖片、SMB 文件、NSIS 生命週期及發行待續。
- 本輪未更新 dist／簽署清單、未提交推送。詳細證據與待辦見 docs/VALIDATION_0_8_67_FULL.md；不要把核心通過宣稱全部驗收完成。保留 icon.png、dist/onboarding-state.json 與既有文件。

## 0.8.67：CAD 離線實機測試與相容修正

- 加入不需公司登入的固定離線測試入口，沿用正式 Broker／COM，測試後清理成功產生的 DWG，保留報告。
- 修正 2022 標準線型檔搜尋路徑；2008 縮放核對容許浮點末位差異，記錄尺寸文字首次可讀，仍拒絕實質變更。
- 缺少 COM 型別庫明示需修復安裝，不誤報模型參數錯誤。本機 2008 登錄經使用者允許後修復，不隨更新自動改登錄。
- 基礎驗證、NSIS及兩版本固定資料流程；外觀、公司AI服務、完整安裝生命週期仍待驗收。

## 0.8.66：解釋與教學目錄

- 一般對話新增「解釋」快速動作，沿用快速動作模型設定。
- 設定新增「教學與更新日誌」，14個短主題獨立記錄已讀，可單張或整組重看；更新日誌可重看。
- 第三輪CAD測試因超時暫停，未視為通過；不處理上游排隊問題。本版採v142 x64基礎驗證與NSIS，完整驗證待補。

使用者已授權本輪進版、編譯、NSIS及推送main；保留icon.png與歷史離線ZIP。

## 0.8.65：技能分組、跨任務精簡與初階教學

- CAD依新建／目前圖面／副本／Excel分開載入；Python、LOG、圖表與Outlook精簡入口、詳細方法按需讀取，共用說明去重。
- 跨任務舊答案改成摘要與查回參照，歷史／記憶共用5000估算token預算，保留近期使用者原文；空白length回覆保存進度並暫停自動重送。
- 登入後一張一般對話教學卡，設定可重看；EXE旁onboarding-state.json分開記錄教學與更新摘要，自本版開始列跨版本更新。
- 程式碼唯讀批次搜尋及Python靜態分析擴充；CAD同worker連線後執行計畫，引線文字錨點放在線上方。
- 使用者第三項CAD測試連續超時三次後手動停止，未視為通過；本版做v142 x64基礎驗證與NSIS，完整安裝生命週期及公司CAD／服務實測待補。

## 待發行：跨任務上下文預算

- 使用者同意將近期三輪的完整助理答案改為摘要與原文查回參照；近期使用者原文及本次要求不截斷。歷史／記憶共用5000估算token預算，使用者原文過大時明示例外，不刪限制。方法筆記去重，詳細結果按需read_task_result。
- 原生回覆length且沒有文字／工具時保存進度並暫停，不自動原樣重送；模型實際token數仍以服務端為準。見docs/PENDING_PROJECT_CONTEXT.md。
- 本輪僅修改與格式整理，未測試、未進版、未編譯、未打包推送；等待第三項CAD實測後再決定發行。

## 待發行：一般對話教學／更新摘要／引線文字

- 使用者要求一張一般對話入門卡、登入後顯示、可重看，EXE旁獨立教學旗標；新增跨版本簡短更新摘要，自0.8.65開始，不補舊公告。教學revision獨立於應用版本，未要求不自動新增教學。
- NSIS成功安裝記錄舊EXE版本，應用更新前保存目前版本；一般對話／設定／專案摘要依記錄分組，沒有可靠舊版不猜測。CAD引線MText固定左下錨點後放在線上方1mm。
- 本輪使用者明確要求先改、不進版編譯；全部未測試／未打包推送，仍等第三項CAD實測。見docs/PENDING_ONBOARDING.md。

## 待發行：CAD 開啟／重連

- 使用者回報任務中開啟／重連無效，需先開CAD再另起任務。已修正探測worker結束後第二個worker重取COM的流程，改同一程序連線並沿用app完成計畫；使用者重連後有界等待就緒，不重播部分繪圖。
- 實際失敗報告待提供，尚未證明全部原因或完成實機驗收。見docs/PENDING_CAD_CONNECTION.md；與程式碼分析修改先保留，未打包推送。

## 待確認：程式庫搜尋限制

- 使用者回報應為0.8.53；程式碼唯讀於0.8.54才加入，舊錯誤與舊白名單一致。本批擴充唯讀程式格式、search_files純程式碼每批100檔、run_python(kind=text)純程式碼32份及研究指南。保留單檔／合計上限、來源保護與不執行來源。見docs/PENDING_CODE_SEARCH.md；本批暫不進版編譯推送，驗證待補。

## 0.8.64：原生線型與目前圖面純縮放

- 新建線／圓／公式曲線可指定 line_style；既有線／圓／弧可修改原生 Linetype／LinetypeScale。沿用已有定義，缺少時查 CAD 安裝目錄標準 LIN 檔，不修改全圖 LTSCALE。線型外觀與舊版介面仍待公司實測。
- 目前圖面純 ZOOM E／A 不依賴 StartUndoMark；仍保留確認、文件／revision 與指令前後可讀內容核對，不儲存不關閉。圖元修改與混合操作仍要求完整 Undo 群組，不以縮放例外繞過。
- 公司實測 0.8.63 的程序身分備援及目前圖面讀取成功；其 ZOOM 被 Undo 前置檢查阻擋，不能視為縮放已驗收。
- 使用者已授權本版進版、v142 x64／空快取 --frozen 基礎驗證、NSIS 及推送 main；完整驗證與公司 2008／2022 實測待補。保留 icon.png 及歷史離線 ZIP。

## 待下次發行：CAD 線型

- 使用者要求先修改線型，同時實測 0.8.63。本批只改原始碼／文件，不進版、編譯、打包或推送，等實測回報後再合併發行。
- 支援新建線／圓／公式曲線的 line_style，以及已讀線／圓／弧的 linetype 修改；保留目前圖面確認、Undo、不儲存契約，不改全圖比例。細節與待驗項目見 docs/PENDING_CAD_LINETYPES.md。
- 0.8.63 公司實測目前圖面身分備援通過，但純 ZOOM 卡在缺 StartUndoMark。併入純縮放不要求圖元 Undo 群組的修正，保留文件／revision、確認及指令前後核對；實際圖元修改與混合計畫不放寬。本批仍暫不打包，實機縮放尚待測試。

## 0.8.63：目前文件程序身分備援與診斷

- 使用者已回報0.8.62在公司iCAD2008新建／儲存正常；目前文件唯讀成功，ZOOM被桌面身分檢查阻擋，尚未執行縮放。不能據此宣稱所有CAD或指令已驗收。
- 缺少COM HWND時，查詢Windows程序完整路徑與建立時間；只接受與CAD回報路徑相符的唯一可查詢程序，且有桌面視窗，再搭配唯一文件名稱／路徑、內容快照與執行中COM文件物件核對。多程序、缺權限、查詢逾時或文件歧義仍唯讀。備援不是文件專屬HWND，使用者確認與操作時勿切換文件仍必要。
- 程序查詢使用PROCESS_QUERY_LIMITED_INFORMATION；不提升CAD權限。Documents列舉每次重設，避免已用盡列舉器誤判文件不唯一。報告加入identity_reason，不把所有阻擋都說成使用者視窗問題。
- 縮放等全圖指令只需目前文件／revision，不需讀完100筆以上的所有圖元或整份工作紀錄。editable_session=false時直接回報原因，不呼叫cad_active_edit、不反覆回讀；使用者要求不儲存時不得自行改成副本存檔。
- 目前圖面仍經確認、Undo群組、不儲存不關閉；程序重啟／文件切換／快照變更須重讀。本版不調整AI推論連線重試，使用者紀錄中的五分鐘連線失敗另待服務診斷。
- 沿用v142 x64、空Cargo快取／--frozen、基礎驗證與NSIS／main交付；完整驗證與公司目前圖面指令實測待補。

## 0.8.62：舊版縮放與指令能力前置檢查

- 佑達 iCAD 2008 的 ZOOM E/A 改用 Document.ActiveViewport；GstarCAD 維持 Application 方法。方法存在不等於實機功能已驗收。
- validate_commands 與 prepare_annotation_styles 分離；能力檢查只查詢方法、不執行指令。報告區分 AVAILABLE／UNSUPPORTED／DEFERRED_NO_DOCUMENT，並明示 executed=false。
- 有現成文件時，在建立新圖前唯讀檢查指令。沒有文件則在新空圖建立後檢查；若不支援，保留空圖並允許移除該指令後另建，不接續修改空圖。既有圖面在 Undo／樣式／圖元修改之前檢查。
- 未執行指令的已知缺介面錯誤不一律 manual_check。新圖計畫確認含不支援指令後不可原樣重送；移除或更換後可重試。RPC／逾時等未知結果仍不放行。
- 沿用 v142 x64、空快取 --frozen、基礎驗證與 NSIS／main 交付；完整驗證與公司 CAD 實機待補。

## 0.8.61：引線／剖面修正與受限 CAD 指令

- iCAD 原生 AddLeader 的第二參數使用註解字串；追蹤本次同步建立、內容及端點相符的唯一文字，設定字高3、樣式與顏色，不另建重複文字。2022 保留相容介面。
- IntelliCAD SelectionSet.Delete 會刪除成員；暫存剖面選取集先 Clear 並核對 Count=0，再 Delete。清空失敗不刪除，保留圖面。
- 每次 COM 列舉重設 IEnumVARIANT；報告記錄逐操作新增／移除及預期 Handle。缺少圖元時帶出對應操作，不猜測其身分、不忽略集合差異。
- operations 可加入 {"kind":"command","command":"ZOOM E"}；支援 ZOOM E/EXTENTS、ZOOM A/ALL、REGEN、PURGE（或 -PURGE A * N）、AUDIT N，可有英文指令底線／點前綴。指令置於本批幾何操作後。
- 指令透過固定同步 COM 方法執行，未開放任意 SendCommand／LISP／Shell／載入程式／儲存／關閉／炸開。缺介面即停止。操作目前文件沿用確認、Undo與不儲存；副本沿用另存重開核對。
- PURGE 清理所有未使用定義（不僅圖層／圖塊）；報告列出圖塊、圖層、線型、文字／尺寸樣式差異，其他表格不在明細範圍。不是炸開圖塊，也不刪除使用中的幾何。
- 維護指令前後核對 ModelSpace／PaperSpace 可讀圖元內容，未驗證完整圖塊定義／不可讀屬性。AUDIT N 只檢查不修復，COM返回不能宣稱圖面沒有錯誤，詳細診斷請看 CAD。
- 請分別實測 2008／2022；合成測試不等於公司實機驗收。

## 2026-10-15：0.8.60 iCAD 尺寸樣式原生介面

- 使用者回報0.8.59在第9步建立尺寸樣式時缺少DimStyles，已有8個圖元但未儲存。舊版改用DimensionStyles.Add與DimTxt／DimAsz等原生屬性，無CopyFrom；新版相容介面維持。一般文字不依賴尺寸集合。
- 新圖在全部幾何之前準備樣式；目前圖面／副本在任何刪改之前準備，保留Undo與不儲存契約。樣式前置作業仍可能建立新空圖／樣式，失敗不能宣稱完全沒有寫入。
- 原生尺寸、原生引線保留；未知額外圖元不忽略、不重播部分圖面。實機仍需使用者測試。
- 沿用v142 x64、空快取／--frozen、基礎驗證、NSIS與main交付授權。README開頭最多保留三版；保留icon.png與歷史ZIP。

## 2026-10-15：0.8.59 CAD 目前圖面與標註相容修正

- 使用者回報 iCAD 2008 Document.HWND 不支援、額外一個圖元阻擋儲存及標註樣式忽略設定。沿用基礎驗證、v142 x64／空快取／--frozen、NSIS 與既有 main 交付授權。
- 缺文件 HWND 改探測應用視窗、唯一文件名稱／路徑、程序建立時間與快照；完全無可用視窗仍可唯讀，不授權目前圖面寫入。介面不支援錯誤在同任務禁止同版本重試，錯誤不呈現為讀取成功。
- 獨立樣式設定尺寸／引線字高3、箭頭3、ISO文字對正、偏移1、simplex.shx；沿用 BigFont 或尋找已安裝 chineset.shx，不保證中文字形。既有樣式不改寫，暫時尺寸變數還原。
- 2008／2022 保留原生引線並套用獨立樣式；只有 COM 明示關聯、此次新建的空白／同文字註解才納入預期集合。逐操作記錄新增圖元，未知額外圖元仍嚴格拒絕，原本 extra Handle 的實際類型尚待實機報告，不能宣稱已證明成因。
- 可讀尺寸／原生引線只開放刪除後重建；仍需已讀 Handle、未鎖圖層及目前圖面確認／Undo。不得任意刪除未知圖元。
- 保留 icon.png、歷史 ZIP 與共享環境；完整 CAD 實機及安裝生命週期驗證待補。

## 2026-10-15：0.8.58 目前圖面與 Excel 座標（最新授權）

- 使用者要求實作共用 CAD 文件操作與 Excel 座標繪圖兩階段；CAD 指令先不做。完成後進版、編譯並推送既有 main，沿用基礎驗證與 NSIS 交付。
- 目前文件讀取包含未儲存內容；寫入由原生視窗確認文件、固定身分／快照、每輪 Undo 群組。不儲存、不關閉，不恢復舊任務的目前文件授權；成功 EDITED_UNSAVED 不當作磁碟 DWG 成果。
- 共用副本／目前文件修改引擎，增加半徑、文字高度、簡單圖元刪除與既有全部新增操作。Excel 先原檔版本／欄位讀取，本機建立有界座標計畫，預覽後套用new/active/copy，來源與單位再核對。
- 仍需 v142 x64、空 Cargo 快取、--frozen、必要基本檢查與 NSIS。公司2008/2022 HWND、Undo、未儲存逐輪修改、Excel到CAD實機未驗收；不以合成測試代替。保留 icon.png、歷史 ZIP 與共享環境。

## 2026-10-14：0.8.57 CAD 連線與既有 DWG 副本（最新授權）

- 使用者確認 %%c10 在 iCAD 2008 可顯示直徑符號；同意一起實作連線對話框、圖元核對診斷及既有 DWG 限定副本閱讀／修改。
- cad_read 將來源鎖定複製後讀取 ModelSpace，本機索引分頁；cad_edit 僅修改已讀取 Handle、來源 revision 一致的全新副本。支援文字／ACI／位移與新增線圓文字引線；圖塊僅名稱位置屬性摘要，不展開、重構、OCR 或修改巢狀物件，不接管使用者已開文件。
- CAD 連線由原生待答事件處理開啟／重試／取消，期限十分鐘；已部分操作斷線須由使用者明確選擇另建副本，不從未知位置接續。原檔不交給 COM，任務恢復仍核對來源及索引。
- %%c 對數字前的直徑符號正規化；Count 不一致只有完整 Handle 集合／屬性吻合才可接受並記錄，未知額外圖元不忽略。
- 沿用測試版 v142 x64、空快取／--frozen、基礎驗證與 NSIS／既有 main 交付，完整驗證及公司 CAD 實機待補。不得把合成 COM 測試稱為實機驗收。保留 icon.png、歷史離線 ZIP。

## 2026-10-14：0.8.56 CAD 復原與佑達 iCAD 修正

- 使用者實測 2008 已成功連線、矩形／圓／尺寸，第一條引線因 Points.Item 索引失敗；修正為 Points.Add(x,y,z)，不宣稱此修正已完成公司實測。
- 尚未建立圖面可補正重試；已回覆 E_INVALIDARG 的部分操作可修改計畫另建圖面，不修改／關閉部分文件。相同部分執行計畫不能換檔名重播，每任務最多六次執行，未知結果仍鎖定；舊版未知狀態不能自動解除。
- 預定路徑不是磁碟成果。file_exists、saved、completed_operations 與交付驗證分開；更新模型說明避免要求使用者開啟不存在的 DWG。
- 支援產品明示浩辰 GstarCAD／佑達 iCAD，保留已知身份及實際路徑核對，不限制安裝資料夾品牌名。
- 使用者授權跳過完整驗證、v142 x64 基礎編譯與 NSIS 打包後推送既有 GitHub you1230783-sys/CompanyAI main，包含原始碼、EXE／NSIS、驗證文件與 LFS。保留 icon.png、歷史離線 ZIP。

## 2026-10-14：0.8.55 AI CAD 繪圖測試版

- 使用者確認獨立測試的 2022 引線箭頭正常，要求直接整合專案 AI 對話繪圖並加入顏色；進版、v142 x64 編譯與 NSIS 打包，只做基礎驗證，完整驗證延後。
- 新增 cad-drawing 技能、cad_inventory 與 cad_draw。只新建 DWG，修改需求重建另一份成果；不操作使用者目前文件、不做 Solid Edge。年份新版優先，不明年份不猜測，失敗不偷換版本。
- 支援線、圓、尺寸、文字、多行文字、帶箭頭引線、ANSI31 剖面線、受限數學公式取樣線段，以及每項 ACI 1–255 顏色。新增的顏色與應用整合尚待公司實機驗收，不把獨立工具的成功擴大為新版全部功能已通過。
- CAD 工作程序採獨立鎖定 CPython／pywin32，固定 JSON 操作，不執行模型 Python／任意 COM。原分析 Python 與 AppContainer 不變。上游預編譯品不宣稱為本機 v142 編譯。
- 以 Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller 交付；新增 CAD runtime 完整性與唯讀管線自檢。完整 Office／SMB／NSIS 安裝回復驗證延後。保留 icon.png、歷史離線包與既有 Git 遠端；交付後使用者追加授權推送既有 main。
- 詳見 docs/CAD_DRAWING.md 與 docs/VALIDATION_0_8_55.md。
- 2026-10-14 19:48 基礎交付完成：v142 x64／空快取／--frozen、41 項選定 Rust 測試、10 項 CAD 合成契約、正式及 dist CAD runtime／唯讀管線、既有基礎 UI／Python／程式唯讀、NSIS 打包與簽署清單核對通過。公司 CAD 繪圖、AI 實際規劃及完整安裝驗證仍待實測；交付後追加授權提交／推送。

## 2026-10-14：CAD 0.1.1 實機回報修正

- 使用者回報佑達 ICAD 2008 已連接，繪圖在產品身份檢查停止；只更新獨立工具身份規則與 ZIP，沿用 Git 交付及基礎自檢範圍。CompanyAI 0.8.54 EXE／NSIS 不變，不重編 Rust。實際繪圖仍待使用者重測。

## 2026-10-14：0.8.54 程式碼唯讀與獨立 CAD 測試（最新授權）

- 使用者要求開放專案程式碼唯讀，並交付浩辰 CAD 2008／2022 的獨立診斷、新建圖面及尺寸標註測試。預設已確認年份由新到舊（2027 > 2026 > … > 2022 > … > 2008），保留手動選擇；Solid Edge 不做。
- 完成後進版、v142 x64 編譯、NSIS 打包與推送既有 Git main；本輪只做基礎驗證，完整驗證等待使用者實測。使用 scripts/Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller；必須核對實際編譯器、空 Cargo 快取與 --frozen。
- 新程式格式只有閱讀能力；PY 保留既有副本編輯。一般聊天附件上傳契約不變。CAD 工具獨立打包，不接入 AI，不改分析用 Python；上游 Python／pywin32 預編譯品不能宣稱本機 v142 編譯。實機 CAD 成功須以公司報告為準。
- 保留 icon.png、共用環境及歷史離線 ZIP。詳見 docs/VALIDATION_0_8_54.md。
- 2026-10-14 基礎交付通過：v142 x64／空 Cargo 快取／--frozen、25 項選定測試、原生唯讀流程與 NSIS 簽署清單核對；CAD 32／64 位元本機自檢通過，實際 2008／2022 尚未測試。完整驗證暫緩。

# 專案級開發指引

## 2026-10-14：0.8.53 補做完整驗證（最新授權）

- 使用者回報測試 OK，要求接續完整驗證；取代下方暫緩完整驗證的安排。維持 0.8.53，使用既有 `scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller`，指定 v142 x64 並核對實際編譯器。
- 補跑完整 workspace、專案／Python 隔離與長流程、Office／本機 SMB，以及 NSIS 安裝／更新／回復／解除安裝。依實際結果更新驗證紀錄；使用者的「測試 OK」不擴大解讀為所有公司環境與 PDF 格式均已驗收。
- 沿用既有 Git main 推送授權；保留 icon.png、共享環境、Python runtime 指紋及歷史離線 ZIP。
- 2026-10-14 16:05 完整流程通過：v142 x64／空 Cargo 快取／--frozen、326 項單元測試、原生與長任務／Python／Office／本機 SMB、正式 NSIS 安裝更新回復解除安裝。前端 8 組圖表與 31 項分析／比較重跑通過。沒有修改功能程式碼，EXE／NSIS 雜湊與使用者測試包完全相同，維持 0.8.53；詳見 docs/VALIDATION_0_8_53.md。

## 2026-10-14：0.8.53 基礎驗證測試包（最新授權）

- 使用者提供網站調查：疑似POST413在建任務前拒絕，桌面續查by-request才衍生404；415網站已放寬PDF開頭判斷，實際加密封裝原因仍未確認。
- 已改為授權進版0.8.53、編譯及NSIS打包，僅做基礎驗證，等使用者公司測試後再補完整驗證；取代下方不編譯安排。建置完成後使用者追加授權推送既有Git main，包含原始碼、產物與基礎驗證紀錄。
- 使用 `scripts/Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller`，仍必須指定v142 x64、實際編譯器探針及空Cargo快取／--frozen。執行fmt／Clippy、聊天及附件相關測試、release、WebView2／原生任務停止自檢、Python runtime必要檢查、NSIS打包及簽署清單核對。完整Office／SMB／專案／Python長流程及NSIS安裝更新回復解除安裝待補。
- 明確413等拒絕直接保留原HTTP錯誤並停止；POST等待中不先查任務。傳輸結果未知或408／未明示拒絕的5xx保留原ID查回，再受三次相同錯誤＋models健康條件限制，不自動重送。
- 報告標示PASS_BASIC與完整驗證未完成，不沿用舊版完整驗收聲明。保留icon.png、共享環境、Python runtime指紋及歷史離線ZIP。
- 2026-10-14 14:52 基礎交付完成：v142 14.29.30133 x64／_MSC_FULL_VER=192930159、空Cargo快取／--frozen、fmt／Clippy、21項聊天附件測試、正式EXE及5組原生任務追蹤自檢、Python runtime及NSIS更新清單簽章核對通過。圖表8組與分析／比較31項前端檢查通過；完整驗證與公司端實測待補；後續依追加授權提交並推送Git。產物及雜湊見docs/VALIDATION_0_8_53.md。

## 2026-10-14：任務停止追蹤與介面整理（待合併發行）

- 使用者要求先修改程式碼，等網頁端調查 PDF 415／任務404後再合併處理。本輪不編譯、不進版、不打包、不提交或推送，保留0.8.52既有產物及更新清單。
- 一般對話任務 GET 連續三次相同錯誤，且當輪模型清單回200及有效內容時，自動停止本機追蹤。依實際HTTP狀態計數；無HTTP狀態則比對本機錯誤。成功、改用遠端任務ID或手動重送歸零。停止保存已收文字及DPAPI停止狀態，不另查詢／取消／重送未知工作。正式failed／cancelled仍立即收尾。
- 「分析依據與處理範圍」可摺疊，答案完成預設收起；同對話重繪保留手動展開。圖表移除上方文字清單及編輯文字區塊，工具列保留新增文字，圖內雙擊編輯／刪除。
- 可執行不需編譯的前端互動與Rust格式檢查；新增Rust測試留待後續指定v142 x64建置執行。詳見docs/PENDING_TASK_TRACKING_AND_UI.md，不把瀏覽器檢查當作原生／公司端驗收。

## 2026-10-12：Outlook Table 原生排列修正（目前階段）

- 使用者回報0.8.52仍出現「Outlook Table批次大小不符」，要求先交付可立即執行的小程式，回傳實機排列後再決定修正，避免反覆完整編譯。
- 已收到公司實機報告：Outlook16.0.0.17932、64位元、3677列；原生SAFEARRAY第一維為列、第二維為欄；GetArray(1/100)分別為1/100×5及1/100×9，ConversationID不支援、必要欄數正確。
- 依實測修正原生維度與GetElement索引，維持每批100列；補上真實方向、正方形批次、非零下界與異常尺寸的回歸測試。錯誤列出實際尺寸，不以寬鬆裁切掩蓋欄位錯誤。
- 使用者最新要求跳過剩餘驗證、直接發布NSIS。已完成v142實際編譯器探針、空Cargo快取／--frozen、fmt／Clippy、319項單元測試、release與部分整合檢查；停止未完成的長流程，不再重跑Office／SMB／完整安裝驗證。維持0.8.52，打包NSIS、核對簽署清單並推送既有Git main。
- 此次交付不能宣稱完整驗證通過；先前13:59驗證紀錄對應先前產物。scripts/Test-OutlookTable.ps1保留為唯讀診斷工具。修正版在公司Outlook的完整讀取結果仍待實測，不把尺寸診斷當作整體功能驗收。

## 0.8.52：分析交付、內容比較與方法重用（最新授權）

- 使用者已要求合併 Outlook ConversationID 修正與五項優化，重新進版0.8.52、完整編譯、NSIS打包並推送既有Git main；取代下方等待Python實測的暫停安排。
- 五項為：原生處理完成摘要、可查原文的LOG事件時間表、唯讀左右紅綠內容比較、可選用及調整的方法重用、文件搜尋範圍與原文定位。沿用既有來源版本、DPAPI、副本及權限。實際Office頁面排版預覽、逐項接受／拒絕及通用CLI不在本版。
- 比較固定建立副本時的基準與選定成果，舊檔無基準不補造；格式／結構變更另列。方法重用須重新核對新來源，不直接重播舊程式。
- 使用 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller，指定v142 x64、空Cargo快取／--frozen及完整安裝驗證通過才發布。保留icon.png、共用環境、Python runtime及歷史離線ZIP。

- 2026-10-12 13:59完整驗證通過：MSVC14.29.30133／_MSC_FULL_VER=192930159 x64、空Cargo快取／--frozen、314項單元測試、20項預覽互動與三種版面、實際來源／固定副本比較、Python／Office／SMB與完整NSIS安裝更新回復解除安裝。首次安裝自檢暴露的測試草稿防抖殘留已修正，保留閱讀位置斷言並完整重跑通過。交付雜湊與公司實測界線見docs/VALIDATION_0_8_52.md。

## 2026-10-12：等待 Python 實測的前置階段（已由0.8.52發行授權取代）

- 使用者仍在測試 Python，要求等待測試結果後再一起進版、編譯及推送；目前只保留原始碼、測試與文件修正，不繼續編譯、不打包、不發布。
- Outlook ConversationID 欄位相容性修正已保留；本輪格式、Clippy及307項單元測試通過。停止前 release 編譯已產生 target 內測試用 EXE，後續驗證依使用者指示中止，完整建置尚未完成。
- 保留0.8.51的 dist、更新清單、歷史離線ZIP、icon.png及共用環境。後續收到合併發行指示再依 v142 x64、空 Cargo 快取／--frozen與必要 Office／SMB／NSIS流程驗證。

## 0.8.51：Outlook 日期批次與程式工作資料（最新授權）

- 使用者已確認以完整主旨與寄送時間精確到秒判定隱藏副本，接受兩欄完全相同時排除；不讀寄收件者地址簿。優先日期篩選＋Table每批100列，備援排序批次；可見收件依收件日篩選，隱藏索引依候選寄送日，避免跨月遺漏。
- 專案Outlook輔助程序可被取消／10分鐘期限終止，完成排除才交付模型；錯誤不冒充零封。舊版郵件介面依候選寄送日延後建立索引。公司Outlook實際速度仍待實測。
- 通用Python流程保留真實副本／版本／區段原文與最小測試介面，section_id由程式管理；每段保存相同草稿。不得針對測試下載器特製修改，也不將模型筆記當成完成證據。
- 本版授權編譯進版、完整NSIS與Git main推送；依原有v142 x64、空Cargo快取／--frozen、Office／SMB與完整安裝驗證。保留icon.png、共用環境、Python runtime與歷史離線ZIP。

- 2026-10-09 17:42完整發行驗證通過：v142 14.29.30133／_MSC_FULL_VER=192930159 x64、空Cargo快取／--frozen、305項單元測試、Outlook批次／取消逾時、實際Python副本與Office／SMB、完整NSIS安裝更新回復解除安裝。產物與公司實測界線見docs/VALIDATION_0_8_51.md。

## 0.8.50：程式需求驗收與 Outlook 四欄位副本比對（最新授權）

- 使用者要求改善通用程式修改流程，不針對範例下載器特製修補。先保存需求，分段修改同一草稿，最後核對真正副本與需求；Python 可用標準 unittest 在既有 AppContainer 執行實際副本。語法、模型核對、功能測試、模擬依賴與未驗證項目分開記錄，證據綁定 revision。
- Outlook 移除 Message-ID 讀取，使用主旨、寄送時間、寄件地址、To/CC 地址集合的本機 SHA256 指紋。使用者接受四欄位恰巧相同時視為副本；不讀隱藏正文。部分欄位缺失時只隔離不能排除關聯的候選，明確無關的郵件仍可讀；完全不可辨識或索引未掃完須明示查詢失敗、數量未知。
- 使用者已授權編譯進版、完整 NSIS、原始碼及產物推送既有 Git main。使用 v142 x64、空 Cargo 快取、--frozen、Office／SMB 與完整安裝驗證；不改 icon.png、共享環境、Python runtime 指紋或歷史離線 ZIP。
- 其他語言先不加入，待本版 Python 的公司實測回饋；資料結構可重用，不預先聲稱模型品質或公司 Outlook 已通過。
- 2026-10-09 15:18 完整發行驗證通過：v142 14.29.30133／_MSC_FULL_VER=192930159 x64、空 Cargo 快取／--frozen、299 項單元測試、實際副本功能驗收、Python／Office／SMB 與完整 NSIS 安裝更新回復解除安裝。產物與公司實測界線見 docs/VALIDATION_0_8_50.md。

## 0.8.49：七項實測修正與有界並行（最新授權）

- 使用者要求隱藏 Outlook 資料夾中的副本也從其他資料夾排除、圖中文字雙擊編輯與對話框刪除、讀信後保存可寫入草稿、圖片本機轉 JPEG／100 張／雙圖合計 8 MB、VNC 單分類排序、修正活動紀錄與手動停止的 Error Code 99、Python 分段修改即保存同一草稿。
- 使用者追加獨立工具／兩個 Python 分析可並行，修改同一副本依序提交；不能以模型筆記假稱批次執行。草稿與語法通過的完成成果須清楚區分，保留原件及來源版本。
- 本版完成後直接進版、完整 NSIS 與既有 Git main 推送；仍使用 v142 x64、空 Cargo 快取／--frozen、Office／SMB 與完整安裝驗證。不改 icon.png、共用環境及歷史離線 ZIP。
- 2026-10-09 11:54完整發行驗證通過：v142 14.29.30133／_MSC_FULL_VER=192930159 x64、297項單元測試、26個原生／9個恢復／13個圖片案例、兩組68輪長任務、兩個真實Python程序重疊約8秒、持續草稿／隔離／逾時、Office／SMB與完整NSIS安裝更新回復解除安裝。產物與界線見docs/VALIDATION_0_8_49.md。

## 0.8.48：圖中文字、AI圖表編輯與本機時間（最新授權）

- 使用者要求連同下方待發行配色／排版完成編譯、進版、完整NSIS與既有Git main部署，取代只修改原始碼的限制。
- 自訂排版加入可拖曳文字，支援粗體／斜體／底線、字型、大小、字色與底色；畫面、加密歷史及PNG共用資料。AI使用相同受控圖表設定，不接受腳本或修改原始點陣。
- 使用者訊息及最終回覆顯示本機日期時間；輪次／工具活動顯示HH:MM:SS，時間於事件發生時保存，舊資料不捏造時間。
- 使用者追加Python原始碼編輯試用：開放.py文字副本，以內建Python固定檢查器AST／compile但不執行原文；目前版本語法通過後才發布副本。區分語法與功能測試，不需另增依賴，Python worker／runtime可維持原指紋。
- 完整發行仍以v142 x64、空Cargo快取／--frozen、Office／SMB與NSIS驗證；不改icon.png、Python runtime或歷史離線ZIP。
- 2026-10-09 08:35完整驗證通過：v142 14.29.30133／_MSC_FULL_VER=192930159 x64、288項單元測試、26個原生／9個恢復／12個圖片案例、兩組68輪長任務、Python編輯／隔離／逾時、Office／SMB及完整NSIS安裝更新回復解除安裝。正式產物與雜湊見docs/VALIDATION_0_8_48.md。

## 圖表文字排版與預設配色（併入0.8.48）

- 前置階段當時只修改原始碼與文件；現已依上方0.8.48授權一併編譯、驗證及發行。
- 參考線標籤自動避讓，提供明確的排版模式拖曳標題、圖例及參考線文字；不移動數據點或改變參考線數值。保留完成、取消與單獨重設排版。
- 一般設定加入八個系列預設色票；已自訂的圖表顏色優先。排版比例位置須隨歷史保存，縮放及PNG共用規則，舊設定缺欄位時相容。
- 執行不需編譯的JS、瀏覽器互動與格式檢查；新增Rust檢查待下次v142建置，不宣稱原生驗證完成。不改icon.png與Python runtime。

## 0.8.47：郵件成果筆記、階段與座標範圍（最新發行授權）

- 使用者已要求將本輪與上一批待發行修改一起實作、編譯進版及部署既有 Git main，取代下方不編譯／不發布安排。需完整 NSIS。
- 郵件閱讀後下一輪同時保存來源摘要／條目草稿，準備寫入時載入選定條目；階段控制技能並允許有理由的定向回讀。沿用 DPAPI／.lmai、來源版本與閱讀範圍，不把模型摘要當已驗證事實、不以改寫筆記假造進展。
- 量測軸以2倍標準差留白，限制在跨度5%～40%，依跨度選1/2/5刻度向外取整；保留全部可見有效點、手動範圍、常數備援及長條零基準。
- 使用 scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot "\\localhost\Y$\Rust\Project\CompanyAI\.build" -IncludeInstaller，確認v142 x64與實際cl版本、空Cargo快取／--frozen、完整驗證後才發布。
- 不改icon.png、Python worker／runtime指紋或歷史Rust離線ZIP。
- 2026-10-08 完整發行驗證通過：MSVC14.29.30133／_MSC_FULL_VER=192930159 x64、空Cargo快取／--frozen、284項單元測試、26個原生／9個恢復／12個圖片案例、兩組68輪長任務、Python／Office／SMB及正式NSIS安裝更新回復解除安裝。交付雜湊、設定／圖表檢查及公司實測界線見 docs/VALIDATION_0_8_47.md。


## 前置階段：參考線、設定、錯誤診斷與日期（已併入上方0.8.47發行）

- 此前置階段當時僅修改原始碼及文件，暫不編譯、不進版、不製作 NSIS 或發布；保留 0.8.46 產物與清單，後續已依上方授權一起驗證發行。
- AI 可設定圖表參考線；多時段未指定排版時使用非阻塞偏好，提供同圖／分圖及其他輸入，明確預設並保留原資料。
- 設定分一般／操作／進階，VNC 與診斷放進階。診斷須能選歷史回合、匯出錯誤，與含正文 JSON／純用量保持分離；錯誤提供固定代碼與文件。
- 查核今天錯到下週的問題，使用 Windows 本機日期作明確依據，區分目前日期、任務起始日期與使用者指定分析日期，不改寫已提交請求。
- 本輪僅執行不需編譯的格式、JS、來源頁面檢查；新增 Rust 測試留待下一次 v142 完整建置，不能宣稱原生驗收通過。
- 保留估算係數 0.48 與 8K–12K 軟目標，先觀察公司長任務；不修改 icon.png。

## 0.8.46：工作筆記、動態技能與非阻塞偏好（最新指示）

- 使用者已要求完成本輪所有討論功能、完整NSIS並部署既有Git main。
- 每輪累積筆記＋最近1–3組工具結果，原始要求／授權／來源與精確資料可查回；完整操作留DPAPI／.lmai。8K–12K是軟目標，不能以token預算直接拒絕任務。技能只保留目前階段的核心說明／工具，完整教學按頁讀取。
- Outlook分開新增資料與有限的舊代號恢復，空字串游標視為首頁，舊ID用outlook_index查回；不可用隨機快照或筆記假造進展。read_work_log索引按筆數固定分頁，原文按字元分頁。
- DEBUG位於進階設定，純用量報表不含正文、檔名或工具參數，與JSON有各自複製按鈕。初始估算係數0.48來自使用者四筆實際API樣本，需明示誤差與服務端usage缺漏，不能當真正tokenizer。
- 可選偏好不阻塞工作，完成前依事先公告的預設。Y空白／異常預設保留缺值，原值明細與索引保留供圖表編輯切換；無效X不默默排除整列。授權與關鍵資料定義仍需明確回答。
- 一般聊天與重試無20輪上限，桌面只送本次問題，網站負責歷史；不可因此截斷Outlook助理的專用請求。回答重點中的分列／可核對單行Markdown表格只修復顯示，不改原payload。
- 發行仍用 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller，固定v142 x64、空快取、--frozen；不改icon.png與歷史Rust離線ZIP。實際模型的速度、token與長任務品質需公司回饋，不以合成測試宣稱達標。
- 2026-10-08 完整發行驗證通過：MSVC14.29.30133／_MSC_FULL_VER=192930159 x64、275項單元測試、26個原生／9個恢復／12個圖片案例、兩組68輪長任務、Python／Office／SMB與正式NSIS安裝更新解除安裝。最終產物、雜湊、DEBUG用法及實測界線見 docs/VALIDATION_0_8_46.md。

## 0.8.45：本機預設專案、上游恢復與實際進度（最新指示）

- 使用者已授權把本批與下方待發行分析功能一起進版、完整 NSIS 編譯驗證並部署既有 Git main，取代待發行段落的不發布安排。
- 預設改為 Windows 目前使用者個人資料夾的 LM_AI_Projects/LM_AI專案資料夾_YYYYMMDD，同日加序號；移除桌面／下載選擇，保留自選資料夾及 UNC／映射磁碟。確認預設為本機固定磁碟，不拼帳號路徑。
- 專案主推論及子請求新增10／30／60／180／300秒重試，耗盡保存並暫停，可手動繼續。結果未知只查原ID；僅已核對的 failed、result=null、限定暫時上游錯誤可另建一次推論。使用者回報的 AI_BACKEND_ERROR 納入有限重試，即使舊網站標 retryable=false；登入／權限／額度／格式／身分／結果未知錯誤不因此放行。這是新增的明確桌面政策，須同步更新契約；不重播本機工具。
- 無進展依實際新資料／閱讀範圍，Outlook 保存查詢與下一頁；四次提醒，八次暫停。筆記、操作ID或游標隨機碼不能假造進度；網路重試與工具5／10次分開。
- 使用者追加移除專案「圖片辨識」快捷按鈕、選圖視窗及獨立 image-read 技能；analyze_image 併入基本檔案閱讀，AI 按需直接使用。保留模型能力／大小／來源／快取／未知提交保護；Outlook 助理及一般聊天附件不擴大。舊 checkpoint 的 image-read 僅作空相容別名，不再公告。
- 發行使用 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller，固定v142 x64、空Cargo快取、--frozen；不改icon.png，不重製歷史Rust離線ZIP。
- 2026-10-07 本版完整發行驗證通過：MSVC14.29.30133／_MSC_FULL_VER=192930159 x64、263項單元測試、25個原生／9個恢復／12個圖片案例、Python／Office／SMB及正式NSIS安裝更新解除安裝。最終產物及雜湊見 docs/VALIDATION_0_8_45.md；公司F槽及真實模型效果仍待公司實測。

## 已納入0.8.45：分析依據、方法記憶與映射磁碟

- 使用者同意五項分析改善：原生範圍紀錄、可展開工具證據、工具結果筆數核對、project 範圍方法記憶、目標／待確認／失效結論。沿用既有工具、DPAPI 記憶與 checkpoint；不把 AI 宣稱當程式驗證。
- 使用者回報 F 槽子資料夾出現「無法核對文件的實際位置」，UNC 可用；映射磁碟以目前工作階段 Windows API 解析 UNC，正規化 handle 路徑提供 NT 名稱備援，不放寬 .lmai／連結／路徑界線。相同錯誤每次重試均重新顯示。
- 前置階段先修改與完整驗證，當時保留 0.8.44 dist／更新清單；後續依上方授權合併0.8.45發行。前置驗證使用 Build.ps1 -ValidateOnly -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build'，仍需 v142 x64、空快取與 --frozen。本機映射測試不得冒稱公司 F 槽已驗收。
- 2026-10-07 前置完整驗證通過：MSVC 14.29.30133／_MSC_FULL_VER=192930159 x64、259 項單元測試、原生／舊版對話與續接、Python／Office／UNC／映射磁碟。新增案例實跑 Big5 分段、超過77 MiB LOG、證據與數量核對、方法去重及跨對話查回；WebView2 重複通知與分析介面也通過。紀錄在 docs/DEVELOPMENT.md 與 .build/analysis-validation-final.log；當時尚未進版，正式0.8.45另依上方流程重新驗證及打包。

## 0.8.44：圖表編輯與失敗預算（最新使用者指示）

- 使用者授權完成七項修改後進版、完整 NSIS 並部署既有 Git main。所有對話框按鈕加底色；圖表編輯分基本設定／座標轉換／顏色與參考線三頁，加入就地說明。
- X／Y 各有自動範圍；使用者已確認依數值尺度向外取整，例如12～23為10～25、0.12～0.23為0.10～0.25。類別軸取完整位置；空白參考線文字不補數字；滑鼠提示同時顯示實體X／Y。
- Python 與其他工具的失敗獨立計數，上限10／5；同類成功才歸零，已辨識原生工具的非嚴格參數錯誤也納入。失敗不重複扣8次無進展額度；未知提交、身分、權限及無法辨識的回覆保護不放寬。
- 使用完整 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller，遵守 v142 x64、空快取、--frozen。Python worker／清單不變，沿用既有安裝最佳化；不改 icon.png，不重製歷史 Rust 離線 ZIP。

## 0.8.43：座標轉換、圖片支援與 Python 沿用（最新使用者指示）

- 使用者已要求本批完成後進版並部署既有 Git main，取代下方待發行段落的暫不發布限制。納入三張異常值示意、X／Y平移及重新編號、全系列無值位置移除、Python安裝沿用及專案圖片一般支援。
- AI 增加 transform_chart，沿用原 CSV／Excel 欄位核對後衍生座標；圖表編輯器也可直接調整。Python 技能增加主動選用時機，固定閱讀／作圖仍走專用工具，不強迫每次使用 Python。
- Python LOG／OUT／ERR 無 BOM 先嚴格 Big5（CP950）、失敗再 UTF-8；BOM 優先，回報編碼／歧義並提供 encoding 明確覆寫。修正原生 UTF-8 前置阻擋；worker／套件指紋維持不變。工具參數拒絕加入欄位路徑診斷，不猜參數執行。使用者後續討論待辦系統，本版保留現有累積筆記，不另加待辦流程。
- 正式驗證用 scripts/Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot '\\localhost\Y$\Rust\Project\CompanyAI\.build' -IncludeInstaller；v142 x64、空快取及--frozen不變。發布 EXE、完整 NSIS、兩份NSIS簽署清單、驗證文件及原始碼，不重製歷史離線ZIP、不改icon.png。

## 待下次發行：Python 更新沿用與圖片附件（最新使用者指示）

- 使用者目前仍在公司測試 LOG／Excel 繪圖；本輪先修改及驗證，不進版、不發布或推送，保留 0.8.42 的 dist、簽署清單與發行驗證紀錄。
- NSIS 使用新版待安裝 EXE 的 Python 完整自檢，通過就原地沿用現有環境；不符時完整替換及失敗回復，不只比對版本字串、不逐檔混補套件。Python worker 暫仍納入同一環境指紋。
- 使用者已澄清圖片支援指「專案內的圖片檔」；移除試驗定位，與其他專案資料一起使用，AI 按需求決定是否讀取。Outlook 助理及一般聊天不改動，不新增郵件附件讀取。
- 圖片單次仍一張／5,000,000 bytes；每任務改為最多20次不同辨識要求，相同來源與焦點完成結果重用。只送當次子請求，後續保留文字重點，fast 不支援；保留既有待查請求與快取識別碼供續接。
- 異常 Y 值選擇視窗加入三張固定五點折線示意，並列保留缺值／略過此點／設為0；標明假設資料、共用座標及原 X 位置，不改變既有選擇或原始數據。只有無效 X 的視窗不顯示這組 Y 值示意。
- 使用者後續要求 X／Y 均支援平移、重新編號及移除無值位置；AI 與完成圖表編輯器共用受控轉換。保留 CSV／Excel 來源欄位核對，另以 transform_chart 衍生座標，不手抄原數列。先移除全系列無值列、再用共同列序號編號，0 保留；實體 X／Y 隨水平長條正確對應。原始點陣、明細、來源保留；PNG 依圖表轉換版本去重，舊 PNG 不覆寫。工具目錄增為49項。
- 測試建置只留 target／.build；Rust／C++ 沿用 v142 x64，完整原始碼驗證使用 Build.ps1 -ValidateOnly -EmptyCargoCache，安裝回歸可用 Test-Installer.ps1 -ValidateOnly。待 LOG／Excel 測試問題收齊後再一起進版。

## 0.8.42：獨立 Python 與完整 NSIS（最新使用者指示）

- 使用者明確授權加入離線 Python 分析、製作完整 NSIS 並部署既有 Git main；取代先前不製作 NSIS 的限制。不改 icon.png、不開放 PowerShell、不移除 Outlook 助理。
- 本版固定 CPython 3.13.12 x64、pandas 2.2.3、NumPy 2.2.6、openpyxl 3.1.5 及鎖定的必要依賴。安裝在 EXE 旁 python 子目錄，不嵌入 EXE、不使用系統 Python／pip／網路下載。
- Python 在單次 AppContainer／Job 執行，原檔由 Rust 唯讀建立快照；公司原始 Excel 仍以既有 COM 匯出資料集交給 pandas，不引入 pywin32。生成 XLSX 可用 openpyxl。
- 原生程式與 C++ 測試必須 v142 x64；上游預編譯 Python／wheel 的來源與編譯器需另據實記錄，不冒稱由 v142 編譯。
- 完整驗證用 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot -IncludeInstaller。交付 EXE、NSIS、兩個指向 NSIS 的簽署更新清單、Python 離線依賴包及驗證文件。歷史 CompanyAI-offline.zip 不重製、不冒稱新版。
- 修改 python_worker.py 後以 Prepare-Python.ps1 -Destination <新目錄> -RefreshManifest 重建，核對 assets/python-runtime-manifest.json 與 python-runtime-sha256.txt，再編譯。Python 原始碼必須 LF，避免不同 Git checkout 改變固定雜湊。

## 0.8.41：圖片關聯、單張試驗與紀錄捲動（最新指示）

- 修正圖片及原生快速摘要子請求：直接使用已核對父請求中的遠端 conversation_id，同 owner／project／run／conversation；client_snapshot 不允許網站自行補回歷史。
- 使用者把100張說明為舉例，本版不做批次功能／技能。每次任務只允許1次不同圖片辨識要求；同來源與焦點的完成結果可重用。最大5 MB明確採5,000,000 bytes，拒絕超額，不自動壓縮；圖片JSON最多10,000,000 bytes並遵守網站公告。
- 圖片只送當次無工具子請求，後續主對話只保留依需求整理的文字重點與來源。待查子請求仍以本機DPAPI保存以防未知提交重播；完成或明確未受理後清除圖片快照。不擅改網站資料庫。
- 快速模型 fast 不公告image-read／analyze_image，不載入圖片技能說明；原生入口、工具及請求建立均阻擋並提示此模型不支援圖片傳入。
- 工具紀錄保留清單節點、底部追加；120筆輪替保留可見列。中途閱讀不強制回頂端，在底部才跟隨；保留進度筆記及已完成紀錄的內部捲動。
- 沿用v142、EXE／main交付授權；完整Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot後推送。不改icon.png，不製作NSIS／ZIP。舊版已保存但未受理的422任務需重新送出新任務，不改寫未知提交ID。

## 0.8.40：Excel 欄位規劃與使用者圖表編輯（最新指示）

- 使用者授權實作 Excel 欄位規劃及完成圖表的本機編輯、恢復原樣與儲存圖片；沿用 EXE／既有 main 交付範圍，不改圖示、不製作 NSIS／ZIP、不開放 PS。
- AI 依要求、實際表頭及少量樣本規劃 time/X/Y；原生重查來源版本、表頭與時間格式，規劃隨 CSV／DPAPI 續接保存。新增 plan_excel_analysis、export_planned_excel，目錄共47項；不重新編號不連續欄。
- 本地時間掃描最多250000列／120秒，符合最多10000列／9欄；HH:MM是時:分，區間起點含、終點不含。日期序號不取餘數；錯誤／空時間不默默略過；不靜默抽樣。
- 圖表使用者設定獨立保存在加密歷史、不傳模型、不改原值；完成訊息可雙擊／編輯。支援標題、實體X/Y軸範圍、系列名稱／色彩、圖例位置及最多10條參考線。PNG只在原生確認的專案_AI_Output新時間資料夾保存並讀回驗證；來源版本碼不顯示，內部保留。
- 圖型為line/bar/scatter/step/area/horizontal_bar，標題置頂置中、圖例預設右側；不提供自由拖曳排版及需額外統計的箱形／直方圖。
- 發行需 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot；驗證三份Excel（含欄序調換）× 五時段共15圖。模型語意判斷與公司真實資料仍待使用者實測。
- 使用者已明確把圖片5 MiB上限與自動壓縮延後至下一版，待公司圖片API實測；本版維持原圖1 MiB與既有圖片請求格式。

## 0.8.39：Outlook 專案入口與圖片試驗（最新使用者指示）

- 使用者授權本版 Outlook 整合＋圖片嘗試、進版編譯 EXE 並部署既有 Git main；沿用 v142、空 Cargo 快取及 Build.ps1 全流程，不改 icon.png／NSIS／ZIP，不開放受控 PS。
- 專案 Outlook 按鈕共用原有 runner、技能、勾選權限及 1000／50 封額度；獨立預覽／MSG 入口先保留，不把入口整合宣稱為所有 UI 已合併。
- 專案 JPG／JPEG／PNG 圖片試驗：analyze_image(path,focus)，image-read 按需載入，工具目錄共 45 項。單張 1 MiB、8192×8192／1600 萬像素、每次任務最多 20 次不同要求。只允許專案相對路徑及既有檔案保護。
- 使用者已實測模型伺服器的 image_url；公司 API 負責轉發及身分驗證。本版明確允許圖片試驗，即使舊 capabilities 只公告 text；仍遵守授權、模型與整份請求大小上限。若轉發端拒絕，不另接模型直連端點、不略過 TLS 驗證。此條僅涵蓋無工具圖片子請求，取代舊文字專用限制。
- 真正 user.content 陣列包含 text／image_url data URL。使用目前模型、既有 agent/turns 與 parent_request_id，tools=[]；主歷史只保留文字及 SHA256。DPAPI 保存待查子請求，未知提交只查原 ID；完成移除圖片快照，相同來源與要求快取。
- 不讀 Outlook 圖片附件／內嵌圖，不解碼掃描 PDF。這些留待後續整合。公司代理／實際模型辨識品質待使用者驗收。詳見 docs/VALIDATION_0_8_39.md。

## 0.8.38：週報精靈、網路專案及回覆恢復（最新使用者指示）

- 使用者已確認實作、編譯 EXE 並推送既有 Git main；沿用 EXE 發行，不重建 NSIS／ZIP，不改 icon.png，受控 PowerShell 延後。
- 專案提供資料傳送說明及目前 Windows 下載／桌面的預設資料夾建立功能；使用 Known Folder 重新導向位置。明確開放映射網路磁碟與 UNC 專案，取代舊版只允許本機路徑的限制；仍檢查路徑、重解析點、硬連結及專案邊界。
- 專案快速操作只留生成週報：說明、建立素材目錄／日期／選填補充、送出前確認。按否回輸入；確認後才啟動任務；取消不刪除素材。日期預設本週一至今天，提示帶本機日期及 ISO 週年／週別，使用者明確日期優先。
- Exchange／OST 線上收件匣及子資料夾加入既有 Outlook 工具，仍需勾選授权；不新增 Graph／新 Outlook 連線能力，不宣稱快取等於即時伺服器資料。
- 無效模型回覆改為兩次一般修復、封存原始工具結果後兩次精簡上下文恢復；每段累計最多 30 次。保留使用者原文、權限、額度、來源及副本狀態；未知提交不可重送，無進展及工具錯誤保護不變。
- 完整驗證使用 Build.ps1 -EmptyCargoCache -TestOffice -NetworkTestRoot，固定 v142 x64、空 Cargo 快取與 --frozen；本機 SMB／映射測試不得冒稱公司 F 槽及真實 Outlook 驗收。見 docs/VALIDATION_0_8_38.md。

適用於本專案及其子目錄的程式碼撰寫、編輯與維護。

## 專案位置與共用環境

- 本專案位於 `Y:\Rust\Project\CompanyAI`，是獨立的 Cargo workspace。
- 原始碼、依賴鎖定檔、.cargo 設定、文件、target 與 dist 留在本資料夾。
- 專案環境入口是 `scripts/Enter-DevShell.ps1`：優先使用解壓後的 `toolchain/`，否則使用外層 `Y:\Rust\.tools`。編譯輸出固定在本專案內。
- `Y:\Rust\.local-ai` 與外層安裝腳本是離線編譯環境的資源，不屬於此應用程式，不要搬入或任意修改。
- 離線 ZIP 自帶固定 Rust 工具鏈與 vendor 套件；解壓到新資料夾即可編譯，但電腦仍須預先安裝指定 MSVC 與 Windows SDK。一般開發也可用 `RUST_SHARED_ROOT` 指定共用工具。

## 使用者指定的程式碼風格

- **簡單好懂、註解充足、他人看得懂且改得了、容易維護，為最優先原則。**
- 使用直觀的控制流程與明確命名；避免難讀的一行式、過度泛型、複雜巨集及不必要的抽象。
- 函式與模組保持單一、清楚的責任，讓接手者能由上而下理解執行流程。
- 主動提供足夠的繁體中文註解，說明模組與函式用途、主要步驟、輸入輸出、限制及重要設計原因。
- 對較難理解的 Rust 所有權、生命週期、並行、錯誤處理及平台相容性設定，補上容易理解的說明。
- 公開介面優先使用 `///` 文件註解，必要時提供小型範例；註解應提供有用資訊，不只重述語法。
- 修改邏輯時同步更新相關註解與文件，避免過時資訊誤導維護者。
- 錯誤訊息交代失敗原因與處理方式；一般執行流程避免無理由的 `unwrap()` / `expect()`。
- 引入套件或增加架構複雜度前，先考慮標準函式庫與較簡單的做法，以實際需求為準。

## 編譯基準

- 公司目標：Windows 11 x64、VS2019、MSVC 14.2x / v142。
- 本機已驗證：Rust 1.98.1、MSVC 14.29.30133、Windows SDK 10.0.19041.0。
- Rust 版本由 `rust-toolchain.toml` 固定；不得因另裝新版 VS 就自行改用較新 MSVC。
- 使用 `scripts/Enter-DevShell.ps1` 載入環境，使用 `scripts/Build.ps1` 檢查格式、Clippy、release 編譯與基本執行。
- VS Code 使用 `scripts/Configure-VSCode.ps1` 產生本機 Rust / MSVC 設定，再重新載入視窗。不要把含本機路徑的 `.vscode/settings.json` 提交 Git 或打進 ZIP；離線工具鏈須保留 rust-src，以支援程式碼分析。
- 不加入 `target-cpu=native` 等綁定家用 CPU 的設定。
- 公司完整 MSVC 修補版本與 SDK 版本尚待核對，不宣稱兩邊環境已完全一致。

## 工作範圍

- 本專案目前開發 LM_AI Windows 測試版：瀏覽器授權登入、30 天登入保存、OpenAI 相容 Chat Completions 請求與回覆顯示；網頁端串接規格記錄在 docs/WEB_INTEGRATION.md。
- 網頁控制介面及共用套件編譯管理由另一個 Codex 任務處理。本專案只維護自己的 EXE、原始碼與離線交付包。

## 0.7.0 產品約定

- 本版覆蓋舊版衝突描述，以 docs/DESKTOP_0_7_CONTRACT.md 為準。一般模式即 stream:true + execution_mode:stream；背景為 false + background；不保留同步聊天入口。路由維持 /lm_server 前綴。
- outlook_triage 與 auto_generate_title 必須互斥；Outlook 初篩走背景，不額外生成標題。一般標題為獨立快速模型背景任務；改名、置頂與草稿保存在本機。
- 關閉視窗及最小化均縮到托盤，托盤「離開」才退出。單一實例鎖只限正式模式；demo 與自我檢查不得喚醒正式程式。
- 選字浮動圖示預設關閉，只查 UI Automation 選區位置，使用者點擊才複製；快捷鍵有獨立啟用開關。
- 安裝包為 dist/LM_AI_Setup.exe，部署至使用者 Programs/LM_AI，資料維持 CompanyAI。0.8 以 NSIS 安裝／更新／解除安裝，禁止使用者端呼叫 CMD、PowerShell、BAT 或外部腳本。更新契約以 docs/UPDATE_0_8_CONTRACT.md 為準：下載完整 Setup，內建 RSA 公鑰驗證版本／平台／長度／SHA256 簽章。一般更新需下載前、安裝前兩次同意；真正退出不自動安裝。低於最低版本鎖住功能，已知門檻持久化直到新版達標。更新私鑰在 .private/，嚴禁提交或納入離線包。

- 正式主機、聊天、device、token、version、models 與 download 路由固定於 `src/config.rs`，不提供 UI 編輯；偏好檔只保存模型代號、快捷鍵、字體大小、側欄收合、通知提示及深色模式偏好。舊設定不得覆寫固定路由。
- 正式請求固定 Bearer 個人 Token，Chat Completions 一般模式為 `stream: true`，背景為 `stream: false`；模型選單從後端取得，UI 顯示 label、JSON 傳 id，真實模型由後端映射。
- 使用者指定：版本檢查失敗暫時允許使用；已知的強制更新跨重新啟動保存，不得被網路失敗或後續降低門檻解除。這個政策不能略過登入驗證或模型權限。
- 全新設定快捷鍵預設 Win+Esc，既有偏好保留；支援直接按鍵錄製，套用成功才生效；必須在顯示主視窗之前取得來源選字。只放入草稿，不自動送出、不覆蓋舊草稿、不背景監控剪貼簿。
- 0.4.1 擷取完成後先填入草稿再請求前景；不依賴 Ctrl+V 或強制焦點技巧。等待新剪貼簿時不限制擁有者 PID／來源 HWND，以支援 Adobe 多程序；讀取新的穩定純文字並要求使用者確認。錄製時暫停原快捷鍵，取消／逾時／離開程式時恢復。
- 0.4.0 已加入內嵌 WebView2 介面、Markdown／KaTeX／高亮、本機 DPAPI 歷史、通知 REST／WebSocket，以及 Classic Outlook 唯讀預覽／確認分析。
- 公司一律使用 Classic Outlook；以 Rust windows COM 操作，不依賴 Python 或 pywin32。不寄信、不修改郵件、正文必須經使用者明確確認後讀取。
- 0.5 契約集中於 docs/DESKTOP_0_5_CONTRACT.md。附件能力由獨立 capabilities 路由提供；文件／圖片合計最多 20 個，server 決定格式與大小。
- 附件以 JSON 預約 job_id、PUT 原始 bytes、REST 查轉檔狀態，ready 後才送 AI；本機只分塊 DPAPI 暫存，不轉 MD、OCR 或處理文件。
- stream／background 都依賴 server 持久任務與 owner + client_request_id 去重；SSE／WS 不取代 REST 結果。同帳號 principal_id 穩定，任務加密隔離保存；未知提交不可換 ID 自動重送。
- 最小化至托盤，右鍵可還原／離開，退出不取消 server 任務。圖示使用 assets/app.png 原圖與 assets/app.ico 多尺寸資源，EXE／視窗／工作列／托盤共用；更新圖片後執行 scripts/Convert-AppIcon.ps1，ICO 為必要交付資源。
- 0.6 全站鈴鐺 API 與 AI events 各有游標／快取；網站已讀與刪除成功才更改本機，deleted_ids／410 完整同步契約見 docs/DESKTOP_0_6_CONTRACT.md。
- 模型切換必須重新查 capabilities?model=alias；不能以舊模型的附件規則放行新模型。網站仍須驗證轉檔後的圖片是否可交給模型。
- 0.6 Outlook 批次 Skill 於 src/outlook/skill.md；只有本批已勾選、經使用者確認的郵件可依 AI JSON 請求匯出 MSG。允許的工具只有 outlook.export_msg，EntryID／StoreID 與磁碟路徑不提供 AI。
- 多封日期查詢預設為 Outlook 目前資料夾及子資料夾，另可選預設收件匣及其子資料夾；介面已移除所有已載入信箱／本機資料檔選項。不掃描磁碟或自動開啟資料檔。每批取最新 50 封，500 個資料夾／10,000 個項目／30 秒上限與讀取失敗須提示結果不完整；自動 MSG 補充最多 20 封並受 backend 規則限制。退出不恢復 COM 授權；最終聊天使用持久任務。
- RAG、UNC 知識庫、任意檔案工具及其他自動化尚未實作，不自行擴張本次範圍。
- 前端程式位於 ui/，build.rs 將資源嵌入 EXE；第三方資源及授權位於 ui/vendor，不使用 CDN，也不要求公司安裝 Node。
- 使用者同意附 WebView2 x64 離線安裝包，放在 dist/ 並納入 ZIP；更新時核對 Microsoft 簽章及 SHA256。
- 不向前端傳 Token、真實郵件 EntryID 或任意檔案／命令執行能力。通知不能直接觸發外部工具。
- 使用者明確要求先提供網頁契約時，可先獨立提交／推送契約文件供同步開發；必須標示程式尚未驗收，其他原始碼及 EXE 待完整交付驗證後再提交。
- 修改功能時同步更新 docs/WEB_INTEGRATION.md 與相應驗收說明。沒有實際操作 Word／Outlook 或取得外觀截圖時，不能宣稱這些驗收通過。

## 0.8.37：Outlook 隱私選擇與本機比對（最新使用者指示）

- 使用者要求實作、進版、編譯 EXE 並部署 Git main；供公司實測。包含未提交的0.8.36變更，不重建NSIS／ZIP、不改圖示。受控PowerShell明確延後。
- 先在本機列資料夾供勾選，首次預設全選、之後保留选择；未勾選分支不可列給AI、讀取或匯出。DPAPI設定共用於專案及Outlook助理，實際資料夾祖先鏈再次核對。變更範圍拒絕舊快照續接，已送出的資料不可宣稱已撤回。
- 本機前文比對最多1000封不同版本、64MiB／120秒；AI內文最多50封，分開計數。工具只回比較摘要，不回未選讀內文；全文正規化精確涵蓋才略過，不以字數／相似度判定完整。
- Outlook任務優先同串最新信，再按缺口補讀；週報一開始載入方法／讀舊結構。保留來源與短結論，足夠即交付。限制與測試見docs/VALIDATION_0_8_37.md。
- 發布必須使用Build.ps1 -EmptyCargoCache -TestOffice驗證v142 x64、空Cargo快取／--frozen、原生UI與Office回歸。公司Outlook／模型品質待使用者實測。

## 0.8.36：本地 CSV 與主動上下文整理（最新使用者指示）

- 使用者要求兩項實作後編譯 EXE 並部署 Git；CSV 建立後不得反覆把完整原始資料帶入對話。沿用 main／EXE 發行授權，不製作 NSIS／ZIP，不改 icon.png。
- Excel／LOG 先確認樣本與欄位，再由原生工具匯出來源追蹤 CSV；模型只見路徑／版本／欄位／筆數、首尾各 10 筆及統計。chart_dataset 直接讀 CSV，回覆只有圖號／處理摘要；型別、顯示文字、原列號與异常值明細保留。
- LOG 每批最多 30 檔、250000 筆／16 欄／64 MiB；固定分隔或起訖標記，不執行腳本。Excel 沿用本地圖表讀取限額 10000 列／9 欄／90000 格。圖表仍最多 10000 筆，不靜默抽樣。
- 匯出成功先封存完整工具結果，再精簡舊原文；compact_context 保存交接筆記／失效結論／下一步。補充於安全界線先縮短舊結果，標記舊笔記待核對，不改寫已提交或未知的模型請求。保留使用者原文、去重、來源／CSV 版本、24 小時與無進展限制。
- scripts/Build.ps1 -EmptyCargoCache -TestOffice 驗證 v142、空 Cargo 快取／--frozen、原生 HTTP CSV／更正／查回、30 份 LOG 及真實 Excel CSV 往返後才發布。公司加密 CSV 行為、實際模型選欄與速度待使用者驗收。

## 0.8.35 發行（最新使用者指示）

- 使用者已要求編譯後推送 Git，取代下方前兩輪只修改原始碼的安排。合併進度筆記顯示、LOG 近似時間定位及主輸入框三種傳送模式，進版 0.8.35。
- 使用 scripts/Build.ps1 -EmptyCargoCache -TestOffice 完成 MSVC v142 x64、空 Cargo 快取／--frozen、原生與 Office 驗證後，交付 EXE、簽署清單、原始碼與文件至既有 GitHub you1230783-sys/CompanyAI 的 main。不製作 NSIS 或離線 ZIP，不改其他專案與 icon.png。
- 排程與停止須驗證原任務先結束、原對話／帳號／模型綁定、重啟不自動送出及加密歷史；不得以 DOM 檢查代替原生執行。公司模型與實際 LOG 品質仍待使用者驗收。

## 0.8.35 納入：LOG 定位與主輸入框傳送方式（原待發行項目）

- 使用者的約略時間先前後各 5 分鐘定位事件錨點，再回讀確認開始；無結果有界擴大至前後 15／30 分鐘，不反覆找整點字串。只有時間的 LOG 可查，日期來源明示；支援時間後逗號分隔訊息。
- 專案執行中使用主對話框選擇下一輪提示（預設）、停止後啟動新任務、完成後啟動新任務；移除補充專用小視窗。每個對話先支援一則可取消的待送新任務，與歷史一起 DPAPI 保存，綁定原對話／專案／帳號／模型。
- 舊 worker 結束後才啟動新任務；暫停、失敗、等待補充、手動停止或重啟保留待送文字供手動傳送。草稿只在確認接受且仍為原文字時清除。
- 前輪只改原始碼的安排已由上方 0.8.35 發行指示取代；原始設計見 docs/PENDING_LOG_AND_COMPOSER.md，發行驗證見 docs/VALIDATION_0_8_35.md。

## 0.8.35 納入：顯示進度筆記（原待發行項目）

- 執行中顯示模型提供的最新 progress_note；屬於工作進度摘要，不是內部思考過程。沿用工具活動紀錄保存，完成／暫停／失敗後連同中途說明收進預設收合區，不混入最終正文或複製內容。
- 前輪延後編譯的安排已由上方 0.8.35 發行指示取代；設計說明保留於 docs/PENDING_PROGRESS_NOTE_UI.md。

## 0.8.34：大型 LOG、Outlook 與長任務

- 本輪授權包含大型 LOG、執行中補充、專案 Outlook 唯讀、占用檔案等待重試、圖表預設略過異常文字及異常值明細、跨對話上下文整理、最長 24 小時任務與 checkpoint。程式原始檔、CMD／PowerShell／虛擬環境另輪處理。
- LOG 檔名 YYYYMMDD_分類_站別.log（A01-01、Z01-CY），內文 `2026/06/23, 15:25:48.084`。內文日期優先；單檔 32 MiB／每次 30 檔，逐行分頁／時間關鍵字搜尋／行號／來源版本。未讀完及未知時間明確標示。
- 補充指示逐次核對 run／conversation，DPAPI 保存原文；下一輪作 user 訊息，保留原始目標。未開始的舊候選工具回傳 executed=false 再規劃，已開始的先完成。
- Outlook 工具第一次執行前必須原生 UI 同意，暫停或重啟續接重新詢問；本地 PST 收信及規則資料夾、Exchange／OST 線上信箱寄件備份分層選擇，先標題後重要內文，不寄信或讀附件。不能把離線快取宣稱為即時伺服器內容。
- 正式原生任務每段最多 24 小時，60 工具／80 回覆自動 checkpoint 換批；無進展及錯誤上限保留，不自動重設。手動繼續開始新一段時間。原請求未知只 GET 查回；本機修改開始先使舊 checkpoint 失效，完成核對後才恢復，不重播未知寫入。
- 原始要求／補充、工作筆記、真實狀態與舊工具索引分開；65% 工具文字預算或 30 對工具歷史自動整理，完整結果 DPAPI 分檔按需回讀。近期三輪用戶要求與 AI 最終答案、工具計數供下輪使用，過大最終答案有查回指標。摘要不取代授權及證據。
- 使用者已授權完成後 EXE 上 Git main；以 Build.ps1 -EmptyCargoCache -TestOffice 完成 v142／空快取與 Office 驗證；不製作 NSIS／ZIP。不得宣稱模擬時鐘等於已連跑 24 小時；實際公司郵箱／加密／模型品質待使用者實測。詳見 docs/VALIDATION_0_8_34.md。

## 0.8.33：圖表資料決策與按需工具

- 使用者授權完成後進版、編譯 EXE 並上傳 main；以 Build.ps1 -EmptyCargoCache -TestOffice 驗證，不製作 NSIS／ZIP。
- 明確數字文字只在數值軸轉換；類別 X 保留前導零。異常值按欄／類型由桌面 UI 選擇 gap／skip／zero，原空白不變；無效 X 僅可明確排除整列。模型不提供使用者決策參數。
- 等待／稍後決定不算工具失敗；同一已完成模型請求暫存續接，重新預檢並核對來源版本。PNG 與聊天室共用座標及缺值規則。
- 原生專案初始七個基本工具；九項技能簡介，load_skill 啟用對應工具與唯一的 system 說明，依賴組合及續接持久化。模型／副本／圖表條件仍適用，不更改網站路由或資料庫。

## 0.8.32：Office PNG、進階診斷與工具可用條件（最新使用者指示）

- 使用者已改為要求完成後編譯並上傳：進版 0.8.32，以 Build.ps1 -EmptyCargoCache -TestOffice 驗證後發布 EXE、簽署清單、原始碼與文件至既有 main；不製作 NSIS／ZIP。
- office_action／office_batch 新增 insert_image：專案相對 PNG，經路徑鎖／CRC／尺寸／SHA256 核對，Word 段落前／文末、Excel 儲存格錨點、PPT 座標，保持比例、嵌入而不外連，只改副本。圖片來源在發布前須保持不變。
- 執行紀錄入口移至設定內預設收合的「進階功能」，可選目前對話的不同任務；聊天／進度區不再顯示診斷按鈕。
- fast 不公告 summarize_document；沒有副本／圖表時不公告相關編輯／儲存／匯出工具。空 copy_id／revision 進有界修復，不執行空白修改。
- 使用者已要求網頁端放行已知欄位正確的回覆；桌面以白名單讀取結果／訊息／工具封裝，忽略額外欄位。必要欄位、身分配對、finish_reason 與工具 arguments Schema 仍驗證；詳見 docs/VALIDATION_0_8_32.md。

## 0.8.31：圖表、PNG 及本機診斷

- 使用者已改為要求完成後進版、編譯並上傳 Git，取代本批先前「不編譯、不進版」指示。發布 0.8.31 EXE、簽署清單、原始碼與文件至 main，不製作 NSIS／ZIP。
- 單圖最多 10000 筆、8 系列；Excel 直接畫圖獨立最多 90000 格，一般 read_excel_range 仍限 2000 格，不靜默抽樣。
- export_chart_png 由內嵌 ECharts 繪製完整範圍，受控寫入 _AI_Output，重名加序號、讀回核對、任務去重及交付前再驗證。
- 工具失敗顯示原因，另提供本機加密日誌的查看／複製；不將本機顯示視為已修復網站資料保存。
- 執行 Build.ps1 -EmptyCargoCache -TestOffice；涵蓋真實 Excel 一萬筆取值、PNG WebView2 callback 與目視檢查。實際公司加密及網站聯測仍需使用者確認。紀錄見 docs/VALIDATION_0_8_31.md。

## 0.8.30：原生工具 Schema 相容性修正

- 使用者回報第一輪 `UNSUPPORTED_SCHEMA`：type 陣列只允許單一型別加 null。桌面轉換器須將 Office 儲存格與圖表橫軸的多型別改為非根位置 anyOf，不要求網站放寬既有契約。
- 沿用 EXE／Git 發行授權，執行 Build.ps1 -EmptyCargoCache，涵蓋 v142、空快取、全部工具 Schema 及既有原生代理整合。Office COM 實作未修改，不重跑 Office／NSIS／ZIP；公司網站聯測仍需使用者確認。

## 0.8.29：desktop-agent-v1 原生工具

- 依使用者要求實作已發布的 docs/DESKTOP_AGENT_V1_CONTRACT.md；新專案及快速摘要委派走原生代理路由。一般聊天、Outlook、標題、PDF 不變。
- 不從模型正文擷取工具 JSON；能力不支援不降級。僅已存在的舊 checkpoint 延續原協定；debug 的 legacy 測試入口不可作正式降級。
- 使用者已要求編譯後上 Git：進版 0.8.29，以 Build.ps1 -EmptyCargoCache -TestOffice 完成 v142／空快取／原生代理及 Office 驗證，提交 EXE、簽署清單、原始碼與文件至 main；不製作 NSIS／ZIP。狀態見 docs/VALIDATION_0_8_29.md。
- 使用者後續要求一併加入 Excel 選欄／分批讀取。新增 inspect_excel／read_excel_range／chart_excel_range；不變更網站通用契約，資料每批最多 2000 格、預設 100 列；完整編輯快照維持原上限。

## 0.8.28 專案技能與視覺化（最新）

- 使用者授權側欄獨立收合、Markdown 說明、三種工作技能、跨文件搜尋、批次 Office、ECharts 與品質委派快速摘要。沿用 EXE／Git 發行授權，不製作 NSIS／ZIP。
- 技能只載入內建文字；圖表固定資料結構、離線 ECharts，不執行模型腳本。Office 批次候選成功才提交；快摘要不標記主模型已讀。
- 委派保存原請求，未知結果不得重送 POST；與父任務共用取消／時限，摘要 DPAPI 快取與來源版本核對。
- 使用 Build.ps1 -EmptyCargoCache -TestOffice 驗證；公司加密、實際模型摘要品質及視覺版面需另行實測。

## 0.8.27 長任務時限與異常續接（最新）

- 使用者要求專案時限延長至兩小時，並修正異常停止只有重試、沒有繼續的情況。每段 7200 秒，60 工具／80 模型回覆不變。
- 時限、格式修復次數、無進展與文字預算上限成功 DPAPI 暫存後可手動繼續。待查模型請求保存 ID／已知狀態；續接只查原請求，不重送 POST，已完成回覆可直接沿用。
- 取消、回應身分錯誤、未知本機操作不可套用一般續接；舊版失敗且沒有 checkpoint 的任務不補造狀態。0.8.26 暫停檔向後相容。
- 沿用 EXE／Git 發行授權，進版 0.8.27，Build.ps1 -EmptyCargoCache；沒有修改 Office COM，不重跑 Office／NSIS／ZIP。時間邊界採 debug 專用短時鐘實測，不宣稱已等待兩小時。

## 0.8.26 Office 初版與暫停續接

- 使用者同意新建 Office、基本結構與排版初版，並要求工具／模型回覆上限改為保存狀態後手動「繼續」。沿用 EXE 測試版交付方式；本版不製作 NSIS／ZIP。
- 新建限 DOCX/XLSX/PPTX，既有 Office 副本維持來源格式。模型只能使用固定 office_action，禁止任意 COM／Shell／巨集。格式快照共用樣式表，數值正規化至千分之一點。
- 每段仍最多 60 工具／80 模型回覆；剩餘四次時提醒摘要，達上限 DPAPI 保存工作副本、去重紀錄與進度，釋放程序。使用者明確續接才重新授權，核對帳號／專案／版本，不自動重播未知請求。
- 404 診斷保留原 POST 錯誤與 GET 路由；同一請求有限重查，不換 ID 或重送 POST。公司那次原因未取得後端日誌不得宣稱已確認。
- 驗證使用 Build.ps1 -EmptyCargoCache -TestOffice；記錄原生 Office、新建及舊格式、真實 60 工具暫停續接與第 20 輪短暫 404。公司加密與視覺排版仍需公司實測。

## 0.8.25 專案文字工具協定（最新使用者指示）

- 使用者要求採用 OpenAI Chat Completions 的 tools／tool_calls 形狀，但定義與回覆都維持在 messages 文字中；網頁只轉送，不新增原生 tools、tool_choice、response_format 或 role=tool 的 HTTP 欄位。
- tools.json 集中定義參數，skill.md 只教一種回覆格式並精簡重複說明。每輪單一呼叫，id 映射原 operation_id，function.name／arguments 映射既有 Decision／Tool；finish／ask_user 也是桌面自訂工具。不是 API 原生工具呼叫，也不宣稱強制 Schema 保證。
- arguments 提示使用標準 JSON 字串，parser 同時容許完整物件，避免多一層引號的無效重試；保留舊 action 格式相容。多操作、混用協定、缺漏參數先修復，不猜工具／版本；未知工具與無效操作 ID 停止。版本、授權、去重與成果檢查不放寬。
- 使用者已要求編譯後放到 Git，取代先前僅改原始碼的限制。進版 0.8.25，執行 Build.ps1 -EmptyCargoCache，交付原始碼、EXE、簽署 EXE 清單與驗證文件至既有 main；不製作 NSIS／ZIP。詳見 docs/DESKTOP_0_8_25_CONTRACT.md。

## 待下次發行：缺少工具名稱的格式修正

- 使用者回報摘要工具 request 漏寫 tool，要求修正原始碼，**先不編譯、不進版**，留待其繼續測試後一起處理；本輪不打包、不提交或推送，保留 0.8.24 成品與清單。
- 缺少／非物件 request，或缺少／空白／型別錯誤的 request.tool，回傳明確格式原因，沿用既有有限修復；不自行猜測工具名稱。非空但不支援的工具仍停止。
- 補上回歸測試，僅做格式與原始碼檢查，未編譯執行的測試必須明確標示；詳見 docs/PENDING_TOOL_NAME_REPAIR.md。

## 0.8.24 可選閱讀筆記（最新）

- 使用者要求取消每三次閱讀／六次操作的筆記要求。每份文件、每個版本各自計數，有效閱讀超過四次且未完整讀完時，下一輪才附一次可選技能；可略過，不重試或阻擋。
- 讀完全文後該文件計數歸零；重讀、重播及失敗不增加計數，切換文件不混算，版本變更重新計數。分段摘要按需，完整文件摘要及 .lmai 保存流程保留。
- 自願提供 progress_note 仍可用於有限續接；沒有筆記不擅自裁掉未摘要原文。沿用 EXE／main 交付授權，進版 0.8.24，執行 Build.ps1 -EmptyCargoCache，不重製 NSIS／ZIP。

## 0.8.23 專案記憶（最新）

- 使用者授權實作 `.lmai` 筆記、文件分段摘要、任務摘要／完整結果、PDF 跨任務快取；一般檔案清單、讀取、匯入與副本工具必须排除 `.lmai`，不把它當文件送出。
- 私有資料 DPAPI 保存，來源版本／閱讀證據／副本狀態由程式核對，摘要只作語意記憶。新對話請求選取摘要及近期要求，完整舊答案可按需讀回；一般聊天不變。
- 單一專案任務卡與最終通知，內部往返只顯示活動；按精確請求 ID 過濾通知，網站須保留 resource_id 關聯。
- 交付 0.8.23 EXE、簽署清單、原始碼及文件至既有 main，使用 `Build.ps1 -EmptyCargoCache`；不製作 NSIS／ZIP。公司加密與模型摘要品質待使用者實測。詳見 docs/DESKTOP_0_8_23_CONTRACT.md。

## 0.8.22 專案筆記與有限續接（最新）

- 使用者已回報 0.8.21 PDF 路由可讀取；本輪改善工具回覆不穩、多 JSON 及空白 done，不變更 PDF／Office 讀取方法。
- 唯一合法操作可附說明；多 JSON／不完整／空白完成只要求修復，不猜選候選命令。未知工具、身分錯誤、取消及結果不明不自動另建任務。
- 三段新閱讀或六次有效非瀏覽操作後，要求下個正常 JSON 同層附 progress_note；不多呼叫一輪筆記。程式保存區間、版本、操作及副本狀態，AI 筆記不是授權或成功證據。
- 無進展最多修復兩次，全任務六次；第一次原地修復，第二次按筆記縮減已摘要歷史，保留原始需求／補充、最近兩段已摘要證據及全部未摘要結果。沿用同個 broker／worker／PDF 快取，不重播成功寫入。
- 上限 60 次工具、80 次模型回覆、連續八次無進展、30 分鐘；文字預算 120 KB 軟門檻／240 KB 硬門檻，不等同模型 token。一般聊天仍維持 20 輪限制。
- 交付 0.8.22 EXE、簽署清單與原始碼至既有 main，Build.ps1 -EmptyCargoCache；不製作 NSIS／ZIP。真實 GLM／Gemma 的改善幅度由使用者測試。詳見 docs/DESKTOP_0_8_22_CONTRACT.md。

## 0.8.21 伺服器 PDF 轉換（最新）

- 使用者已確認 0.8.20 MSG 在公司測試成功，PDF 一般附件伺服器轉檔可正常閱讀；不把此回報當成新路由已驗收。
- 專案 PDF 正式任務改用 POST /api/desktop/documents/pdf-to-markdown，保留部署前綴（公司 /lm_server）；Bearer 桌面 Token、X-Client-Version、單一 multipart file，50 MiB 上限；200 回原始 UTF-8 Markdown，不是 JSON。
- 不传 extract_images；Server 已固定 false。同步等待，不輪詢；WinHTTP 非同步 I/O 支援取消，receive 180 秒／整次 300 秒，不自動重送。原檔鎖、專案邊界與 TXT 成果規則保留。
- 相同任務依來源 SHA256 重用記憶體快取；不寫明文 MD、不把伺服器 filename 當磁碟路徑。全文仍為 200 KB 上限，不靜默截斷。
- 交付 0.8.21 EXE 與簽署清單、原始碼與文件，Build.ps1 -EmptyCargoCache；沿用 main 推送授權，不製作 NSIS／ZIP。真實新路由仍待公司測試。

## 0.8.20 詢問與讀取修正（最新）

- 使用者回報 PDF／MSG 都可手動開啟，但工具失敗；說明＋ask_user 被誤拒絕。统一 Decision 解析與等待補充狀態。
- MSG 獨立暫存副本協調 Outlook 共享需求，不放寬原件寫入保護；保留 COM EXCEPINFO。公司加密仍待實測。
- 使用者確認公司免費 Acrobat Reader 可正常開啟，但 Word 讀 PDF 是亂碼。Word PDF 備援本機測試未正常返回，不納入；PDF 改進原因說明及 ask_user 匯入流程，不宣稱自動解密已修好。
- 沿用 EXE／main 推送授權，交付 0.8.20，使用 Build.ps1 -EmptyCargoCache -TestOffice；不重製 NSIS／ZIP。詳見 docs/DESKTOP_0_8_20_CONTRACT.md。

## 0.8.19 文件相容性與工具說明（最新使用者指示）

- 修正成果定位路徑，完整單一工具 JSON 可附說明並繼續執行；說明直接呈現在對話中，不再要求重送完整合法工具。歧義／未知工具停止，不完整 JSON 最多修正兩次。
- PDF 以內嵌 pdf-extract 在 AppContainer 讀取；MSG 以已開啟的 Classic Outlook OpenSharedItem 讀取。兩者只支援文字閱讀及 TXT 衍生副本，不編輯原始格式、不做 OCR／附件解析。
- Office 支援 DOC/DOCX/DOCM、XLS/XLSX/XLSM/XLSB、PPT/PPTX/PPTM，副本保留原格式；仍只修改既有文字區塊，不新增表格／列欄／版面。
- 使用者已詢問 Word 表格能力並獲告知：既有表格文字可修訂，尚無新增表格工具；本輪不擴充表格結構操作。
- 本輪交付 0.8.19 EXE、清單與原始碼，執行 `Build.ps1 -EmptyCargoCache -TestOffice`，沿用 main 推送授權；不打 NSIS／歷史 ZIP。MSG／公司加密未實測通過不得宣稱驗收。

## 0.8.18 成果與 Office（最新使用者指示）

- 使用者要求成果可定位 Explorer、輸出保留可讀檔名、任務資料夾以時間命名，並試用 Word／Excel／PowerPoint；完成後編譯 EXE 並推送既有 main。
- 進版 0.8.18，執行 `Build.ps1 -EmptyCargoCache -TestOffice`；本輪不製作 NSIS 或歷史 ZIP。公司加密及版面外觀未經公司實測不得宣稱驗收。
- Office 第一版 DOCX 正文、XLSX UsedRange 文字／數字、PPTX 一般文字框；原件唯讀，固定 COM 工具，禁止模型指定任意 COM 方法。公式不改，不從空白建立 Office。相關限制见 docs/DESKTOP_0_8_18_CONTRACT.md。

## 0.8.17 專案歷程與重試（最新使用者指示）

- 使用者已回報公司測試：列專案目錄、檔案清單、編輯 TXT 與儲存副本均成功；不延伸宣稱所有公司加密格式皆已驗收。
- 本輪合併先前 UI 修改與 VNC 更新對話框，通知摘要最多 50 字，專案工具歷程可收合，包裝工具 JSON 最多修正重送兩次，最新提問提供重試。
- 重試保留原對話及成果；結果不明時使用原請求 ID，不另建重複伺服器工作。專案重試建立新執行器／輸出目錄；一般聊天保留原模型、用途及仍有效的附件。
- 最新使用者明確要求完成後編譯 EXE 並放到 Git：進版 0.8.17，執行 `Build.ps1 -EmptyCargoCache`，提交並推送原始碼、EXE、簽署 EXE 更新清單及文件至既有 main。此條取代上一輪僅修改原始碼的限制。
- 本輪先不製作 NSIS 或歷史離線 ZIP。原生選檔、專案移除及真實網站操作的未測部分需據實記錄，不以 DOM 測試冒充完整公司驗收。
- 詳見 `docs/DESKTOP_0_8_17_CONTRACT.md`；先前 `PENDING_PROJECT_UI.md` 與 `PENDING_VNC_SYNC_DIALOG.md` 的 UI 調整一併納入本版。

## 0.8.16 專案文件工具（最新使用者指示）

- 本次實作專案、固定 TXT／MD 工具與 AppContainer 文字執行器；專案位於 VNC 下、最近對話上。一般對話不附帶桌面文件技能，專案對話仍產生獨立標題。
- 對外用途旗標改成 skills 布林值；一般／附件最終整理 true；Outlook 初篩、標題與專案工具往返 false。網站不依標題關鍵字分流。保留本機用途紀錄。
- Outlook 勾選自動補充內文時，初篩與最終整理均為 quality；未勾選沿用使用者模型，不修改一般聊天偏好。
- 原檔唯讀、成果預設 TXT；TXT 不轉 MD。MD 不受公司加密保護，已讀 TXT 的任務禁止發布 MD。無法直接解密時停下，接受使用者明確匯入的記事本文字；不宣稱已整合自動操作記事本。
- 更新箭頭位於設定旁，有更新才顯示；一次原生確認涵蓋指定版本下載與安裝。同版优先 NSIS，較新 EXE 仍優先，EXE 維持手動更換。此條取代兩次更新確認規則。
- 使用者已要求完成後以 `Build.ps1 -EmptyCargoCache` 編譯並推送 EXE、EXE 更新清單、原始碼與文件；先不製作 NSIS 或重製歷史 ZIP，待公司讀寫實測後再打包。本機驗證不能代替公司加密／模型環境測試。

## 修改後的交付與 Git 規則

- **0.8.15 登入與彈窗交付（最新）**：使用者已要求編譯、打包並推送，取代下方本批修改暫不發行的限制。進版 0.8.15，使用 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 完成 v142、Rust、WebView2 與 NSIS 安裝／更新驗證，交付 EXE、NSIS、兩份簽署更新清單、原始碼與文件並推送既有 main。保留現有 VNC 設定與歷史離線 ZIP，不重新製作 ZIP；Git 推送不代表公司內網路由已更新。

- **待下次發行：登入門檻與彈窗關閉（最新）**：登入改為獨立「登入」按鈕，提示「尚未登入時無法使用其他功能」，登入後整區隱藏；未登入時所有其他介面控制及原生命令均禁用，包含 Outlook、VNC 與設定。仍有效的保存授權可沿用。所有 HTML dialog 支援點擊外部關閉；確認視窗以取消處理，關閉更新提示不解除原生版本門檻。使用者明確要求本輪只修改原始碼與文件，不編譯、不進版、不打包、不推送，留待下次一起驗證發行；此條優先於下方舊交付流程。現有 0.8.14 成品與清單不變，詳見 docs/PENDING_LOGIN_AND_DIALOGS.md。

- **0.8.14 NSIS 交付（最新）**：使用者已回報測試成功並要求打包及部署更新。維持 0.8.14，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller`，驗證安裝、更新、重新啟動與解除安裝，交付 EXE／NSIS、兩份已簽署更新清單與驗證文件並推送既有 main。保留 VNC 設定與使用者資料；不重製歷史離線 ZIP。此條取代本版先前等待 NSIS 的限制，Git 推送不代表內網已部署。

- **0.8.14 VNC 預覽與重取（最新）**：匯入預覽依英文／數字自然排序，未分類最後；匯入時排序受影響分類內的機台，既有手動移動功能仍可使用。名稱與 IP 相同標綠並禁止勾選，不同標紅供使用者決定；比對名稱跨分類，重複匹配不任意覆寫。保留網站 Session 到匯入／捨棄，首次清單未分類達 50% 時自動重取一次，上限一次，不循環；手動「再次取得更新清單」只重取一輪 API。取消、停用、正常離開及錯誤均嘗試登出。使用者要求完成後編譯 EXE 並推送 Git，進版 0.8.14，執行 `Build.ps1 -EmptyCargoCache`；NSIS 待實測確認才製作，不重製 ZIP。此條取代先前取得後立即登出與匯入不排序的規則。

- **0.8.13 VNC 手動同步（最新）**：依使用者要求進版 0.8.13，加入可記住帳密的手動網站同步、可調整根目錄／首頁／登入／登出與十個 API、匯入預覽及分類／機台多選管理，仍由既有 vnc_enabled 勾選啟用。網站帳密以 DPAPI 保存，PHPSESSID 只在當次工作使用；每次請求前等待 300ms，取得後嘗試登出。併入前輪更新提示用詞修正。執行 `Build.ps1 -EmptyCargoCache`，只交付 EXE、EXE 更新清單、原始碼及驗證文件並推送 main；使用者實測成功後才製作 NSIS，本輪不更新 NSIS 清單／驗收，不重製歷史 ZIP。下列「待下次發行」的用詞修正已納入本版。

- **待下次發行：更新提示用詞**：使用者要求將下載完成後的按鈕及安裝確認明確寫為重新啟動 LM_AI 應用程式，避免誤認為重啟電腦。本輪只保留原始碼與文件修改，不編譯、不進版、不打包，留待下次一起驗證與套用；現有 0.8.12 成品保持不變。

- **0.8.12 統一完成結果（最新）**：支援使用者指定的 `result.choices[0].message.content` 搭配同層 `sections`、`citations`，背景與串流共用解析。沿用先前打包推送授權，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller`，交付 EXE、NSIS、兩份更新清單及文件至既有 main。沿用 0.8.11 安裝、圖示及歷史 ZIP 界線。

- **0.8.11 完成回覆保留（最新）**：接續使用者回報與原先打包推送授權，修正正文獨存及單行英文五段格式。完成結果非空欄位優先；正文一致才補缺漏，不一致另存可展開原文。執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller`，交付 EXE、NSIS、兩份更新清單與驗證文件並推送既有 main；沿用 0.8.10 安裝、圖示與歷史 ZIP 界線。

- **0.8.10 結構化回覆（最新）**：使用者要求將本輪回覆解析修正打包後推送 Git。進版 0.8.10，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller`，交付 EXE、NSIS、兩份簽署更新清單、原始碼與驗證文件至既有 main。結構化欄位優先、編號舊格式備援，背景與串流完成結果共用解析；正文及重點可見，其餘可展開。沿用三花貓圖示、單純安裝、不附帶／檢查 WebView2、一般安裝不自動啟動；不重製歷史離線 ZIP、不恢復 SFX。Git 推送不代表內網網站已部署。

- **0.8.9 三花貓圖示（最新）**：使用使用者提供的 `icon.png`，原樣複製至 `assets/app.png` 並以 Convert-AppIcon.ps1 產生多尺寸 ICO。使用者授權進版、重新編譯與打包後推送；本輪選用 0.8.9，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller`，交付 EXE、NSIS、兩份更新清單與驗證文件。沿用單純安裝、不檢查 WebView2、不在一般安裝後自動啟動的約定；不重製歷史離線 ZIP，不恢復 SFX。來源圖片不重新繪製或裁切。

- **0.8.8 單純安裝（最新）**：使用者要求移除 SFX 測試包，NSIS 只安裝、不偵測或安裝 WebView2，一般安裝完成不自動啟動主程式。LM_AI.exe 保留原有 WebView2 初始化失敗提示，不新增偵測；既有更新 `/UPDATEPID`／`/RESTART` 交接契約保留。維持 0.8.8，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 後推送 main。使用子程序限定的無效 Runtime 位置驗證「安裝可完成、主程式啟動才失敗並提示」；不移除本機 Runtime、不宣稱已解決趨勢警報。只從最新 Git 移除 SFX 檔案，不改寫歷史，也不重製歷史 ZIP。以下較早的安裝前 Runtime 檢查約定由本條取代。

- **0.8.8 精簡 NSIS（最新）**：使用者同意拆出 WebView2，由公司另外提供。Setup 不嵌入、不下載或自動執行 Runtime；缺少時在替換前提示使用者取得 `MicrosoftEdgeWebView2RuntimeInstallerX64.exe`，以退出碼 2 停止。保留 Git 的獨立 Runtime。維持 0.8.8，執行 `Build.ps1 -EmptyCargoCache -IncludeInstaller` 並推送；測試缺少／零版本、HKCU／HKLM 偵測與既有檔案保留。不重製歷史離線 ZIP。以下隨安裝包提供 Runtime 的舊約定由本條取代。

- **0.8.8 NSIS 交付（最新）**：使用者已實測目前版本大致正常，要求製作 NSIS 並上傳 Git。維持版本 0.8.8，執行 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`，交付 EXE、Setup、兩份更新清單及驗證文件。安裝／更新／解除安裝須保留 EXE 旁既有的 VNC 設定。本輪不重製離線 ZIP；以下較早的「不製作 NSIS」限制由本條取代。

- **0.8.8 附件修正**：使用者回報 0.8.7 選檔後無附件請求；修正已移除刪除按鈕的殘留引用與接收控制項恢復，補上合法檔案完整接收／失敗重試自檢。沿用前輪 `Build.ps1 -EmptyCargoCache`、EXE 與 Git 交付範圍，不重製 NSIS 或離線 ZIP。

- **0.8.7 使用者最新指示**：精簡側欄／對話列操作、沿用黑貓圖示，更新 Outlook 說明及品質模型可用性檢查；先執行 `scripts/Build.ps1 -EmptyCargoCache`，交付並推送 EXE、EXE 更新清單、原始碼與驗證文件。使用者確認介面後才製作 NSIS，本輪不重建 NSIS 或離線 ZIP。使用者已實測 0.8.6 VNC 可正常運作，本輪未修改 VNC 啟動流程。

- **0.8.6 使用者最新指示**：先以本機已安裝 UltraVNC 實測正式呼叫流程，再進版 0.8.6，編譯並提交／推送 EXE、EXE 更新清單、原始碼與驗證文件。使用 `scripts/Build.ps1 -EmptyCargoCache -TestVnc`；本輪先不重建 NSIS 或離線 ZIP，舊包維持歷史版本。這取代前次「僅保存本機變更」及 0.8.5 完整 NSIS 交付要求。後續確認需要 NSIS 時再打包。
- VNC 預設隱藏，只有設定 `vnc_enabled` 啟用後才提供本機頁面。固定沿用 LM_AI.exe 旁 `machines.json`／`user_config.json` 的 Python 格式；機台以原陣列順序顯示，僅手動上移／下移，不自動排序，編輯保留原位置。原生層直接啟動已安裝的 vncviewer.exe，不使用 shell，不把既有密碼傳到前端或 AI。此使用者明確要求的功能為上述「其他自動化尚未實作」的例外；細節見 docs/VNC_QUICK_CONNECT.md。

- **0.8.5 使用者最新指示**：供應商顯示改為 `Largan, Inc.`；重建 NSIS，正式安裝路徑固定 `C:\largan\LM_AI`，逐層建立缺少的資料夾、保留既有目錄及未知檔案。沿用一般使用者權限及單一 LM_AI.exe，避免 Outlook 權限差異。此次交付原始碼、EXE、Setup、兩種更新 JSON 與驗證文件，執行 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`；未要求重新製作離線 ZIP。以下舊版路徑及不製作 Setup 的約定由本條覆蓋。

- **0.8.1 使用者最新指示優先**：本次不製作 NSIS 安裝包或離線 ZIP；以 `scripts/Build.ps1 -EmptyCargoCache` 完成 v142、測試、release 與 EXE 簽章驗證後，提交／推送 LM_AI.exe、清單及對應原始碼與文件。只保留 LM_AI.exe 主程式檔名，CompanyAI.exe 停用。雙格式更新以 docs/UPDATE_0_8_1_CONTRACT.md 為準；既有安裝包與 ZIP 保持歷史版本，不稱為本次交付。未來明確要求完整包時再使用下方完整交付流程。

- 使用者要求專案包含原始碼、已編譯 EXE、Rust 離線編譯包及附屬文件，這些產物必須保持同一版本。
- 完成原始碼、依賴、設定、腳本或交付文件修改後，執行 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Prepare-Delivery.ps1`。它會更新 vendor、執行 Build、製作 ZIP，並在全新解壓目錄使用包內工具鏈、空 Cargo 快取及 `--frozen` 再次驗證。
- 成功後交付 `dist/LM_AI.exe`、相同內容的相容檔名 `dist/CompanyAI.exe`、`dist/LM_AI_Setup.exe`、`dist/MicrosoftEdgeWebView2RuntimeInstallerX64.exe`、`offline/CompanyAI-offline.zip`、ZIP 的 `.sha256`、`offline/manifest.json`、`offline/verification.json`、`offline/environment.txt`，dist/update-manifest.json、offline/installer-verification.json，以及對應原始碼與文件。
- 檢查失敗時修正問題並重新執行；不要把舊 ZIP 配上新原始碼宣稱為完整交付，也不要略過驗證。
- 依賴變更須同步更新 Cargo.lock；先取得所需 registry 套件，再用 `Prepare-Delivery.ps1 -RefreshDependencies` 更新離線包。保持固定 Rust / MSVC 基準。
- Git 儲存庫以本專案資料夾為根，不把外層共用環境或 `.local-ai` 納入。
- `.gitattributes` 已為 `dist/*.exe` 與 `offline/*.zip` 設定 Git LFS。首次加入二進位檔前必須在儲存庫執行 `git lfs install --local`；確認 LFS 可用，避免把大型 ZIP 直接當一般 Git 物件提交。
- 使用者要求提交或上傳時，先完成上述交付流程，再將原始碼、EXE、離線包、校驗與文件一起提交；確認 LFS 物件亦成功上傳。提交前檢查暫存內容，不包含 API Key、登入憑證、個人設定、target、解壓後的 toolchain/vendor、快取或暫存目錄。
- 遠端儲存庫為 `https://github.com/you1230783-sys/CompanyAI.git`，主要分支為 `main`。使用者已授權首次完整交付上傳；後續依使用者的提交／推送指示操作，明確授權可沿用。不自行更換遠端、不強制推送覆蓋歷史。
- 操作細節見 `docs/OFFLINE_AND_GIT.md`。驗證記錄描述本機測試，不能代替公司實機驗收。
## CAD 分批讀取與進度（2026-10-17，依使用者要求暫停）

- 使用者同意讀取提示、十分鐘讀取期限、單一 COM 讀取者分批累積快取，以及減少無關屬性探測；本輪修改及測試，不進版、更新 dist、封裝或推送。
- 純讀取每批 50 圖元，stderr 數字進度由桌面每秒顯示；提示非模態、沿用任務取消，成功／失敗／取消均清除。active_probe 與寫入仍維持原期限。
- 只有已知 Line／Circle／Arc 原生名稱縮減無關欄位；其他圖元保留既有讀取，修改前完整 revision 與修改後可讀屬性核對仍在。
- 分批期間核對可觀察的文件／數量／單位／DBMOD，部分結果不交付模型、不恢復失敗批次。DBMOD 不是遞增版號，同數量編輯仍需修改前完整核對。
- 用 Build.ps1 -EmptyCargoCache -ValidateOnly -BasicValidation 產生 v142 x64 隔離測試版；實測結果補記 docs/VALIDATION_LUNA_CAD_INDEX.md，不以合成測試代替 2008 大圖驗收。
- 使用者要求先停止，討論 CAD 程序內批次匯出。已停止本次原生讀圖助手、建置流程、隔離 EXE 與 localhost 8769；保留 CAD／圖面及原始碼。65 CAD 合成與新增 Rust 測試通過、release 編譯成功，但完整基礎驗證及新 GUI／Luna 測試未完成；停止進度見 .build/cad-read-stopped.json。不得自行恢復測試或發布。
## 本機模型分級驗證（2026-10-18，長期流程）

- 使用者要求將流程記入專案記憶；完整／代表性／重點三級，以及每次基本檢查，詳見docs/LOCAL_MODEL_VALIDATION.md。
- 本輪先做CAD多種類型、2008／2022的真實Strata測試。需求採一般用戶模糊口吻，必要追問以普通使用者資料補充，不先餵座標／工具格式；涵蓋日常小物、多層住宅與大小工廠。時間不趕，不以加速省略驗證。
- 每案保留初始需求、追問、原始模型參數、正式工具報告、獨立產物核對及外觀狀態。失敗保留、不自動重畫。模型／工具／幾何／視覺分別判定，模擬資料與真實環境分別記錄。
- 可用examples/cad_model_suite.rs串行呼叫正式runner測新建CAD，沒有目前圖面寫入／重連的自動確認；此入口不代表GUI驗收。測試連線／聊天／DWG留.build，不進版、打包或推送。


- **02:45最新進度**：同一session20035已完成4／14，現在陽台花架2008。桌下架2022原生49操作／49圖元（30線、10字、9尺寸），GStarCAD.Application.22，存檔重開與獨立資料PASS；SHA `962a46bab1330e82af67754901afdaea323e251ded062051bff3b9b4fa965041`，023734/桌下架子設計.dwg。真CAD最大化、全圖縮小及文字放大已覆核，VISUAL_ISSUES：字小／長中文擠疊；板深350與主機深350無前後餘量、靠右支撐但未確認散熱口、未確認腿部位置卻保證腳不會卡；側視圖實為寬度方向。圖仍開、不改幾何／補存。6輪模型450.552秒（含一次空可用輸出24.305s修復），draw193.648s／8239tokens、native約54秒，沒有length。詳細4案表已追加docs/VALIDATION_CAD_USER_SCENARIOS.md。繼續凍結產品完成圖型覆蓋，不為設計品質重啟同一案。

- **03:08最新**：完成6／14，現在small_factory-2008，session20035/current/runtime不變。花架2022 CAD_FAILED(reopen)，實際已存檔30圖元，COM268屬性相符／DBMOD0，SHA855fd3…。LISP HEADER把簡體檔名回傳反斜線U+9633／89C6／56FE，COM為原字，直接路徑比較誤拒絕；尚未修復。native-failure-review保存原始匯出及快照，真CAD畫面已看、failure-review已寫，文件留開不改／補存。全部細節與花架2008紀錄已追加docs/VALIDATION_CAD_USER_SCENARIOS.md最新段落。繼續凍結版測其餘8案，再安排Unicode檔名定向修正與原檔兩版回歸，不能忽略身分核對或改檔名繞過。

- **03:18最新**：同session20035完成7／14，現在small_factory-2022。小廠2008原生97操作／97圖元，正確Icad1、存檔重開約100秒；模型6輪422.742秒。SHA54767b…，本案031435/factory_layout.dwg。獨立確認12台2×1.5m、6桌，沒有任何原生尺寸故data.status=DATA_ISSUES；真CAD最大化已看，封閉無門隔間、DASHED全圖過密、出貨口紅線示意待釐清，visual=ISSUES。controller總狀態VISUAL_ISSUES不可誤讀為資料通過，內嵌data完整保留。全部細節已追加docs/VALIDATION_CAD_USER_SCENARIOS.md最新段。文件仍開不改。後續小廠2022→大廠雙版→住宅雙版→浴室雙版，仍不進版／dist／Git；花架2022Unicode檔名缺陷尚未修復，先完整覆蓋後定向修正。

- **03:54等待中**：仍8／14，large_factory-2008在最後既定continuation2，run_id6644142ceb25f6779f4d92b15deaab4c。t54 compact263.779秒／12715tokens後，t55 433.901秒／20000、t56 412.322秒／20000，皆無可執行工具，沒有cad-report／沒有新圖。不能把長等待當CAD讀取超時。最後一次續接03:54:08開始；待結果後讓同session20035自動進large_factory-2022，不重啟現有任務。原資料與所有下方8案證據不變。不要因重複等待就跳過驗證或提前結束工作。

- **04:58來源修正待驗證（凍結矩陣仍舊runtime7018）**：住宅2022 t79 488.991秒／20k無工具，目前continuation1自04:56:19等待，run_id7cfbb2baa6467ed5577ee6fa1fa99ab8，11／14尚未完成。不要重啟或中斷；後續浴室雙版。
- 已在等待期間修改**來源**src/cad_bulk.py：新增ansi_lisp_representation、exported_name_matches、exported_path_matches，分開比對DWGNAME和DWGPREFIX，僅從COM原字正向產生CP950外BMP字元的反斜線U+表示法；不一般解碼、不猜字、不略身分。檔名以反斜線U+開頭時原os.path.join會誤丟目錄，已避免；未知非BMP跳脫仍拒絕。未存檔名稱改splitext後同法比對。
- 已改**來源**src/cad_worker.py：native_model_bounds在TILEMODE1、worldXY前置成立且EXTMIN/MAX無效時，以全部圖元GetBoundingBox原生WCS外框合併，再既有ZoomWindow。每個Handle唯一、數量、完整合法範圍、DBMOD與目前文件、570秒預算必須通過；任一失敗停止，不跳圖元。不改正常批次讀圖路徑。尚未打成新runtime／未完整native修正流程測過，不能稱已完成建置或已修復上線。
- 原生唯讀接口原型已證明：.build/probe_house_native_bounds.py對Drawing11全部86圖元GetBoundingBox合法，low[-804,-800.5,0]、high[18000,12900,0]、Count/DBMOD不變、無寫幾何；證據本案native-failure-review/native-bounds-proof.json。最初誤用14C無效Handle唯讀查詢失敗，改快照真Handle156後成功；型別資訊確實有GetBoundingBox。
- cad-new.md補來源指引：保留未存檔圖面、僅依證據解釋範圍錯誤；明列實際幾何／簡化占位／未完成需求，核對淨距與門口，不保證承載散熱。指南仍<5000bytes。
- **85項Python CAD契約通過**：.build/run_unicode_path_unit.py從src/cad_worker.py以worker別名載入本次來源，其他固定依賴使用凍結runtime；最新log .build/cad-name-and-native-bounds-unit.log。含新增路徑反例、全原生範圍／缺圖元／DBMOD變更／重複Handle／非模型空間拒絕，以及回覆指引。這不是v142重建或新版模型流程通過。後續矩陣完成後做新來源兩版native回歸、v142正式空快取frozen建置及必要新模型定向測試。不要拿本85項說舊候選build有85；舊候選原80。

- **05:04即時**：仍11／14，住宅2022 t80 461.918秒／20k無工具，已進最後continuation2，無cad-report／無新圖。current／session20035／舊runtime7018不變。來源兩修正＋cad-new指引已改、85 Python契約通過，尚未新runtime/v142建置或完整native修正流程；詳04:58追加及docs/PENDING_CAD_Q2_OPTIMIZATION.md最新段。保持繼續等待，不重啟現有任務。
