# 0.8.24 驗證紀錄

2026-09-30 執行 `scripts/Build.ps1 -EmptyCargoCache` 成功。

- MSVC v142 14.29.30133，實際 `_MSC_FULL_VER=192930159` x64；Rust 1.98.1、Windows SDK 10.0.19041.0，靜態 CRT。
- Cargo.lock、vendor、.cargo/config.toml 齊備，依賴未變；全新空 Cargo 快取的 fmt、Clippy `-D warnings`、147 項測試、`cargo build --release --frozen`、WebView2 DOM 及真正 AppContainer 整合全部通過。
- EXE：8,823,296 bytes；SHA256 `66d08fd4d5fc1f1a690ada55fc0a17038ad63ac12bdd0e433fee510ad2f5483e`。簽署清單與建置結果已核對；Build 以原生公鑰驗證簽署清單。

## 本次新增／調整驗證

- 第 1–4 次有效閱讀不提示；第 5 次仍未讀完才提供一次可選技能，之後不反覆附帶。
- 完全忽略筆記仍可繼續；讀完後計數清零、不增加修復次數。第 5 次恰好讀完不提示。
- A、B 文件分開計數；同文件的 read_file／read_document_section 共用計數，大小寫及分隔符號正規化。
- 失敗、重讀、相同操作重播不增加；跳到末段但仍有閱讀缺口不清零，版本變更重新計數及提示。
- 六次儲存操作不要求筆記；自願 progress_note 仍可用於有限續接，未摘要原文仍保留。
- 真實 loopback HTTP＋AppContainer 讀取 23 段、138,000 字：完全不附 progress_note 仍以 24 次模型請求完成；技能只出現一次、計數歸零、沒有修復、原件不變。
- 另一長文情境自願於第 5 次閱讀後附累積筆記，再注入裸 done／空 result；兩次修復後正常完成，保留最近兩段已摘要與全部未摘要證據，共 26 次請求。
- 既有工具去重、副本編輯／儲存、錯誤身分／取消／未知提交拒絕、`.lmai` 排除、筆記持久化、PDF 跨任務快取與来源失效回歸皆通過。

機器記錄：`offline/exe-verification.json`、`offline/environment.txt`。

未連線公司 GLM／Gemma；以上為確定性模型回覆與本機工具驗證，仍須公司實測是否改善閱讀中錯亂。Office／MSG 方法未改，本輪未重跑真實 Office。沒有新外觀截圖，不宣稱視覺驗收。

只交付 EXE、EXE 簽署清單、原始碼及文件；不製作 NSIS／ZIP。既有 `.lmai` 可沿用，不需清除或重新轉換 PDF。Git 推送不代表內網部署完成。
