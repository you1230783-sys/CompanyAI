# 目前 CAD 圖面，不儲存

先 cad_snapshot(version,path=null,refresh=false)，一次建立完整本機快照，只接收總覽。已有快照就沿用 snapshot_id，不逐頁翻完、不因整理上下文從頭掃描。使用者已手動改圖或工具回報失效時才 refresh=true；新快照的候選編號不能套用舊快照。

用 cad_query(snapshot_id,filter,offset=0,limit=20,details=false) 篩選種類、圖層、顏色、文字、直徑或 WCS XY 矩形區域。空條件可檢視其他候選；只翻符合條件的結果。snapshot 是本機保存範圍，不等於已核對所有圖元。需要精確資料時依候選編號或 Handle 查 details=true（每頁最多10筆），無須重讀前幾百個圖元。

候選編號在同一快照固定，並附 Handle。總覽的矩形僅限可確認的 XY 平行閉合輪廓；filter.rectangle 可取得同平面圓到該矩形四邊的「圓心距離」與「圓外緣間隙」，兩者不得混淆。矩形不一定是底板，圓不一定是安裝孔。單位以 units_code 為準，未知不能自行稱毫米。區域是幾何外接框相交；不可讀圖元不算已排除。

多個候選仍有歧義時，用 cad_preview(snapshot_id,candidates,region,show_user=true) 給使用者看局部幾何示意並指認。非空 selected_candidate／selected_handle 表示使用者已選定，不再重問指認；原要求已指定修改時，取得詳細資料後呼叫 cad_active_edit，寫入仍由原生端另外確認。兩種模型都能提供使用者預覽；關閉時兩欄為 null，不等於選了第一個。預覽 X右/Y上，非目前螢幕方向，不呈現完整 CAD 外觀。未知圖元及省略範圍須照實說明。

修改前 cad_query(details=true) 取得目標詳細資料，再 cad_active_edit(revision,edits,operations)。只取必要圖元，不把完整圖面抄入筆記。圓直徑15對應radius=7.5；保持圓心與其他未指定屬性。每輪筆記保留目標、已確認事項、snapshot_id、revision及Handle；精確資料由查詢取得。

快照不是即時畫面：查詢輕量核對文件身分／數量／單位，不能偵測所有同數量的手動修改。寫入前原生端仍完整重讀可讀屬性、核對revision及桌面確認。若不符，停止寫入並重建快照，不自動換用新revision重送舊計畫。修改完成後舊快照作廢，下一輪重新取得。

editable_session=false直接回報identity_reason，不反覆讀取或修改。unsupported_capability停止。純ZOOM E/A不要求Undo群組，其餘修改／混合操作需要完整Undo介面。成功EDITED_UNSAVED只表示尚未儲存的修改，不儲存、不關閉、不自行改走副本。

第一次修改前先讀cad-edits確認handle/kind/value格式；新增特殊圖形讀cad-geometry；指令讀cad-commands。cad_active_read保留舊分頁相容入口，新任務使用快照查詢。
