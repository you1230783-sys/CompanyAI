> **0.8.42：離線 Python 分析與完整安裝包。** 新增 pandas／NumPy／openpyxl，支援 CSV／LOG 統計及 XLSX 成果；公司原始 Excel 仍經 Excel COM 讀取。請使用本版 NSIS 安裝。詳見 [驗證與限制](docs/VALIDATION_0_8_42.md)。

> **0.8.41：圖片請求關聯與閱讀位置修正。** 圖片／快速摘要沿用父請求對話；圖片最大5 MB、每次任務1張，快速模型不公告或執行圖片工具。後續只帶文字重點；工具紀錄追加時保留閱讀位置。詳見 [驗證與限制](docs/VALIDATION_0_8_41.md)。

> **0.8.40：Excel 欄位核對與圖表自訂。** 時間／X／Y先規劃再本機篩選。完成圖表可雙擊或按「編輯圖表」，調整標題、軸範圍、圖例、色彩與參考線；可恢復原樣，或直接存PNG至_AI_Output。類別軸使用實際標籤（時間請含完整秒數），重複標籤可用 #資料序號。新增階梯線、面積及水平長條。詳見 [本版驗證與限制](docs/VALIDATION_0_8_40.md)。

# LM_AI — Windows 工作助理

CompanyAI 是 LM_AI 的原始碼專案。LM_AI 連接公司 AI 服務，提供日常問答、文件整理、專案副本修訂與 Classic Outlook 郵件分析。

**目前 EXE／NSIS：0.8.42** · Windows 11 x64 · [版本更新紀錄](CHANGELOG.md)

## 下載與開始使用

1. [下載完整 LM_AI_Setup.exe](https://media.githubusercontent.com/media/you1230783-sys/CompanyAI/main/dist/LM_AI_Setup.exe)。先從系統托盤選「離開」，再以一般權限安裝至 `C:\largan\LM_AI`；Python 會放在旁邊的 `python` 子目錄。
2. 開啟程式，按「登入」，在瀏覽器核對短碼並允許授權。
3. 選擇模型後開始對話；需要讀取或修改資料夾內的文件時，先建立「專案」。

執行需要 WebView2 Runtime，不需安裝 Rust、Node 或 Python。若啟動時提示缺少 WebView2，請依 [離線安裝說明](dist/WEBVIEW2-OFFLINE.md) 處理。正式功能需要公司服務連線與桌面帳號權限。

本版 EXE 與 NSIS 都是 **0.8.42**。Python 分析需要完整安裝目錄，請不要只下載或移動 EXE。歷史 `CompanyAI-offline.zip` 未更新；新增的 `python-inputs.zip` 僅供開發者離線重建 Python 環境。

## 可以做什麼

| 功能 | 用途 |
| --- | --- |
| 一般對話與附件 | 問答、翻譯、摘要、潤飾，支援串流與背景處理。 |
| 專案文件工作 | 支援本機、映射磁碟及 UNC；可在下載／桌面建立預設資料夾，成果可點擊定位。 |
| 圖片辨識（試驗） | 專案 JPG／PNG 交給目前模型；每次任務1張、最大5 MB，辨識後只保留文字重點；快速模型不支援。 |
| 生成週報 | 建立素材資料夾、設定日期與補充、再次確認後整理文件及授權郵件；可參考上週格式。 |
| 專案技能與圖表 | 論文證據、週報增量、Excel 抽取、跨文件搜尋、批次 Office、離線圖表與快速模型摘要委派。 |
| Python 分析（試驗） | 本機 pandas 分組、合併、事件分析；輸出追蹤 CSV 及多工作表 XLSX，模型只接收精簡結論。 |
| LOG 分析 | 支援只有時間的紀錄；以近似時間定位事件，分批回讀前後文並附來源行號。 |
| 執行中傳送 | 主輸入框選下一輪提示、停止後新任務或完成後新任務；待送訊息可取消。 |
| 本地 CSV 畫圖 | Excel／LOG 批次保存 CSV，對話只帶首尾預覽，直接引用檔案作圖及更正欄位。 |
| 主動整理上下文 | 保存交接筆記與失效結論，移出舊工具原文，完整紀錄按需查回。 |
| 即時進度筆記 | 執行時顯示工作摘要，結束後收進工具紀錄，保持最終答案清楚。 |
| 長任務與專案記憶 | 每段最長 24 小時，工作筆記、加密 checkpoint、工具計數與原始結果按需回讀。 |
| Outlook 助理與專案 | 本機勾選可用資料夾，最多1000封前文比對、AI最多50封，優先最新信與必要補讀，專案快速入口可整理待辦、回覆與追蹤事項，也可接續生成週報。 |
| 選字快捷鍵 | 在其他程式選取文字，帶入 LM_AI 草稿後再送出。 |
| VNC 快速連線 | 選用功能，管理機台清單並啟動已安裝的 UltraVNC Viewer。 |

專案支援 TXT、MD、Word、Excel、PowerPoint、PDF、MSG，以及唯讀 LOG／OUT／ERR／JSONL。各格式的可讀取／可修改範圍，請見 [使用指南](docs/USER_GUIDE.md)。

## 文件導覽

| 想了解的內容 | 文件 |
| --- | --- |
| 安裝、登入、對話與各項功能怎麼用 | [使用指南](docs/USER_GUIDE.md) |
| 每個版本改了什麼 | [版本更新紀錄](CHANGELOG.md) |
| 環境設定、編譯、測試與程式碼位置 | [開發說明](docs/DEVELOPMENT.md) |
| 後端契約、提示詞、驗證與歷史文件 | [技術文件索引](docs/README.md) |

## 開發概況

使用 Rust、Windows API 與內嵌 WebView2 介面；前端資源隨 EXE 提供，不依賴 CDN。編譯基準為 Rust 1.98.1、MSVC v142 x64 與 Windows SDK 10.0.19041.0。

維護前請閱讀 [AGENTS.md](AGENTS.md)，編譯與交付方式見 [開發說明](docs/DEVELOPMENT.md)。
