# localhost Luna 驗證服務

開發機需要 Python 3 與已登入的官方 Codex。沒有額外 Python 套件；由 Codex
App Server 自行處理登入，腳本不讀取或複製憑證。只綁定 127.0.0.1，使用
localhost 網域即可，不需修改 hosts、DNS 或公司網站。

```powershell
./scripts/Start-LocalLuna.ps1 -OpenApp
```

開啟獨立測試資料夾及專案，不載入正式登入與聊天資料。本工具隨 0.8.68 納入原始碼。
Rust 修改必須先以 `Build.ps1 -EmptyCargoCache -ValidateOnly -BasicValidation`
指定 v142 x64 產生測試 EXE。這是局部驗證，不是完整發行。

## 測試方式

在「Luna 本機 CAD 測試」專案建立對話，送出合成圖面的唯讀核對要求。
正式 runner、Broker、上下文整理與 CAD worker 都會執行。localhost 的
`/health` 顯示模型及使用輪數；每個 `turn-*.request.json`、
`turn-*.response.json` 保存實際快照、選定工具、offset、耗時與 Codex 用量。
紀錄留在 .build，可能含測試正文，不放進發行包或 Git。

用 `python tools/summarize_luna_test.py <該次紀錄資料夾>` 產生 summary.json，
可一次查看 offset 順序、各輪模型耗時及桌面工具間隔。

服務預設最多 30 次推論，單輪 240 秒。使用 `server.pid` 對應本次程序停止服務；
先停止應用中的任務再停止服務。連線授權碼只存在本機 connection.json。
每次啟動資料夾均不同，重啟服務不續接前次記憶體任務。

## 模擬範圍

- 指定 GPT-6 Luna、low；啟動推論時核對 model/list，不支援就明示失敗，不偷偷換模型。
- 每輪新建 ephemeral thread，只提供 LM_AI 當輪的 messages／tools，無跨輪 Codex 記憶。
- 使用 Codex App Server 的原生 dynamicTools，收到 item/tool/call 就將參數轉給桌面的
  tool_calls；Codex 不執行 CAD 工具。每輪程序隨即結束，下一輪以桌面快照重新建立。
  Codex harness 仍有額外固定開銷，所以 token 與速度不等同公司模型。
- 不修正模型的 CAD offset，不裁切內容、不代模型製作核對答案；正式 Broker 驗證參數。
- 不允許測試模型使用 Codex 的 shell／檔案／MCP 工具；出現這類工具事件即停止該輪。
- 目前提供專案 agent 背景任務與必要能力路由；不是完整公司網站，未實作一般聊天、附件與正式登入服務。
- localhost 只是轉接，推論仍送到 OpenAI。請使用本次合成測試資料。
- events.jsonl 記錄協定事件種類，stderr.log 保存啟動診斷；尚未回傳 usage 就收到工具呼叫時，
  codex_usage 會是 null。有值也只是最近一次事件，未必包括全部工具參數輸出；
  不以它冒充完整一輪用量，不用來判斷公司模型是否碰到輸出上限。

## 分頁案例

`tools/create_luna_cad_fixture.py` 用既有鎖定 CAD worker 在 2022 新建法蘭板，
預設正弦曲線 600 段，讓圖元超過六頁。只建立新路徑，不覆寫現有圖檔。
實際驗證結果另記錄於 `VALIDATION_LOCAL_LUNA.md`，不得把架設成功當成 CAD 測試通過。
