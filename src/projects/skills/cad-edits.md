# 已讀圖元修改
edits每項{handle,kind,value}；依回傳allowed_edits，圖層鎖定停止，同Handle每批僅一項。
例如圓直徑改15：{"handle":"實際已讀Handle","kind":"radius","value":7.5}。kind是修改動作，不是circle等圖元類型；不要使用radius或set_radius當作欄位名稱。
text=文字；color=ACI1–255；move=[dx,dy]；radius=正數；height=正數；delete=null；linetype={name,scale}。不是所有圖元皆支援，尺寸／引線僅允許已讀後依需求delete重建。不猜Handle、不刪未知圖元。
需新增時用operations，無新增填[]；特殊圖形按需讀cad-geometry。副本模式存成新檔，目前圖面模式不存檔，不能互換承諾。
