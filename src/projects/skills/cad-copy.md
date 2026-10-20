# 既有DWG副本
副本可填確切ProgID或已安裝年份，程式由inventory解析，年份不存在或不唯一就拒絕，不猜ProgID後綴。同任務沿用resolved_copy_version；latest/auto/最新版固定為首次成功版本，不改選其他CAD。cad_query直接單獨呼叫，不能包在run_batch或改成read_code_section。
優先 cad_snapshot(version,path=專案DWG相對路徑,refresh=false)，以唯讀工作副本建立本機索引並回精簡總覽。cad_query 按種類／圖層／顏色／文字／直徑／WCS XY區域篩選，details=true取得需要的精確圖元（每頁最多10筆）。候選編號綁定snapshot_id，來源變更或refresh=true後舊編號作廢。原檔不修改。
有歧義可用cad_preview(show_user=true)給使用者看局部幾何示意並指認。非空 selected_candidate／selected_handle 表示使用者已選定，不再問同一個指認問題；原要求已指定修改時，先讀 cad-edits、取得詳細資料，再呼叫 cad_edit，保留原生端既有確認。關閉時兩欄為 null，不猜目標。距邊關係可用filter.rectangle指定總覽矩形編號；圓心距邊與圓外緣間隙不同。
cad_read保留舊分頁入口；已有本機快照時直接cad_query查回先前目標，不從頭翻頁。
上下文整理後依 cad_paging 的來源／revision 與 next_unread_offset 續頁，不回到已讀的 0／100。all_entities_delivered=true 後整理核對結果，缺少舊細節查 read_work_log；已讀範圍不等於內容符合使用者規格。
最多512MiB來源／20萬圖元／64MiB索引。圖塊僅名稱位置與可讀屬性，不展開定義或圖紙空間；不宣稱視覺理解整張圖。
cad_edit(path,revision,name,edits,operations)建立另一份全新副本，僅修改已讀且allowed_edits允許的Handle。儲存重開核對後交付；後續修改成果需對新路徑重新cad_snapshot取得版本。
原生修改結果含comparison_available時，用export_cad_comparison(source_operation=實際修改操作ID,name="修改比較.txt")保存TXT及JSON清單，包含新增／刪除／屬性前後值。尺寸固定文字及geometry_consistency=unverified必須明示待確認，不把Measurement自身核對當成與圓／孔關聯通過。無比較資料不可從計畫猜結果；匯出報告失敗不重做修改。
編輯參數按需讀cad-edits；新增特殊圖形讀cad-geometry；指令讀cad-commands。目前圖面不儲存需求改用cad-active。
修改前一次檢查：主／副目標details已交付、每個修改Handle每批一次、幾何批次operations=[]。PRECHECK_REJECTED尚未開啟CAD修改，依issues一次修正；FAILED／UNKNOWN依原生recovery決定，不重播未知結果。
同快照detail_delivery記全部已讀範圍，recent_details保存精簡精確幾何；常駐清單略過不等於未讀。需要最終獨立核對時只讀必要項目一次，保存結論；筆記原文移出不是重新查全部圓／線／弧的理由。各步與存檔核對完成便finish，未確認項目明列，不為報告排版延長查核。進度筆記只寫新結果／未完成步驟，不每輪重抄整個計畫或座標。
