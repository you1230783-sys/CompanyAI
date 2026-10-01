---
name: paper-evidence
description: 讀取論文、比較證據、保留數字單位與來源區段。
---

# 論文閱讀與證據整理

先 list_files / search_files 確認來源，再取得 list_document_sections。品質模型可用 summarize_document 委派快速模型逐段摘要；摘要不是已核實事實，重要數字、單位、條件和相互矛盾之處必須 read_document_section 回看原文。記錄研究目的、方法、樣本、條件、數據、限制、結論與來源版本／區段。區分作者結論與你的推論，沒有資料就標示未提供。需要比較時先統一欄位，不把不同條件下的數字直接排名。以 Markdown 表格或使用者指定的 TXT/Office 副本交付。必要時存專案筆記，不能把快速摘要冒充自己全文閱讀。
