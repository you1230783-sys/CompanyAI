# Excel座標繪圖



1. 先 inspect_excel 取得 Excel revision／表頭／樣本，確認 X/Y、半徑或標籤欄、列範圍、來源單位及目標單位。不能由列號或欄名猜測實際數值。
2. cad_excel_plan(path,revision,settings) 在本機讀取原資料並產生計畫，模型不手抄數列。settings 全部欄位如下：sheet/header_row/start_row/row_count、x_column/y_column、radius_column（無則 null）、label_column（無則 null）、kind="circles" 或 "lines"、radius（無半徑欄時預設半徑）、scale、origin=[x,y]、color、height、closed、units_code。
3. 座標 = Excel 數值 × scale + origin；半徑同乘 scale，height 已為目標圖面單位。units_code 1英吋/4mm/5cm/6m；新圖固定4。初版不接受未知單位0，請使用者先在 CAD 設定並重讀。circles 不可 closed；lines 不用 radius_column，依列順序連線，可 closed=true 首尾閉合。
4. 每次至多100列，展開後至多100個操作（圓+標籤為每列2項）；無效、空白、重複相鄰點、負半徑或超額時整批拒絕，不補0或暗中略過。標籤位於座標點，需依需求另行排版；目前不提供自動避讓。
5. 先向使用者說明欄位與轉換預覽，需求已明確時可直接套用。cad_excel_apply(plan_id,target,path,revision,name,version)：target=new 時 path/revision=null；target=active 時先 cad_active_read，revision 為圖面版本、path=null；target=copy 時先 cad_read，提供該 DWG path/revision。name 為副本單一.dwg檔名，active 不使用名稱可填空字串；version=latest 或確切 ProgID。
6. apply 再核對 Excel 原檔版本與目標圖面單位；一次性消耗計畫，失敗不可直接重播。new/copy 會保存新檔；active 僅修改目前圖面、不儲存。圖面與來源版本變更需重新讀取／預覽。單位、欄位與原點由使用者意圖確認，工具不猜測。


本技能已提供inspect_excel及目標圖面讀取，不必先切其他技能。只套用座標計畫；一般修改切cad-active/cad-copy，新建自由圖形切cad-new。
