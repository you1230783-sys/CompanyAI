# 內建桌寵插畫來源

以內建image_gen產生的原創貓／狗插畫圖集，2026-11-03替換舊幾何基本款。沒有使用其他作者角色參考或外部圖庫。

- cat-atlas.png：奶油金色長毛幼貓，綠色眼睛。
- dog-atlas.png：可可棕／奶油白垂耳幼犬。
- 兩張均1536×1024、3欄2列；依序待機、思考、工作、等待、完成、錯誤。
- 最終提示詞見[prompts.json](prompts.json)；生成方式為內建image_gen，並要求transparent_background=true。
- scripts/Create-PetDefaults.ps1只作分格與縮圖，輸出ui/pets的12張256×256透明PNG。原始生成alpha保留，沒有用背景顏色猜測去背。
- [manifest.json](manifest.json)記錄輸出雜湊、大小及alpha分布；每張小於500,000 bytes。原圖集不嵌入EXE，只有12張小圖嵌入。

```powershell
.\scripts\Create-PetDefaults.ps1
```

修改圖檔後需重新編譯，才會更換應用程式內建角色；使用者自訂角色不會被覆蓋。
