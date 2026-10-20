# 圖表編輯

1. inspect_chart(message_index=null,chart_index=null)列出本輪及同對話歷史圖表索引；依使用者指的圖選擇，若不能確認就詢問。message_index=null表示本輪，數字表示歷史訊息索引。
2. 指定message_index/chart_index取得完整style、來源摘要及筆數，不傳點陣。歷史圖的使用者最新設定優先。
3. edit_chart(message_index,chart_index,style)沿用回傳樣式，只改使用者指定欄位。不要重讀Excel、重新手抄數據或生成相同圖。歷史圖另建本轮版本，舊訊息保留；後續用回傳chart_index與message_index=null。
4. 支援title/x_label/y_label、六種kind、legend、四個範圍（null自動）、series名稱／#RRGGBB顏色、lines、transform、quality_policy，以及layout。水平長條X是量測值、Y是類別；換圖型需同步調整標題、轉換與參考線。缺值切換只作用於已有原始異常紀錄。
5. layout.title/legend/reference_labels使用圖框左上角0–1比例座標{x,y}；null讓程式自動排版。reference_labels按lines順序，移除線須同步移除位置。
6. layout.annotations最多20則，每則text為1–500字，可換行；position={x,y}為比例位置；font_family可選sans-serif、Microsoft JhengHei、PMingLiU、DFKai-SB、Arial、Times New Roman、Consolas；font_size整數8–72；bold/italic/underline布林；color為#RRGGBB，background為#RRGGBB或null透明。字型未安裝時用系統備援。不可HTML、ECharts option或腳本。
7. 文字位置只控制呈現，不等於資料座標；縮放不移動參考線數值。使用者可在調整排版拖曳文字。圖表較小可能避讓文字；不要宣稱每種尺寸完全相同。
8. 圖表原始點陣與来源唯讀，PNG不覆寫；需要新版圖片時再export_chart_png。樣式會隨本輪成果、續接及加密歷史保存。
