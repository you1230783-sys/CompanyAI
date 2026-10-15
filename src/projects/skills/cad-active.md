# 目前CAD圖面，不儲存
cad_active_read(version,offset=0)取得document/revision/units_code及第一頁圖元。editable_session=false直接回報identity_reason，不反覆重讀或呼叫修改。
定位所需Handle後cad_active_edit(revision,edits,operations)。每輪重新讀取，依allowed_edits與editable操作，不猜Handle；頁面使用next_offset。只有縮放不必讀完全部圖元。
桌面核對文件身分並確認後才寫入；操作時勿手動切換文件。純ZOOM E/A不要求Undo群組，其餘修改／混合操作需要完整Undo介面；unsupported_capability停止，不改用另一種寫入碰運氣。
成功EDITED_UNSAVED回報修改完成尚未儲存，由使用者自行檢查／存檔；不儲存不關閉，不改走自動存副本以規避要求。
編輯參數按需讀cad-edits；新增特殊圖形讀cad-geometry；指令讀cad-commands。
