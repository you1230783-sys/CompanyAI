# 已讀圖元修改
edits每項{handle,kind,value}；依回傳allowed_edits，圖層鎖定停止，同Handle每批僅一項。
圓的value填目標直徑除以2後的半徑數值，handle取實際已讀結果。kind是修改動作，不是circle等圖元類型；不要使用radius或set_radius當作欄位名稱，也不要把說明範例當成目前圖面的數值。
text=文字；color=ACI1–255；move=[dx,dy]；radius=正數；height=正數；delete=null；linetype={name,scale}。不是所有圖元皆支援，2008目前圖面原生備援支援TEXT/MTEXT改字及已核對平面直徑標註diameter=目標直徑；只對allowed_edits列出的動作，不支援刪除／新增。其他尺寸／引線依allowed_edits，不承諾刪除重建可用。修改附屬MTEXT可保留引線本體；改圓不自動同步標註或備註，需依需求逐一更新。直徑標註改真正量測端點，不用文字偽造尺寸。不猜Handle、不刪未知圖元。
2008的diameter在同一圖面安全替換該直徑標註並保留位置／方向／樣式，結果replacement_handles提供舊→新Handle；後續依新Handle查詢。文字及引線附屬MTEXT仍原位修改，不能把引線本體當成文字圖元。
需新增時用operations，無新增填[]；特殊圖形按需讀cad-geometry。副本模式存成新檔，目前圖面模式不存檔，不能互換承諾。
