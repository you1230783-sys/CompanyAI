# 既有DWG副本
cad_read(path,version,offset=0,revision=null)複製來源後讀ModelSpace，回傳revision、統計與第一頁100圖元。用next_offset及相同revision續頁；來源變更從頭重讀。原檔不修改。
最多512MiB來源／20萬圖元／64MiB索引。圖塊僅名稱位置與可讀屬性，不展開定義或圖紙空間；不宣稱視覺理解整張圖。
cad_edit(path,revision,name,edits,operations)建立另一份全新副本，僅修改已讀且allowed_edits允許的Handle。儲存重開核對後交付；後續修改成果需重新cad_read取得版本。
編輯參數按需讀cad-edits；新增特殊圖形讀cad-geometry；指令讀cad-commands。目前圖面不儲存需求改用cad-active。
