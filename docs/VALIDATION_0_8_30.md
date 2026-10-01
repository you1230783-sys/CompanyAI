# 0.8.30 原生工具 Schema 相容性修正

日期：2026-10-01。沿用 [desktop-agent-v1 契約](DESKTOP_AGENT_V1_CONTRACT.md)。

## 原因與修正

公司網站拒絕第一輪提交，回報 HTTP 422、UNSUPPORTED_SCHEMA、task_accepted=false。桌面 0.8.29 將內建工具的 type 陣列直接送出，其中 office_action／office_batch 的表格儲存格接受 string、number、null，create_chart 的橫軸接受 string、number；契約僅允許 type 陣列表達單一型別加 null，因此網站拒絕符合契約。

本版將這類多型別轉為非根位置的 anyOf，各分支保留原限制；原有 nullable、工具權限及操作流程不變。網站不需要新增欄位或放寬規則。strict=true 與 false 都套用轉換，避免非嚴格模式仍在提交驗證時失敗。

新增測試遍歷兩種模式下全部 31 個工具，檢查 type 陣列只含一種型別及 null；另測試文字、數字、空值、非法型別，以及 enum／既有 anyOf 約束保留。

## 驗證狀態

`scripts/Build.ps1 -EmptyCargoCache` 已以退出碼 0 完成：

- Rust 1.98.1、MSVC v142 14.29.30133、Windows SDK 10.0.19041.0；實際編譯器探針確認 `_MSC_FULL_VER=192930159`、x64。
- 空 Cargo 快取，依專案 Cargo.lock／vendor／.cargo/config.toml 執行 `--frozen` 離線建置；格式、Clippy、179 項單元測試及 release 建置通過。
- WebView2 自檢、實際 AppContainer 隔離、11 個原生代理情境、既有工具往返、60 次工具暫停與 DPAPI 續接、筆記及 PDF 快取整合通過。期限測試採短時鐘邊界模擬，不代表實際等待兩小時。
- EXE 更新清單已簽署，並透過本次 EXE 內建公鑰驗證成功。

Office COM 未修改；本次未重跑實際 Office 大表或格式操作，也未製作 NSIS／ZIP。前版 Office 結果見 [0.8.29 紀錄](VALIDATION_0_8_29.md)。

產物：`dist/LM_AI.exe`，版本 `0.8.30.0`，大小 `11,508,224` bytes。

SHA256：`ef3d2dd489bdcf7a252159b73daa33e6bcbaf851714eb2a94bc1c9602fb8352f`。

本機測試不代表公司網站或 GLM／Gemma 端到端驗收，需重新以新版 EXE 實測原任務。
