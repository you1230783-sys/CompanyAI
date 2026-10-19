# 目前 CAD 圖面，不儲存

正常流程：cad_snapshot → cad_query(details=true) → cad_active_edit → finish。資料足夠就執行下一步，不為重述計畫重載技能、讀指南或重查同一圖元。

首次cad_snapshot(version=使用者指定ProgID,path=null,refresh=false)。本任務固定該版本與文件；latest只在首次選新版。已有snapshot_id就沿用；使用者確實改圖或工具明示失效才refresh=true。不逐頁讀全圖。

依總覽的closed_contours、圓位置及要求選目標，再cad_query(snapshot_id,filter,offset=0,limit=10,details=true)取得Handle與屬性。可按候選／Handle／種類／直徑／WCS XY區域直接查，不必先查details=false。recent_details是程式保留的同快照精確資料，已查過可沿用，不因整理上下文重查。封閉輪廓的bounds_center僅外接框中心；矩形不一定是底板，區域相交不是裝配干涉。filter.rectangle的圓心距邊與圓外緣間隙不同。

有多個合理目標才cad_preview(show_user=true)請使用者指認；selected_candidate／selected_handle非空就沿用選擇，不再問同一題。兩種模型都可提供使用者預覽；關閉時兩欄null不等於選了第一個。預覽X右/Y上，只是局部幾何示意。單位以units_code為準，4才是毫米。未知圖元及省略範圍須照實說明。

目標明確且已取得詳細資料後，直接cad_active_edit(revision,edits,operations)。edits每項{handle,kind,value}，同Handle一批一項。目標圓直徑D對應kind="radius",value=D/2；color=ACI1–255；text=文字；height=字高；move=[dx,dy]；delete=null；linetype={name,scale}。依allowed_edits與回傳能力操作，保留未指定屬性。沒有新增則operations=[]。基本格式已在本頁，不需要先讀cad-edits；特殊新增／指令才按需讀cad-geometry／cad-commands。

寫入由桌面另行確認；原生端重讀revision、圖層鎖與全部可讀屬性。EDITED_UNSAVED且verification通過即可finish簡短回報實際新值與未儲存，不要求save_copy。需要額外量測才刷新同一文件一次；不可沿用舊快照假裝讀回，也不可重送成功修改。revision不符時先確認差異，不能換新revision重送舊計畫。

cad_session是程式維護的執行事實，優先於舊筆記。stopped=true或unsupported_capability時finish回報實際失敗步驟與是否已修改，不重讀、不換CAD、不改副本、不重畫。editable_session=false直接回報identity_reason。純ZOOM E/A不要求Undo群組，其餘修改需要通過驗證的Undo方法。

進度筆記只記新進展，ID及精確資料已有常駐狀態，不反覆抄寫；最後答案只說工具已核對的數值。cad_active_read保留舊分頁相容入口，新任務使用快照查詢。
