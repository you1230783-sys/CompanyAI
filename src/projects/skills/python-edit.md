# 程式需求、分段修改與驗證（目前支援 Python）

先create_working_copy，再plan_code_change記錄1–16項需求（id、description、origin=user/preserve/added）。使用者原始要求持續保留；不要自行加入功能後漏掉驗證。先前版本續接也要補需求。

原件唯讀。find_text取得行號；read_code_section讀1起算、包含尾行的小段（最多200行／6000字）。edit_code_section帶copy_id、revision、section_hash與replacement，不重送舊全文。一次修改imports或一個函式，每次成功立即更新同一_AI_Output草稿，即使語法未完成也保留。續接沿用copies中的copy_id/draft_path，不重建副本，不反覆讀完整檔案。

所有修改完成後check_python檢查內建Python 3.13的AST／compile與編碼，原文不執行。再review_code_change(copy_id,revision,checks=[])取得需求、真實副本差異與測試；摘要截斷時讀相關區段與呼叫處，核對預設值、輸出名稱及錯誤處理，不能只依進度筆記宣稱完成。

## 功能測試

test_python傳copy_id、目前revision和1–4組tests。每組包含id、requirement_ids、code、mocked_dependencies；code最多12000 bytes，定義unittest.TestCase。工具提供unittest、mock、workspace與load_target(argv=None,as_main=False)。load_target載入完整目前副本，不能另抄一份函式測試；未載入副本、沒有案例或跳過案例不算通過。

```python
class Behavior(unittest.TestCase):
    def test_default_and_boundary(self):
        target = load_target()
        self.assertEqual(target.convert(0), 0)
        with self.assertRaises(ValueError):
            target.convert(None)
```

以上僅示範語法，需依實際需求設計預期值，不能照抄。需要外部模組時可在load_target之前使用with mock.patch.dict('sys.modules', {'module': fake_module})；mocked_dependencies要列出模擬內容。模擬HTTP成功與失敗能測處理流程，不能宣稱真實伺服器、套件或設備已驗證。

每批在新的AppContainer程序執行，60秒／1GiB，沒有網路、子程序或原件存取權；workspace是本次私有目錄，可建立合成資料與核對输出檔。print輸出有上限；用unittest斷言檢查正常、預設、邊界、錯誤和輸出結果。原始.py即使有頂層動作也會執行於隔離區；不要在測試中嘗試安裝依賴或啟動GUI。

缺套件／外部服務／逾時標unavailable，保留草稿並說明界線，不無限重試。failed按錯誤行號只修相關小段再測；同一test id重測更新結果。測試資料與完整結果留加密操作簿，後續僅載入摘要。

## 交付核對

review_code_change提交每項requirement_id、status、evidence、first_line、last_line、test_ids。tested必須引用目前副本真正通過且對應該需求的測試；reviewed僅程式核對，unverified說明無法驗證的原因，incomplete表示尚未完成。所有需求都要核對；失敗測試不能改標reviewed跳過。

語法通過且本版核對完成後save_copy標記同一路徑為交付成果；incomplete保留草稿。缺外部環境仍可交付，但明列未驗證項目。任何修改都使舊語法、測試與核對失效。原生最終回覆附驗證界線；AI說明必須符合實際程式與測試，不能虛構檔名或功能。

技能維持到明確load_skill切換。獨立閱讀可run_batch；修改與驗證相依，依序執行。
