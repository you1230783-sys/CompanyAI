# 0.8.65 技能整理

load_skill只切換本階段工具；read_skill_guide可按需讀參考頁，不改變工具權限。
CAD共用安全規則常駐一次；舊cad-drawing不再公告，但舊checkpoint保留相容入口與既有權限，指南要求轉新分組。
新建／目前圖面／副本各自只公告2／3／3個CAD工具；Excel保留計畫與目標讀取工具並提供inspect_excel。
下表為原始緊湊JSON的CAD工具定義UTF-8 bytes，不含基本工具、strict schema轉換或服務端tokenizer，不宣稱實際tokens／延遲同比例下降。

| 組別 | CAD工具數 | 定義bytes |
|---|---:|---:|
| 舊全部CAD | 8 | 26692 |
| 新建 | 2 | 7099 |
| 目前圖面 | 3 | 8928 |
| 副本 | 3 | 9708 |
| Excel | 5 | 3385 |

Python保留共用執行規則，表格／LOG／程式碼分析另設參考頁；不再一併開放資料集繪圖工具。
LOG分離時間定位與事件配對；圖表依資料準備／繪圖座標／外觀編輯切換；Office、短文字與筆記維持短指南。
Outlook閱讀與前文比對不再互相啟用；現有mail_ids與草稿保留，切換不重新掃描。
論文研究等相依技能若核心文字相同只送一次。版本歷史、COM實作與編譯說明移出模型CAD指南。
不得將技能精簡解讀為放寬唯讀、副本、Undo、文件revision、授權及失敗後重試保護。

## 參考頁

- CAD：cad-geometry、cad-edits、cad-commands。
- Python：python-tables、python-logs、python-source。
- LOG：log-time、log-events。
- 圖表：chart-details。

## 驗證界線

技能目錄／schema一致性、按分組工具開關、參考頁不可當技能啟用、相依說明去重納入基礎契約。
公司模型連續超時是否改善需重測；本機沒有CAD，不能宣稱2008／2022第三項已通過。
