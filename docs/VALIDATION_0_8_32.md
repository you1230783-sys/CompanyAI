# 0.8.32：Office 圖片、進階診斷與工具可用條件

本批依使用者最新指示進版、編譯 EXE 並發布到既有 Git main；不製作 NSIS／離線 ZIP。

## Office PNG

沿用 `office_action`／`office_batch`，增加下列操作，不增加 API 路由或資料庫欄位：

```json
{
  "kind": "insert_image",
  "path": "_AI_Output/20261001_190000/亮度趨勢.png",
  "width": 320,
  "target": { "kind": "word", "before": null }
}
```

- 操作外層仍需本次工作副本的 `copy_id`、最新 `revision`。先以 `export_chart_png` 產生圖片，或使用專案內既有 PNG。完成後呼叫 `save_copy`。
- `target`：Word 為 `{kind:"word",before:null}`（文末），或指定正文段落 ID；Excel 為 `{kind:"excel",sheet:1,cell:"H2"}`；PPT 為 `{kind:"ppt",slide:1,left:40,top:100}`。
- 寬度、PPT 座標以點為單位（72 點＝1 英吋），高度由原像素比例計算；寬度 12–1200 點，Word／PPT 另檢查頁面邊界。Word 插入獨立段落中的行內圖片；Excel 為跟隨儲存格移動、不隨列欄縮放的浮動圖片。文字不會自動避讓 PPT 圖片，仍需確認版面。
- 圖片嵌入，不建立外部連結。來源限專案內 PNG：8 MiB、8192×8192 以內、合計 1600 萬像素；不讀網址、不做圖片理解、OCR 或 JPEG／SVG 轉換。
- 共用目錄邊界、檔案鎖、拒絕重新解析點／硬連結。CRC／容器／尺寸先核對；SHA256 由 broker 管理，模型不得提供。接受操作後，每次重建均核對圖片來源版本；PNG 在發布前變動或移除就拒絕，發布後文件不依賴原 PNG。
- 圖片數量、尺寸、位置與來源標記納入 Office 結構快照，因此插圖也會改變 revision；保存及交付重新開啟核對。批次失敗不提交候選版本。
- 隱藏 Word 文件沒有完整視窗物件時，圖片 `LockAspectRatio`／`AlternativeText` 會回 COM E_FAIL。本版建立文件視窗物件但保持 Word Application 隱藏；已用實際 Word 確認。

