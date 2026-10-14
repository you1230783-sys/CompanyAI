# 0.8.55 CAD AI 繪圖測試版驗證

狀態：**PASS_BASIC，2026-10-14 19:48（台北）**。正式 EXE 與 NSIS 均為 0.8.55.0；完整驗證未執行。

使用者要求跳過完整驗證，只做基礎驗證並提供 NSIS 供公司實測。

## 已有證據

- 使用者回報獨立工具 2022 基礎繪圖一次通過；文字、剖面、公式曲線外觀正常，修正 AddLeader Type=2 後箭頭出現。
- 2008 基礎繪圖曾完成；標註 Measurement 未實作但使用者目視有尺寸。不能據此宣稱本版全部操作已在 2008 通過。
- 本版新增顏色、AI 契約、工作程序與安裝整合尚需公司測試。

## 基礎範圍

v142 x64 編譯器探針、空 Cargo 快取／--frozen、fmt／Clippy、選定契約單元測試、release、既有原生 UI／聊天與程式唯讀自檢、CAD 十項合成契約與真實唯讀管線、Python runtime、NSIS 打包及原生簽署更新清單核對。

## 實際完成

- MSVC 14.29.30133 x64、`_MSC_FULL_VER=192930159`，同版 linker，Windows SDK 10.0.19041.0。
- 空 Cargo home `cargo-empty-8f7d14b6a8da4b8ba3f19f4ffd895b04`，專案 Cargo.lock／vendor／.cargo/config.toml 與 `--frozen`，無新增 Rust 依賴。
- fmt、Clippy all targets、41 項選定 Rust 單元測試，正式 release 編譯成功。
- CAD 十項合成契約通過：算式、非法運算／程式碼拒絕、顏色界線、非有限座標、引線水平段與 Type=2、顏色讀回、版本排序。原生 broker CAD 成果恢復、交付雜湊及外部改檔拒絕通過。
- 正式 EXE 的 CAD runtime 檔案核對、模組載入、匿名管線與唯讀 COM 登錄枚舉通過；dist 內 EXE 再次 `--cad-self-check` 通過。沒有建立任何實機 CAD 圖面。
- 既有 Python runtime、WebView2／原生輸入、任務拒絕與停止追蹤／DPAPI 恢復、Outlook helper 管線（不讀信箱）、程式碼唯讀實際 worker 流程與 DLL 核對通過。
- NSIS 打包與原生更新器簽署清單驗證通過；兩份更新清單位元組相同，均指向本次 NSIS。
- 過程中修正技能說明長度、工具目錄覆蓋測試樣本及舊文字契約 64 KB 限制，最終全部選定測試重跑通過。原沙箱的 Python ACL 初始化回錯誤 5，已用獲准的建置帳戶權限完整重跑，未跳過該檢查。

機器紀錄：[基本建置](../offline/exe-verification.json)、[CAD 整合](../offline/cad-integration-verification.json)、[編譯環境](../offline/environment.txt)。

## 產物

| 檔案 | Bytes | SHA256 |
| --- | ---: | --- |
| dist/LM_AI.exe | 18160128 | f7d591e8539f533abda5dcbba23828c7f6e8bd412a38c907e3cb8881107a0702 |
| dist/LM_AI_Setup.exe | 36021402 | 4264b52e319a9d2b6a2d0941778d1e874a381e0b53622448f7ece3d98b79c341 |

CAD runtime 清單 SHA256：`a147602918d0087cd7ab371afddc5de7ed9c53642c4a119a6c2ba059da5eb1e8`。請使用 NSIS 安裝，不只複製 EXE，否則可能缺少新增 CAD runtime。原分析 Python runtime 指紋未修改。

使用者於交付後追加授權提交及推送既有 Git main，包含本版原始碼、EXE、NSIS 與驗證紀錄；沒有重製歷史 Rust 離線 ZIP 或獨立 CAD ZIP，保留原有 icon.png。

## 延後

完整 workspace、Office／SMB／Python 長流程、正式 NSIS 安裝更新回復解除安裝、公司 CAD 2008／2022 全部操作、ACI 色彩、公式曲線精度與 AI 真實模型規劃品質。
