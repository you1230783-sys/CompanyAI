# 0.8.75 互動教學與正式交付驗證

完成時間：2026-10-21T00:14:31.738087+08:00

## 已完成範圍

- 使用 `Build.ps1 -EmptyCargoCache -BasicValidation -IncludeInstaller`，實際 v142 14.29 x64 編譯器探針、格式與 Clippy、空 Cargo 快取、Cargo.lock／vendor／frozen 編譯及 release EXE。
- 18 組篩選共 107 個 Rust 測試通過，含教學完成與首次選單旗標分離保存；不是執行全部 430 個測試。
- 真瀏覽器逐題完成 55 個互動教學；驗證輸入／選單／確認框門檻、鍵盤焦點、完成回呼、搜尋，不發正式命令、不改真實草稿。
- 設定頁亮／暗與窄版、重看入口、更新日誌回歸通過；首個教學 520px 窄版截圖已目視。
- release EXE 原生 WebView2 自檢、composer／任務追蹤、Python runtime、CAD 合成契約與 runtime、Outlook 無效要求／EOF、不讀信箱，以及唯讀程式碼範例核對通過。
- NSIS 0.8.75 安裝包已建立；RSA 簽署更新清單與安裝包由正式 EXE 內建公鑰核對，兩份更新清單一致。

## 成果雜湊

| 檔案 | bytes | SHA256 |
|---|---:|---|
| LM_AI.exe | 58615296 | `0a1e102ff4a032d11a661b7c9063cdc30d6d3c1cbdfcd3a98b0251efa1d503dc` |
| LM_AI_Setup.exe | 46813158 | `b214c7fa9723a22758f1496a9728d309006f3ff999b0860e2b114e3169ff85fa` |

## 限制與原始失敗

- 本版為基本驗證，不重跑完整 Office／CAD／公司模型或 NSIS 安裝、更新、回復、修復、移除生命週期。0.8.74 的完整結果保留為歷史，不能當成本版重測。
- 教學全部是假資料與固定示範；自動逐題成功不代表模型理解或真實外部工具作業成功，亦未逐一目視所有主題、深色與縮放組合。
- 初次瀏覽器檢查未建立 DevToolsActivePort；切換至可啟動桌面程序的執行環境後，同一份測試重跑通過。原始失敗留在私人 .build，不抹除改判。
- 公司登入與後端服務仍需公司環境確認，本次未修改登入協定。

## 文件

- [55 個主題覆蓋表](TUTORIAL_COVERAGE.md)
- [教學操作與維護](PROJECT_TUTORIAL_PREVIEW.md)
- [可公開分項摘要](../offline/tutorial-verification-0.8.75.json)
- [基本原生驗證](../offline/exe-verification.json)
- [本版封裝驗證](../offline/installer-verification.json)
- [0.8.74 完整原生驗證存檔](../offline/exe-verification-0.8.74.json)
- [0.8.74 完整安裝驗證存檔](../offline/installer-verification-0.8.74.json)
