# 繪圖與資料座標
小型已核對資料用create_chart；有追蹤CSV切dataset-charts用chart_dataset；Excel先切excel-read核對欄位並規劃匯出。不要手抄完整數列。明確不建CSV才用chart_excel_range，未儲存工作副本用chart_from_excel。
支援line/bar/scatter/step/area/horizontal_bar；散佈X須為數字，系列單位一致。最多10000筆／8個Y系列；超額分圖或確認範圍，不暗中抽樣。
缺值用null；0有效，原始缺值／異常不自行補零。set_chart_policy依使用者偏好調整，無效X deferred不是已完成圖。
座標平移、重新編號用transform_chart，不重抄資料；原值保留。參考線set_chart_reference_lines使用轉換後實體座標，類別軸為0起算位置，最多10條。
外觀／文字／排版切chart-edit。需要PNG用export_chart_png，修改後重新匯出；圖不等於Office成果，插入Office另切office-edit。
特殊轉換、缺值政策與Office插圖細節按需read_skill_guide(chart-details)，不必先讀完整指南。
