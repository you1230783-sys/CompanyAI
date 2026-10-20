# 0.8.64：CAD 原生線型與純縮放

使用者於 2026-10-15 要求先修改線型功能，同時在公司測試 0.8.63 目前圖面操作。
後續使用者已授權進版 0.8.64、編譯、NSIS 與推送 main。基礎驗證結果見 VALIDATION_0_8_64.md；公司 CAD 實機與完整驗證待補。

## 併入：目前圖面純縮放

使用者實測 0.8.63 的 iCAD 2008 目前圖面讀取成功，process_and_document_snapshot 取得 editable_session=true，Document.ActiveViewport 的 ZOOM 能力檢查通過。但 StartUndoMark 不存在，故指令尚未執行，不能視為縮放通過。

目前文件僅包含 ZOOM EXTENTS／ZOOM ALL 且沒有任何 edits 時，改走不要求 Undo 群組的視野操作。仍經桌面確認、文件身分／revision 檢查、每項指令前後身分核對及 ModelSpace／PaperSpace 可讀內容核對；不儲存、不關閉，也不宣稱可一次 Undo。混合計畫、圖元修改及其他維護指令不放寬。一般修改在開始前確認 StartUndoMark／EndUndoMark 都存在，診斷步驟明示 prepare_undo_group。

待驗：2008 純縮放無 Undo 介面仍可執行；混合操作不能繞過 Undo；文件切換／內容變更須阻擋；縮放錯誤或圖元核對不符不能報成功。尚未驗證公司原生縮放實際顯示效果。

## 操作介面

- 新增 line／circle／curve 可選 `line_style: {"name":"DASHED","scale":1}`。
- 已讀取的 line／circle／arc 可使用 `edits: [{"handle":"156","kind":"linetype","value":{"name":"CENTER","scale":1}}]`。
- 支援 CONTINUOUS（實線）、DASHED（虛線）、HIDDEN（隱藏線）、CENTER（中心線）、DASHDOT（點畫線）、DOT（點線）、BYLAYER、BYBLOCK。name 不分大小寫。
- scale 為圖元線型比例，範圍 0.000001–1000000。省略整個 line_style 時不覆寫原生預設。
- 不改動全圖 LTSCALE／目前預設線型，不修改圖塊內部、尺寸或引線的線型。公式曲線仍為取樣線段，每段重新起算線型，短段可能無法顯示完整虛線。

## 載入與核對

先沿用文件已有同名線型。缺少時只從 CAD 執行檔所在目錄及 Support 子目錄的標準 icadiso.lin／gcadiso.lin／acadiso.lin／icad.lin／gcad.lin／acad.lin 尋找指定名稱，不接受模型傳入路徑或任意定義。找不到時提示先在 CAD 載入；不偷偷改成實線，也不改寫同名定義。

線型準備在幾何修改之前；目前圖面載入仍包含於 Undo 群組。每個圖元寫入 Linetype／LinetypeScale 後讀回，納入既有存檔前後屬性核對。讀取目前圖面／副本時也回報可讀線型屬性。缺介面或回讀不符仍回報錯誤，不宣稱成功。

下一版基本驗證應覆蓋：省略線型的舊計畫、名稱／比例驗證、已有線型不載入、不存在標準檔、載入失敗、讀回不符、新圖／副本存檔重開、目前圖面不儲存及 Undo；2008／2022 的比例與虛線外觀另外實測。

介面參考：[IntelliCAD Load](https://www.progesoft.com/products/progecad-professional/manual?mp=developer-reference%2Fapi-reference%2Fmethods%2Fload-method)、[LinetypeScale](https://developer.bricsys.com/bricscad/help/en_US/V16/DevRef/source/Properties_LinetypeScale.htm)。文件不等於已驗證 iCAD 2008／GstarCAD 2022 相容。
