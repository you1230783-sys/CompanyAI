# 文字搜尋與副本修改

find_text 定位後，以 create_working_copy 取得真實 copy_id 和 revision。edit_text 同時核對版本、位置與預期原文；版本變動先重讀。插入使用空 expected，刪除使用空 replacement。最後 save_copy，只交付成功發布的版本。
TXT 維持 TXT，既有 MD 維持 MD；一般成果預設 TXT。README.md 僅放操作說明。來源 DOC/PDF/MSG 的純文字輸出使用 TXT，不冒稱保留原排版。修改原 Office 格式先載入 office-edit。
search_files 可搜尋多個明確指定檔案；回傳位置、摘錄與版本，未讀成功不等於沒有命中。不自動搜尋 .lmai。delete_copy 只刪本次可刪工作副本，不刪使用者原檔。
