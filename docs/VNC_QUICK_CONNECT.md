# VNC 快速連線（0.8.6）

本次先交付 0.8.6 EXE、EXE 更新清單與原始碼，暫不重製 NSIS 或離線 ZIP。發行驗證見 [0.8.6 驗收](VALIDATION_0_8_6.md)。

## 開啟功能

在「設定 → 使用習慣」勾選「啟用 VNC 快速連線功能」，側欄才會顯示入口。舊設定與全新設定一律預設關閉；此偏好保存於各 Windows 使用者原有的 CompanyAI 設定。

未啟用時不讀取 VNC 機台檔、不啟動搜尋或連線，原生層也會拒絕 VNC 命令。停用會關閉 VNC 管理畫面並清除已載入的機台狀態；已開啟的 UltraVNC 視窗維持獨立運作。這是本機工具，不需要 AI 登入，但仍受應用程式既有最低版本門檻約束。

## 沿用 Python 設定檔

固定讀寫 **目前執行的 LM_AI.exe 同一資料夾**：

- `machines.json`：機台分類、名稱、位址與密碼。
- `user_config.json`：Viewer 路徑與連線選項。

正式安裝位置目前為 `C:\largan\LM_AI`，因此可將原工具的兩個檔案直接放到該處，不必重新填寫機台。開啟 VNC 頁面後可按「重新讀取設定」。沒有機台檔時顯示空清單，不建立示範機台；新增機台並儲存時才建立 `machines.json`。執行帳號需能寫入此資料夾，否則顯示儲存失敗並保留原資料。

機台格式維持原本的「分類 → 機台陣列」，沒有新增外層版本欄位、ID 或密碼包裝：

```json
{
    "A": [
        {"name": "加工機10", "ip": "192.0.2.10", "password": ""},
        {"name": "備用機", "ip": "192.0.2.20", "password": ""}
    ],
    "B": []
}
```

使用者設定維持：

```json
{
    "vnc_path": "C:\\Program Files\\uvnc bvba\\UltraVNC\\vncviewer.exe",
    "options": {
        "fullscreen": false,
        "viewonly": false,
        "autoscaling": true
    }
}
```

讀取 UTF-8（可含 BOM）；寫回使用 UTF-8、四格縮排與 LF。既有機台及使用者設定的額外欄位會保留。檔案上限 4 MiB；格式錯誤時停止操作，不以預設資料覆寫原檔。儲存前比對讀入的檔案快照；發現其他程式已修改時要求重新讀取，避免覆蓋對方的修改。

為相容原工具，密碼仍以原本的字串欄位保存。機台清單僅顯示「已設定／未設定」，既有密碼不回傳 WebView。編輯時留空代表保留，填入新值代表更換，勾選「清除已存密碼」才清除。機台資料不送 AI、網站或日誌，實際使用者檔案已加入 Git 忽略規則。

## 機台管理與順序

- 分類按鈕切換機台卡片，點機台卡片才啟動連線。
- 「機台管理」可新增、編輯、移至另一分類及刪除；刪除前會確認。
- 同一分類的機台完全依 JSON 陣列順序顯示，**不依名稱或數字自動排序**。
- 修改既有機台的名稱、IP 或密碼會保留原位置。新增機台或改到另一分類時追加到該分類最後面。
- 管理清單提供「上移／下移」，只調整同一分類內的位置，並直接保存；第一／最後一台對應的移動按鈕停用。
- 管理命令帶清單版次，過期畫面的索引不會誤改或誤連另一台機台。

UI 沿用目前應用程式的主題色、圓角、按鈕與深色模式；卡片隨寬度排列，窄視窗的管理欄位改為上下排列。

## Viewer 啟動

沿用 `user_config.json` 中有效的 `vnc_path`；沒有有效路徑時在標準 Program Files 目錄背景搜尋 `vncviewer.exe`，也可手動指定或重新搜尋。搜尋有 30 秒／20,000 個資料夾上限，略過 reparse point，不掃描任意磁碟。找不到時提供手動指定提示，不自動下載安裝 Viewer。

Rust 直接啟動指定的 `vncviewer.exe`，工作目錄為 Viewer 所在資料夾，參數逐項傳遞；不依賴 Python、CMD、PowerShell、BAT 或 `where`。

參數沿用使用者提供的 Python 工具：有密碼時加入 `/password`，固定 `/shared /autoreconnect 5 /reconnectcounter 3 /quickoption 7`，依勾選加入 `/fullscreen`、`/viewonly`、`/autoscaling`，最後傳 IP／Server。主機支援名稱、IP、`:display` 或 `::port` 形式；不接受夾帶空白或命令選項。

Viewer 自己保存的預設設定也可能影響實際行為；命令列參考 [UltraVNC 官方說明](https://sc.uvnc.com/docs/ultravnc-viewer/52-ultravnc-viewer-commandline-parameters.html)。本程式顯示「已啟動連線」只代表 Viewer 已啟動，實際驗證與遠端連線結果仍由 Viewer 顯示。

## 驗證紀錄

2026-09-21 執行 `scripts/Build.ps1 -EmptyCargoCache -ValidateOnly`：MSVC 14.29.30133，實際 `_MSC_FULL_VER=192930159` x64；fmt、Clippy、81 項 Rust 測試、使用 Cargo.lock／vendor／.cargo/config.toml 與空 Cargo 快取的 frozen release 編譯、WebView2 DOM 自檢皆通過。`-ValidateOnly` 在驗證後停止，不改交付檔與既有 offline 驗證紀錄。

Rust 測試包含原 Python JSON 讀寫、額外欄位保留、密碼不出現在公開清單、格式錯誤保留原檔、外部修改衝突、儲存失敗不改記憶體、參數排列與邊界、手動順序保存、預設停用與偏好保存。WebView2 自檢使用虛構機台，攔截前端命令，包含啟用入口、分頁、名稱文字顯示、連線參照、選項、手動移動、編輯保留密碼、明確清除及停用關閉畫面；同輪也通過先前 Outlook 範圍與快捷鍵提示修改的自檢。

受限執行環境首次無法初始化 WebView2；改在正常 Windows 環境重跑後通過。2026-09-22 另以本機真實 UltraVNC 1.8.2.4 完成 RFB loopback 整合測試：正式 `vnc::connect()` 呼叫、實際密碼 challenge-response、`/shared` 及畫面更新要求均成功，測試預設選項與全螢幕／唯讀／停用縮放兩種組合。

`examples/vnc_smoke.rs` 只綁定 `127.0.0.1` 的動態 port，建立合成色塊畫面，使用虛構測試密碼與隔離設定檔；成功或失敗皆清理自己啟動的 Viewer，不碰使用者既有連線。測試參考 [RFB RFC 6143](https://www.rfc-editor.org/rfc/rfc6143.html)，未連線公司機台。

Computer Use 未獲准存取 Viewer 視窗，未取得畫面截圖。上述結果證明傳入指定參數後可完成連線初始化，不等同於全螢幕／縮放視覺效果或唯讀輸入攔截的驗收。公司實機的指定路徑對話框、重新連線、Outlook 與整體視覺效果仍需確認。
