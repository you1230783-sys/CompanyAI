# CAD 離線實機測試

自 0.8.67 提供固定計畫入口，不需登入公司網站、不呼叫 AI。仍使用正式 LM_AI 的 Broker、版本選擇、檔案保護、CAD Python runtime 與 COM，不以獨立範例替代應用程式。

## 使用方式

先手動開啟欲測的 CAD，完成試用／授權提示，保留一份空白文件。測試期間不要切換或編輯文件。使用一般使用者執行，權限須與 CAD 相同。

```powershell
.\scripts\Test-CadOffline.ps1 -Version 2008
.\scripts\Test-CadOffline.ps1 -Version 2022
```

腳本預設使用 `dist/LM_AI.exe`；可用 `-Executable 'C:\largan\LM_AI\LM_AI.exe'` 指定完整安裝。只有 EXE 而無配套 runtime 時會失敗。`-ReportRoot` 可指定報告目錄。

底層入口是 `LM_AI.exe --cad-offline-test <絕對報告目錄> <ProgID>`。它是 Windows GUI EXE，建議使用上述腳本等待結束並擷取輸出。

## 固定範圍

1. 列出登錄版本，唯讀目前文件，再執行一次 ZOOM E；不儲存、不關閉目前文件。
2. 新建八角板：線、圓、顏色、CENTER／DASHED 線型、原生尺寸／引線、中文單行／多行文字、帶孔剖面、公式取樣曲線及 ZOOM E。
3. 另存測試 DWG、關閉重開核對；讀取工作副本，再於另一副本修改兩個圓的顏色／半徑並新增文字，存檔重開核對。
4. 全部成功才刪除本輪測試 DWG／BAK／DWL／DWL2，保留 JSON 報告。失敗停止，不重畫；可能仍開啟的部分文件留待人工檢查，不強制關閉 CAD。

固定入口只接受目錄及 ProgID，不接受任意繪圖指令或其他專案路徑。對目前文件的自動確認僅限本入口固定的單次縮放，正式應用程式的確認流程保持原樣。

`PASS_DATA_VISUAL_PENDING` 表示已通過資料核對及清理，不等於視覺驗收。未涵蓋 AI 需求理解、公司登入、任意 DWG、圖塊拆解、Excel 座標匯入、目前圖面幾何修改或完整安裝生命週期。

## 本機 2008 安裝修復

本次試用安裝缺少 ProgID／COM 型別庫登錄。使用者允許後，先保存原狀，再以原廠 `IcadAuto.dll` 登錄目前使用者的型別庫及該版本的 ProgID／LocalServer32；使用者完成初始化並重開後，COM 才可讀取應用名稱及版本。

修復限本機，不隨安裝包自動執行。程式遇到缺少型別庫時會回報 `repair_installation`，不再把它當成模型參數錯誤。不同安裝環境應先診斷，不能直接套用本機登錄。
