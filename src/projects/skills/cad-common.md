# CAD 共用規則
本輪版本指定優先，可沿用記憶偏好，未指定用latest。cad_plan_start自動解析並固定版本，無需額外inventory；其他入口依schema。版本不存在／不明不換版。支援浩辰GstarCAD／佑達iCAD；連線由桌面提示，保留計畫。
新圖mm；既有圖依units_code，未知不猜。name是單一.dwg檔名；新建／副本不覆寫原檔。目前圖面不儲存、不關閉。每任務最多6次原生寫入；新圖分段每次1–20項、累積最多400項，commit才寫入；其他新增入口仍最多100項。最多4000圖元。
依工具schema提供參數。ACI顏色1紅2黃3綠4青5藍6洋紅7白/黑8深灰9淺灰；非RGB。直徑文字用%%c12。標註／引線字高3、箭頭3、偏移1、ISO、simplex.shx；中文字形仍需目視。
completed_operations只代表呼叫返回；intended_path不是成果。saved=false/file_exists=false不宣稱磁碟有DWG。成功path由桌面自動交付，finish.artifacts不填DWG路徑；EDITED_UNSAVED只表示記憶體修改完成。核對不等於視覺驗收。
失敗以recovery為準：correct_and_retry先修參數；revised_new_document可修改計畫另建；user_requested_new_document僅按桌面明確同意另建；manual_check/blocked_after_error停止。unsupported_capability不重送同呼叫，不重讀同報告；部分圖面保留人工檢查，不擅自關閉／撤銷。
3D、圖塊展開、輪廓文字辨識不支援；圖塊僅摘要。不要用未實作能力替代承諾。
