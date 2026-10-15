# 受限CAD指令
operations可加入{kind:"command",command:"ZOOM E"}，指令在本批幾何操作後執行。
允許ZOOM E/EXTENTS、ZOOM A/ALL、REGEN、PURGE（或-PURGE A * N）、AUDIT N。只接受固定同步介面；不開放任意SendCommand/LISP/Shell/程式載入/儲存/關閉/炸開。
PURGE清理未使用定義，不刪使用中幾何；AUDIT N檢查不修復，COM返回不是無錯誤證明。詳細診斷看CAD。
目前圖面只ZOOM E/A時仍需讀取、revision與確認，不要求Undo；有其他操作仍需Undo。缺介面直接回報unsupported_capability，不反覆試同指令或改色／移動。
只縮放不需讀遍所有圖元或工作紀錄。視圖完成不是已儲存，遵循目前圖面不儲存、不關閉規則。