實作依據：[Word AddPicture](https://learn.microsoft.com/en-us/office/vba/api/word.inlineshapes.addpicture)、[Excel AddPicture](https://learn.microsoft.com/en-us/office/vba/api/excel.shapes.addpicture)、[PowerPoint AddPicture](https://learn.microsoft.com/en-us/office/vba/api/powerpoint.shapes.addpicture)。

## 診斷與工具選擇

- 執行紀錄僅在「設定 → 進階功能」（預設收合）提供；可選目前對話的執行中或歷史任務，保留更新／複製功能。
- 快速模型的原生工具清單移除 `summarize_document`；執行器原有品質模型限制保留。
- 新請求依 broker 真實狀態過濾：無圖表不提供 PNG 匯出，無副本不提供修改／存檔／刪除，文字副本與 Office 副本各自提供對應編輯工具。待查回的舊請求保持原樣，避免破壞去重。
- 空白 `copy_id`／`revision` 在原生解析層回可修復工具結果，提示先建立副本或閱讀；沿用有限修復次數，不執行空白修改，不虛構副本。
- 提示詞明確要求清單任務完成後呼叫 `finish`，不自行測試匯出或修改。這些是降低無效工具選擇的措施，不能保證模型永不偏題。
- 使用者第二份紀錄已含逐輪 Task ID／回覆，證實新診斷可見；第一份缺少逐輪欄位符合舊紀錄形狀，不能据此宣稱網站遺失資料或還原未保存的內容。

## 原生回覆白名單

使用者收到網站錯誤 `UPSTREAM_INVALID_RESPONSE`（`choices[0].message.provider_specific_fields`），並已要求網站只驗證白名單欄位。桌面本版同步採取以下規則：

- result、choice、assistant、tool_calls 項目及 function 封裝中的未知欄位一律忽略，不轉成 content、不參與工具選擇，也不回填下一輪訊息。TaskStatus 與 context 附加 metadata 同樣不影響已知欄位解析。
- 下一輪 assistant 只使用 `role/content/tool_calls`；每個 call 只保留 `id/type/function.name/function.arguments`。arguments 字串原樣保留，參數本身仍按本輪工具 Schema 驗證；不靜默刪除未知操作參數。
- 必要欄位仍須存在且型別正確，`content:null` 合法但缺少 content 不合法；請求／對話／專案／輪次等身分逐一核對，工具結果仍依 call ID 配對。
- `refusal`、`finish_reason` 是已知控制欄位，不能當成額外 metadata 忽略。拒絕、截斷、工具數量或結束原因矛盾仍不能執行工具；傳輸大小與重複 JSON key 檢查保留。
- 沒有新增路由或資料庫欄位。網站必須先回正常 TaskStatus/result；如果仍回 UPSTREAM_INVALID_RESPONSE，桌面不會把錯誤當作成功。實際需續接的 provider 私有狀態仍由網站 adapter 管理，或另行協商擴充，桌面不猜測。
- 快速模型重複工具仍須以實際模型聯測；若持續發生，網站應核對送往模型的 assistant.tool_calls、role:tool、tool_call_id 及結果是否完整。不能僅由目前紀錄判定是模型品質或轉送歷史問題。

本儲存庫未修改網站程式；使用者已安排後端修改，但本機無法驗證公司服務是否部署完成。

## 驗證

Office 驗證腳本改在獨立 PowerShell 程序執行，確保測試用 COM 包裝物件全部釋放後才進行 PNG 測試。原先同程序串接時曾因殘留 PowerPoint 參照而被應用程式正確阻擋；此為測試生命週期問題，應用程式原有的使用中簡報保護未放寬。

執行 `scripts/Build.ps1 -EmptyCargoCache -TestOffice` 完整通過，使用 MSVC 14.29.30133／v142 x64，實際編譯器 `_MSC_FULL_VER=192930159`。

- 格式檢查、Clippy（禁止警告）、189 項 Rust 單元測試通過；空 Cargo 快取搭配本地 vendor 的 `cargo build --release --frozen` 成功。
- 白名單測試涵蓋各層額外欄位忽略、不回填歷史、缺少必填欄位／錯誤型別拒絕、工具參數 Schema 保留，以及委派純文字回覆。原生 HTTP case 0 實際帶額外欄位完成讀取、複製、修改、存檔與交付，後續訊息僅含允許欄位。
- WebView2 DOM 自我檢查通過，包含預設收合的進階設定、目前任務選擇與開啟執行紀錄。原生代理 11 個情境、舊協定、長任務暫停／續接、PDF 與 AppContainer 隔離回歸通過。
- 實際 Word／Excel／PowerPoint 共 10 種新舊格式讀取、修改、另存及重開通過；新建文件、結構／格式、批次失敗回復，以及 XLS／XLSX 非連續 A／F 欄、100 列分頁、10000 點圖表數值與缺值保留通過。
- PNG 經正式 broker 插入 DOCX／XLSX／PPTX：新建與既有文件副本、版本變更、重複操作去重、越界及來源變更拒絕、另存與原件 bytes 不變均通過。移除原 PNG 後，六份成果仍能以 Office 重新開啟並核對圖片；另確認封裝含 media，圖片關聯不是外部連結。
- EXE 與簽署更新清單版本、大小及 SHA256 一致。工具鏈與報告見 [EXE 驗證](../offline/exe-verification.json)、[Office 驗證](../offline/office-verification.json)。未製作 NSIS 或離線 ZIP。

公司加密環境、實際 fast／quality 模型及網站回應正規化仍需公司環境聯測；不宣稱本機測試已驗證真實模型的任務品質。
