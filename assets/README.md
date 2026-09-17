# 應用程式圖案

`app.png` 是使用者提供的 `05AF27DC-7858-4166-90BF-07BBF175E87F.png` 原始圖片副本。
`app.ico` 保留透明背景與完整圖案，包含 16、20、24、32、40、48、64、96、128、256 像素。

更新 PNG 後，在專案根目錄執行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Convert-AppIcon.ps1
```

轉換只使用 Windows 內建 .NET，不需要額外安裝套件；日常與離線建置直接使用已產生的 ICO。
build.rs 以 Windows SDK 的 rc.exe 嵌入 EXE 資源 ID 1，檔案總管、視窗、工作列與系統托盤共用此圖案。
視窗分別設定大小圖示，托盤依 Windows 的小圖示尺寸載入；Windows 管理共用圖示生命週期。
缺少 ICO、ICO 損毀或資源編譯失敗會停止建置，避免交付預設圖示；執行時載入異常仍保留系統 fallback。
圖示內嵌於 EXE，執行時不需另放 PNG 或 ICO。更新後須完全退出舊程式再開啟新版。
