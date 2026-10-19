# 新建CAD圖面：分段保存，同圖提交
新設計在獨立DWG完成，不先讀舊圖找空白。集中確認用途、可用空間、安裝及必要尺寸；排版合理假設，不逐座標追問。現場固定、承載等未知須列待確認，概念圖非加工／施工核准。
自撰中文採繁體。2008字碼頁不支援會建圖前拒絕；只補正自撰字詞，使用者原文須詢問，不改問號或翻譯。manual_check／blocked_after_error須finish回報實際步驟及存檔狀態，保留未存檔圖面供核對。不推論圖面超出CAD能力，不關閉殘圖、ASCII重畫或換版繞過。
version依共用規則；沿用selected_cad固定版本，不重複載入技能或已讀指南。
按階段儘早交工具，不在單輪想完整張圖：
1. 必要條件未齊先ask_user，沿用已回答內容。
2. compact_context以200–400字記單位、主尺寸、視圖位置、構件數量、假設、預定區段；不展開operations。簡單圖1–2句。
3. cad_plan_start(name,version)建立同一張圖的本機計畫。
4. **本輪只產生下一小段1–20項**，立即cad_plan_part(plan_id,plan_revision,part_id,summary,operations)。依構件或視圖拆段，超過20項拆成_a/_b；不要先生成其餘區段或整圖再拆。每次成功程式會保存精確參數並提供新plan_revision及區段清單，下一輪只做下一段，勿重送已完成段。
5. 幾何、必要尺寸及註記齊備後cad_plan_commit(plan_id,plan_revision)，不再抄operations。程式合併後一次建立**同一張DWG**、存檔重開。暫存區段尚未繪圖，也不代表已存DWG。
整圖最多40段、400項及500KB。cad_plan_inspect指定part_id可取單段，修正以相同ID及最新plan_revision整段替換；其餘段不變。錯誤依指定區段改，不重送全圖。舊cad_draw每次另開新圖，不用它追加。
recent_details常駐最近至多4段完整參數（32KB），直接沿用，不反覆inspect；缺段才查回。省略ID見recent_details_omitted，原資料不刪。
核對含支撐、背板與墊片的整體外廓，計算總突出與寬高。孔邊及孔心距離分清。矩形外框需4項line；沒有折線、圖塊或陣列操作。保留要求數量，標籤不能替代設備；能力不足明示未完成。
line／circle／curve的line_style若required，未指定填null，指定{name,scale}；null還原實線。尺寸與文字留區域，勿遮擋。尺寸／引線可每段1–3項，不必湊滿20項；收到輸出額度續接提示就直接交下一小段，不重算整圖。
回報實際幾何、簡化占位及未完成需求；核對真實長寬、淨距與門口，不保證未驗承載、散熱或通行。成功DWG程式自動交付，finish.artifacts填[]；分開回報資料核對與待目視字型／排版。
引線、剖面、曲線或線型按需讀cad-geometry；指令讀cad-commands。方法閱讀不切技能，不先讀全部指南。目前圖面修改用cad-active，磁碟DWG副本用cad-copy，表格座標用cad-excel。
