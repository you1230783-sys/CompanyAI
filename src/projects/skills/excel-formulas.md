# Excel 自訂公式

先分清「原始資料」與「公式」：write_excel_formulas只能寫=開頭的真正公式，不能寫表頭、文字、一般數字或空白。禁止為了通過檢查把「品項」改成「=品項」；這會產生#NAME?。

本技能已同時提供office_action／office_batch寫入原始資料，不必切換或重複load_skill。先用office_action(copy_id,revision,operation={kind:"excel_write",sheet:1,cell:"A1",rows:[["品項","金額"],["甲",10]]})寫入資料（實際值依使用者要求）。成功後沿用同一copy_id及新revision，以write_excel_formulas僅寫目標公式格。每次rows必須等寬；不同位置分開操作，不能把不同寬度的資料與公式拼成一個矩形。新建活頁簿已有第一張工作表，sheet=1，不是0，不需新增同名表。不要重建副本、不要在錯誤公式尚未修正時save_copy。

依使用者要求自行編寫Excel A1公式，不限預設函數。先create_working_copy建立或複製xlsx，再read_file核對真實工作表、欄位及最新revision；write_excel_formulas(copy_id,revision,sheet,cell,rows,dynamic)一次寫等寬公式矩形。sheet為1起算，cell為左上角如C2，rows如[["=SUM(A2:B2)"],["=SUM(A3:B3)"]]。每個字串需以=開始；不可把公式寫成文字或用edit_office改公式。

預設dynamic=false使用Formula；需要動態陣列才dynamic=true使用Formula2，不支援時會明示失敗。不改巨集、合併格、既有陣列公式、外部工作簿、DDE或網路資料連線；原檔保留。每批最多2000格，目標限前1000列100欄，整個UsedRange也不可超過2000格。

工具以Excel真正計算，calculation列出公式、值、顯示及error。檢查每個目標格與相依公式；error=true（如#DIV/0!、#SPILL!）不是計算成功，需修公式或如實說明。NOW/RAND等易變公式每次計算可能不同，不能承諾值固定。完成save_copy後交付；資料與公式都可在本技能完成，沿用同一copy_id及最新revision。
