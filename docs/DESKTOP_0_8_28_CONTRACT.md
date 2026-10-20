# 0.8.28：專案技能、批次操作、圖表與委派

UI：projects_collapsed／recent_collapsed 分別保存；區塊分隔，補充說明共用 DOMPurify Markdown 渲染。圖表保存在 Message.project_charts 與任務 DPAPI 紀錄，活動事件不建立額外任務通知。

技能為隨 EXE 發行的固定目錄 src/projects/skills。load_skill 只接受核准 id，內容按需加入後續 system 訊息並隨 checkpoint 保留；工具 schema 仍在基本 system，技能不擴大權限。不執行專案資料夾內技能或腳本。

search_files：明確提供 1–20 個檔案與字面查詢，沿用受控讀取／PDF 快取，不自動遞迴掃目錄。最多 60 摘錄，各帶來源版本／字元位置；讀取錯誤及截斷明示，不算完整未命中。

office_batch：1–20 個既有 Action、同一 copy_id／revision，順序執行並重建候選文件，整批成功才替換記憶體工作版本。單份累計 200 操作不變，仍要 save_copy／讀回驗證。結構變更後的 ID 按執行當下解讀。

create_chart：line/bar/scatter，1–1000 筆、1–8 系列，每任務最多 12 圖。固定標籤、數值／null 資料，拒絕其他欄位，不接受任意 JS、HTML 或 ECharts option。chart_from_excel 核對快照版本，第一列標題，第一欄 X；公式不取計算值，遇到不支援型別拒絕。自填圖表來源為模型說明，程式從 Excel 取值才有核對過的版本範圍。

ECharts 6.0.0 從 Apache 官方 tag dist/echarts.min.js 取得，授權與 NOTICE 在 ui/vendor；離線內嵌，不放寬 CSP。參考：https://echarts.apache.org/handbook/en/concepts/dataset/ 。

summarize_document：只允許 quality，重新查模型清單確認 fast 可用。固定摘要 system、原文區段與焦點，skills:false，無工具定義與遞迴委派。每段獨立遠端 conversation，序列最多 64 區段，與父任務共用取消／兩小時期限。整份摘要預算 24,000 字，各段分配上限且最多 1500 字／段，不把取得摘要當主模型已讀。

.lmai/delegation 以 DPAPI 保存；快取鍵含文件路徑／內容版本、焦點、fast alias、提示版本、帳號。後端在同 alias 換模型時不會自動辨識實際權重變更（需變更摘要 profile 或清理快取）。來源已變更不重播舊委派。每段 POST 前保存 Task；未知結果只 GET 原 ID，父任務暫停並保留已完成父回覆，續接後從子任務狀態接回。程序意外退出不自動重播，沿用既有手動處置。操作去重與 read_work_log 共用 broker。

路由不變。後端須採用完整 messages，不自行補入先前歷史。此仍是 messages 內文字 tool_calls 協定，非 API 原生工具呼叫。公司模型品質、加密與版面待實測。
