# 既有DWG副本
優先 cad_snapshot(version,path=專案DWG相對路徑,refresh=false)，以唯讀工作副本建立本機索引並回精簡總覽。cad_query 按種類／圖層／顏色／文字／直徑／WCS XY區域篩選，details=true取得需要的精確圖元（每頁最多10筆）。候選編號綁定snapshot_id，來源變更或refresh=true後舊編號作廢。原檔不修改。
有歧義可用cad_preview(show_user=true)給使用者看局部幾何示意並指認；關閉不猜目標，選擇不是修改授權。距邊關係可用filter.rectangle指定總覽矩形編號；圓心距邊與圓外緣間隙不同。
cad_read保留舊分頁入口；已有本機快照時直接cad_query查回先前目標，不從頭翻頁。
上下文整理後依 cad_paging 的來源／revision 與 next_unread_offset 續頁，不回到已讀的 0／100。all_entities_delivered=true 後整理核對結果，缺少舊細節查 read_work_log；已讀範圍不等於內容符合使用者規格。
最多512MiB來源／20萬圖元／64MiB索引。圖塊僅名稱位置與可讀屬性，不展開定義或圖紙空間；不宣稱視覺理解整張圖。
cad_edit(path,revision,name,edits,operations)建立另一份全新副本，僅修改已讀且allowed_edits允許的Handle。儲存重開核對後交付；後續修改成果需對新路徑重新cad_snapshot取得版本。
編輯參數按需讀cad-edits；新增特殊圖形讀cad-geometry；指令讀cad-commands。目前圖面不儲存需求改用cad-active。
