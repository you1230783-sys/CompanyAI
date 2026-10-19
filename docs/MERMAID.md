# 專案 Mermaid 圖形

在專案對話中，可以貼文字、Mermaid 語法，或要求 AI 閱讀授權資料夾的程式碼後畫圖。例如：

> 請讀取這個資料夾的程式碼，先畫主要流程總覽，再畫登入模組的流程圖。標示來源檔案與行號，未確認的動態呼叫請註明；不要執行程式。保存原始碼、PNG 與 SVG。

大型專案會先找入口與相關模組，再分段閱讀；流程圖代表實際閱讀範圍的靜態整理。反射、執行期插件或外部服務的行為，需要另外確認。

## 檢視與保存

- 圖形卡片依SVG原始尺寸顯示，可縮放10–800%、回到原始比例、符合視窗或全畫面檢視；大圖在圖形區內捲動。全畫面時按「返回對話（Esc）」或Esc離開。
- 「查看 Mermaid 原始碼」與「複製原始碼」提供真正的文字來源。
- `.mmd`：UTF-8 原始碼，供下一輪對話或其他 Mermaid 工具修改。
- SVG：可縮放的向量圖片，適合長圖與排版。
- PNG：白底圖片，適合文件與簡報；保留圖形比例，最長邊不超過 4096 像素。
- 新任務的檔案存到專案的 `_AI_Output/對話_<識別碼>/`，同一專案對話跨訊息共用此位置，MMD／PNG／SVG不另分資料夾；同名檔案加序號，不覆寫舊版。舊暫停任務保留原有目錄。

可直接要求 AI 保存 PNG、SVG 或 MMD，也可以按卡片按鈕保存。檔案成功讀回核對後才會回報已儲存。

## 修改前一張圖

按「請 AI 修改此圖」會把圖形參照放入草稿，補上修改要求再送出。也可以直接說：

> 請把上一張流程圖的「檢查失敗」改成「資料檢查未通過」，增加重試分支，其他部分保留。

AI 會取得已保存的完整原始碼與版本，另建新圖。舊訊息、舊原始碼及舊圖都保留；不從 PNG 猜原始碼。對話關閉或重新啟動後，已保存來源仍可讀取。外部修改了本次成果時，續接或交付會要求重新確認。

## Mermaid 與 draw.io

常用語法包括 `flowchart`、`sequenceDiagram`、`stateDiagram-v2`、`classDiagram` 與 `erDiagram`。本版使用離線 Mermaid 12.2.0，不需 CDN。

draw.io 可在其 Mermaid 插入功能貼上複製的語法；`.mmd` 不是原生 `.drawio` 檔。可轉換的圖形、來源重編輯方式及功能取決於 draw.io 桌面版本。一般 PNG／SVG 匯出不會自動變成 draw.io 的原生節點或附帶其專屬編輯資料。[draw.io 官方說明](https://www.drawio.com/docs/manual/mermaid/)

## 範圍與錯誤

單張原始碼最多 48,000 bytes、每個任務最多 12 張；複雜圖建議拆成總覽及模組圖。來源不開放覆寫引擎設定、外部圖片、外部連結、click 或自訂腳本。語法錯誤會回到 AI 修正；無法呈現時仍保留工具紀錄，不把錯誤結果當作成功圖片。

### 開發維護

來源契約在 `src/projects/diagrams.rs`；受控保存／版本／完整性在 `src/projects/files/diagrams.rs`；呈現在 `ui/diagrams.js`。固定的 WebView2 `Runtime.evaluate` 使用 `awaitPromise`，模型不能指定腳本。SVG 以 strict 設定產生，再經 DOMPurify 與外部資源檢查。

模型每輪固定帶入最近兩張圖的精確來源（合計上限32 KB），以及本任務已讀的程式／完整短篇 README 快照（最多6段、文字合計16 KB），與工具歷史整理分開保存。來源片段帶行號與revision，新revision會淘汰同檔舊片段；checkpoint也保留。這是已讀版本，不能宣稱即時同步外部檔案。超出預算的圖只保留索引並明示按需讀完整原碼，不截斷語法冒充完整來源。

上游 bundle、source map 及 66 個確切依賴版本的來源／完整性資料，見 `ui/vendor/packages.json`、`mermaid-dependencies.json` 與 `MERMAID-NOTICES.txt`。建置直接嵌入資源，不執行 npm 或套件安裝腳本。

### SVG 比較符號相容修補

0.8.73 內附 Mermaid 12.2.0 的一個單字元 SVG 文字分詞修補，避免 `>=` 被畫成 `=`；`>`、`<=`、`<` 與既有實體編碼均有回歸。原始 MMD 不改寫，安全層級與 SVG 淨化保持啟用。上游與修補後雜湊、重現工具及修改聲明在 `ui/vendor/packages.json`、`tools/patch_mermaid_svg_text.py` 與 `ui/vendor/MERMAID-NOTICES.txt`。上游函式位置：[handle-markdown-text.ts](https://github.com/mermaid-js/mermaid/blob/develop/packages/mermaid/src/rendering-util/handle-markdown-text.ts)。
