# 文件摘要與專案筆記

可重跑流程：save_workflow只能引用本任務成功operation_id；steps逐項命名，bindings_json把既存參數JSON Pointer連到param:名稱或step:先前步驟ID:/result/欄位。新copy_id／revision必須引用新步驟結果，資料路徑與日期改成參數；不能沿用舊副本ID。list_workflows查索引；prepare_workflow(parameters_json)只列計畫，核對本次要求／資料後execute_workflow_step逐步執行，查看outcome與next_step。需要技能先load_skill，讀取後先保存閱讀成果。失敗停止，不重建流程來重播未知寫入。範本不保存權限；成功操作不等於公式與設計均通過。

記憶：先用摘要定位，重要數字、引文或修改依據再讀原文。要修改舊答案的具體內容，先 read_task_result 查全文；舊 copy_id 不跨任務重用，從成果檔建立新副本。
完整讀完後依 document.summary_needed 保存全文摘要；分段摘要按需，只摘要 sections/read_this_run 已確認讀完的區段。快取存在不代表讀過，摘要需區分來源、使用者補充與推論。
每輪在 arguments.progress_note 更新累積工作筆記，建議200–500字，記目標、仍有效的決策、已確認重點、來源版本／範圍、已完成與待辦及下一步。不記思考過程，不宣稱待執行操作已成功。下一輪以最新工具結果修正；原始結果用 read_work_log 查回。
finish／ask_user 的 arguments 可附 task_summary，簡記成果、未完成事項、使用者決定與下一步，不重複全文或額外呼叫一輪。新要求優先；持續性要求可更新 conversation 筆記。程式進度與版本是實際狀態，筆記只是摘要。

接近每段上限時依提醒提供 progress_note；達上限由桌面暫停並保存副本。使用者續接後先依實際副本版本與摘要接續，舊工具結果可用 read_work_log 分段查回，不重做成功操作。

分析方法可透過 record_analysis.report.method 保存為 project 範圍筆記；包含適用條件、步驟、核對方法、失效條件與原任務索引。同標題修訂既有版本，list_notes/read_note/update_note/delete_note/restore_note 仍可管理。下次先檢查目前樣本，才決定沿用；方法不是新的事實或授權。跨對話可讀方法筆記，舊完整任務操作仍限原對話讀取。
