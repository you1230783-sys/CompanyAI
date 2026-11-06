# 文件索引

[專案首頁](../README.md) · [使用指南](USER_GUIDE.md) · [版本更新](../CHANGELOG.md)

文件以用途分類；版本驗證依版本排序，日期只用於一次性的實測與歷史紀錄。先從下方入口找資料，不需要逐份翻閱歷史報告。

## 常用入口

| 想找的內容 | 文件 |
| --- | --- |
| 安裝與日常操作 | [使用指南](USER_GUIDE.md) |
| 支援哪些功能 | [功能總覽](FEATURES.md) |
| 出現錯誤時如何處理 | [錯誤代碼](ERROR_CODES.md) |
| 現在做到哪裡、接下來驗什麼 | [實作總計畫](IMPLEMENTATION_PLAN.md) |
| 最新已發布版本與公司端待驗項目 | [0.9.18 驗證紀錄](validation/VALIDATION_0_9_18.md) |
| 如何建置與維護 | [開發說明](development/DEVELOPMENT.md) · [專案指引](../AGENTS.md) |

0.9.18 完成本機相關驗證與封裝，包含 X1／X2、M7 混合 stdout 及資源鎖補強；完整回歸與公司模型效果不在本次驗證範圍。Git／Release 狀態以實作總計畫與版本驗證紀錄為準。

## 分類瀏覽

| 目錄 | 內容與用途 |
| --- | --- |
| [development](development/README.md) | 工具鏈、離線建置、依賴、Python runtime、回歸與模型驗證方法 |
| [features](features/README.md) | Outlook、通知、排程、VNC、Mermaid 與教學規格 |
| [contracts](contracts/README.md) | 網站／桌面代理／更新／結構化回覆契約與請求範例 |
| [cad](cad/README.md) | CAD 能力、繪圖、快照查詢、環境及測試 |
| [validation](validation/README.md) | 各版實際驗證結果、發行證據與未驗範圍，按新到舊排列 |
| [history](history/README.md) | 已結案實作、交接、搬移對照與舊指南；不代表目前授權 |
| [refactors](refactors/README.md) | 純搬移重構的機器可讀對照表 |
| [images](images/) | 文件引用的畫面與圖片 |

## 修改需求與未來構想

這些文件保留在本層，方便交接與查找。需求手冊保留原文；實作狀態與驗證結果另外記錄。

- [EIS 回饋修改手冊](CHANGE_HANDBOOK_EIS_FEEDBACK.md)
- [0.9.18 Excel 閱讀精簡手冊](CHANGE_HANDBOOK_0_9_18_EXCEL_READ.md)
- [0.9.14～0.9.17 修改手冊](CHANGE_HANDBOOK_0_9_14_TO_0_9_17.md)
- [未來構想（未排期）](FUTURE_IDEAS.md)
- [實作總計畫](IMPLEMENTATION_PLAN.md)

## 後續文件放置原則

- 一般操作更新使用指南；功能細節放 `features`，CAD 細節放 `cad`，串接格式放 `contracts`。
- 建置、維護與可重複使用的驗證方法放 `development`；某一版的實際驗證結果放 `validation/VALIDATION_版本.md`，同步更新該目錄索引。
- 當前進度只維護實作總計畫；已結案的獨立實作紀錄放 `history`。舊文件中的「待辦／目前」須依當時版本理解。
- 機器驗證收據仍在專案的 `offline`，圖片仍在 `images`；文件以相對連結引用，不複製一份形成多個版本。
- 本次整理保留既有檔名，搜尋檔名仍可定位；除了必要路徑更新，需求與歷史結論不改寫。手冊中的舊路徑可依檔名到分類目錄查找。
- 本次只整理文件與引用，沒有編譯、進版、打包或發布。0.8.74 以前的既有封存規則見 [封存政策](development/ARCHIVE_POLICY.md)。
