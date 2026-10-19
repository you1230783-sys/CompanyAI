# 原生幾何、圖層與圖塊

新增圖元：
- polyline：points為2–512個XY頂點、closed為布林；封閉不重複首點。是原生直線輕量多段線，非取樣曲線。
- arc：center/radius/start_angle/end_angle；角度單位為度，自正X逆時針，不能以同角度表示整圓。
- radius_dimension：center/chord/leader_length。chord是圓上點，量測真實半徑，不以文字偽造。
- angular_dimension：vertex/start/end/location；start/end定義兩條射線，location在小角內；只支援0–180度小角，不支援反角。
- layer：name/color/linetype，建立本輪新圖層，不覆寫既有定義。先定義再在圖元填layer名稱；層線型不能BYLAYER/BYBLOCK。圖元仍有明確ACI顏色，想依圖層顯色需另輪擴充BYLAYER顏色。
- block_definition：name/base/entities；entities限1–50個line/circle/arc/polyline，不巢狀、不讀外部DWG。定義不會在ModelSpace出現。
- block_reference：name/position/scale/rotation/color；只插入本輪定義，統一正比例縮放，rotation為度。原生圖塊可重複插入，不能用名字載入外部檔案。
所有可繪幾何可指定layer；arc/polyline也可指定line_style。若當輪schema列可選欄位required且接受null，未指定填null，由程式還原省略。建立圖塊、線型、圖層與尺寸都需存檔重開核對；新增到2008目前圖面仍須依edit_capabilities，不宣稱通用新增已開放。

# 已讀既有圖元的幾何修改

cad_active_edit或cad_edit的edits每項仍為{handle,kind,value}。先cad_query(details=true)取得所有主／副目標Handle與allowed_edits。每批同Handle僅一次；新幾何修改與改字/半徑/新增operations分批，不混合兩套Undo路徑。目前支援WCS Z=0的直線／圓／弧／直線輕量多段線；傾斜、3D、TrueColor、反應器或擴充字典保留不動。
- copy：value={offset:[dx,dy]}，保留原圖元。
- rotate：value={base:[x,y],angle:度}，修改原圖元。
- mirror：value={axis_start:[x,y],axis_end:[x,y],copy:布林}；copy=false改原圖元，true保留原圖元並新增鏡像。
- array：value={rows,columns,row_spacing,column_spacing}，WCS XY矩形陣列，含原圖元總數2–100；不重複原點副本。
- offset：value={distance:非零數}，限直線與圓；直線正值在起點往終點的左側，圓正值向外，負值向內；新增副本，不移動原物。
- trim：value={boundary:第二條直線Handle,pick:[x,y]}；兩線段有唯一內部交點，pick靠近要剪掉的端點。
- extend：同trim格式；pick靠近要延伸的端點，交點須在該端點外，且在邊界線段上。
- fillet：value={other_handle:第二條直線Handle,radius:正數,pick:[x,y],other_pick:[x,y]}；pick與other_pick各自靠近要保留的端點。裁短兩線並建立相切原生圓弧；半徑過大、平行或有歧義則整批寫入前拒絕，不猜保留側。
- layer：value=已存在且未鎖定的圖層名稱。只改圖元歸屬，不改圖層定義／鎖定狀態。

需要新圖的重複構件時優先原生圖塊重複插入；array等Handle修改工具用於已有圖元。所有操作核對全部可讀屬性及未指定圖元，部分結果或未知回條不重送。一次Undo涵蓋本輪修改；後續CAD操作另依原生Undo順序。這些是有界2D幾何工具，不是任意TRIM/FILLET指令、通用曲面或3D編輯器。
