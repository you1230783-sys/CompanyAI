# 0.8.9 三花貓圖示更新

使用者提供新的 `icon.png`，要求重新編譯、打包並推送。將原圖完整複製到 `assets/app.png`，透過既有 `Convert-AppIcon.ps1` 產生多尺寸 ICO；不重新繪製、裁切或拉伸。版本進至 0.8.9，程式功能與 API 不變。

圖示來源統一供 EXE 資源、視窗大小圖示、工作列、托盤、介面左上品牌，以及 NSIS 安裝／解除安裝使用。一般安裝仍不檢查或附帶 WebView2、不自動啟動 LM_AI；VNC 設定保留，SFX 維持撤下。

## 驗證

2026-09-23 執行 `scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller`，首次完整建置即通過：

- MSVC v142 14.29.30133、實際 `_MSC_FULL_VER=192930159` x64、SDK 10.0.19041.0、Rust 1.98.1。
- fmt、Clippy、81 項 Rust 測試、空 Cargo 快取 `cargo build --release --frozen`；依賴由既有 Cargo.lock、vendor、.cargo/config.toml 提供，鎖檔只變更本專案版本。
- WebView2 DOM 自檢（包含左上品牌載入）、EXE／NSIS 更新清單原生驗證。
- NSIS 真正安裝、更新程序交接與重啟、鎖定失敗保留舊 EXE、VNC 設定／未知檔案保留、解除安裝清理。
- Runtime 無法載入模擬：安裝成功，主程式啟動自檢才回傳既有補裝提示；正常 Runtime 下安裝後自檢通過。

另檢視使用者原圖與 ICO 256px 預覽。逐一核對 EXE 圖示群組 1、Setup 圖示群組 103 的十種圖框，其位元組均與 `assets/app.ico` 相同，並實際使用 Windows LoadImage 載入每個尺寸，確認尺寸及透明角落保留。尺寸為 16、20、24、32、40、48、64、96、128、256px。記錄見 `offline/icon-verification.json`。

本次沒有拍攝實際桌面／托盤畫面，不宣稱已驗收 Windows 圖示快取或公司防毒。原始圖片、ICO 預覽及成品資源已驗證；公司趨勢攔截情況仍需實測。前版記錄的串流閱讀位置自檢間歇性失敗，本輪未重現，也未修改或略過該測試。

## 成品

| 檔案 | 大小（bytes） | SHA256 |
|---|---:|---|
| assets/app.png（與使用者 icon.png 相同） | 2,275,398 | `3c4998dc9a69f33f625a34702add209291a2795f258d0d293e4e5526a2a980b6` |
| assets/app.ico | 247,581 | `6f537ed8eed22b8a2ecef10e302c377abf879766b97fb628e9bdae38a0ee8b91` |
| dist/LM_AI.exe | 4,742,144 | `d75e2bbd61df61da2ccabcec1a68afb63e05053c5c6867dffd693fe57568909d` |
| dist/LM_AI_Setup.exe | 1,934,693 | `4250daa5c7f04f5bb9d10830785b2c3219e0d01017ab54eecfaba4092cda309f` |

EXE 與 Setup 檔案版本均為 0.8.9.0。每份 EXE／Setup 必須與各自的更新清單成對部署；同版雙格式仍優先 EXE，若要提供 NSIS 自動安裝，網站所有探索來源應只公告新版 NSIS。Git 推送不代表內網網站已更新。

歷史離線 ZIP 未重新製作；VNC Viewer 連線仍沿用先前驗收，本次沒有重測連線。安裝及建置結果詳見 `offline/installer-verification.json`、`offline/exe-verification.json`。
