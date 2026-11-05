# 0.9.7 完成後交接

> 歷史交接：後續0.9.8、0.9.9來源交付與0.9.10已完成。最新停止點、收據及待公司實測項目見[0.9.10驗證與發行](VALIDATION_0_9_10.md)。下方「未開始」僅代表當時狀態。

更新日期：2026-11-05。**最新節奏：0.9.8完整驗證及發布；0.9.9（F08／F09／C01）僅提交推送原始碼，[skip ci]，不編譯／封裝／驗證或發布Release；直接接續0.9.10 D03，再將合併範圍完整驗證／封裝並發布Git及Release。詳細commit供Claude二次驗證。下方0.9.7完成紀錄保留。**

## 已完成

- 0.9.7 CI／漏洞檢查已發行：[Release](https://github.com/you1230783-sys/CompanyAI/releases/tag/v0.9.7)，tag提交`5fb110822ca461d92df867da48e57cd4c93a9751`。
- [發行提交CI](https://github.com/you1230783-sys/CompanyAI/actions/runs/41945624630)通過。首次CI揭露的11項記憶檔共享衝突與2項ANSI測試假定已修正；沒有增加互動桌面以外的略過項目。
- 本機v142 x64 14.29.30133／實際編譯器192930159、空Cargo快取frozen、499完整／455核心Rust、Clippy、Office、Python、原生回歸、NSIS均通過。28份證據見[驗證紀錄](VALIDATION_0_9_7.md)及[摘要](../offline/release-verification-0.9.7.json)。
- 五項Release資產已公開下載並核對SHA256。Setup為55,533,469 bytes，SHA256 `5c75f7ab83028f160af778e3ca56f6ecf4f50250a61324e955d2103dd7ef19f6`。EXE只留本機，不上傳Git或Release。
- 最後的交接提交只修改Markdown文件，以`[skip ci]`避免重跑未變更的程式；發行tag仍對應上面已通過CI的提交。沒有修改CI工作流程或略過任何發行測試。

## 未開始的工作

| 順序 | 版本／項目 | 下一輪內容 |
|---|---|---|
| 1 | 0.9.8／D02b | 同來源量測baseline、ThinLTO及codegen候選的體積與建置成本，再選配置；保留unwind，完整驗證封裝後發行 |
| 2 | 0.9.9／F08＋F09＋C01 | Word依內容對齊，涵蓋前插／刪除、重複段落、表格與格式差異；額外功能預設關閉，Outlook直達／進階功能入口切換，VNC獨立，教學依各開關過濾 |
| 3 | 0.9.10／D03 | 已保存流程僅在程式開啟／托盤時執行，預先確認固定參數與範圍，保存執行身分，不重播成功或未知寫入；不做關閉程式後的服務排程 |

以上均未進版、未修改對應功能。M02使用評價與統計依使用者指示擱置；跨專案知識庫不在目前排程；Authenticode已移除待辦。NAS同步由使用者處理。

2026-11-05追加記錄：C01輸入預算與原文保留方案已確認，50,000 tokens軟目標、超標整理至約40,000，取消日常最近三組限制並放寬固定次數／匯出後整理；技能與筆記維持現狀。最新排入0.9.9，與F08／F09同版；詳細範圍與驗收見[實作手冊](IMPLEMENTATION_PLAN.md)。最新授權已取代先前僅記錄的限制。

## 不可遺漏的限制

- 工作目錄為`Y:\Rust\Project\CompanyAI`，先讀專案AGENTS.md及[實作手冊](IMPLEMENTATION_PLAN.md)。不修改共享工具／其他專案。
- 使用`scripts/Build.ps1`與指定v142 x64；保留Cargo.lock、vendor、.cargo/config.toml，使用空Cargo快取與frozen驗證。一次只執行一條Cargo建置流程。
- 安裝位置`C:\largan\LM_AI`、公司HTTP、machines.json明文格式與匯入匯出、更新同源／RSA／SHA256不變。內建更新仍從lm_server下載，GitHub只是手動入口。
- Git不提交EXE、ZIP、vendor、target、私鑰、Token、對話或機台資料；Release僅NSIS、兩份更新JSON、驗證JSON與SHA256清單。歷史根0.8.75保持，不再改寫；伺服器LFS配額回收仍待平台。
- 公司模型、信箱、加密Office、實際SMB／VNC／防毒、DPI與翻譯品質仍待公司實機確認，不能宣稱全部驗收。
- ttf-parser停止維護公告的精確例外須於2027-02-01 UTC前重新評估；[依賴政策](CI_AND_DEPENDENCIES.md)仍拒絕新漏洞與未審查公告。

0.9.7已結案，不需要重跑已通過的完整流程。本輪從0.9.8開始，依最新授權逐版完成至0.9.10。
