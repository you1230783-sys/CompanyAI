# 0.8.76：非CAD發行驗證

使用者已授權本輪先進版打包並推送Git，CAD修改與操作驗證延至下一版。正式建置於2026-11-02 16:02退出0；完整非CAD自動化驗證與NSIS生命週期通過。模型內容／多步交付仍有已知未通過項目。

## 範圍

沿用scripts/Build.ps1，指定MSVC v142 x64、空Cargo快取、Cargo.lock／vendor／.cargo/config.toml與frozen。執行完整非CAD測試、Office／Python／本機SMB／VNC、NSIS安裝／更新／重啟／回復／修復／移除。CAD圖面與公司模型、真Outlook信箱未測；安裝包只檢查既有CAD runtime套件完整性，不等於CAD操作驗收。

## 模型與先前候選

[非CAD候選驗證](VALIDATION_2026_11_02_NONCAD.md)記錄0.8.75候選的394個Rust測試、21項工作介面及既有UI回歸。GPT-6 Luna low三圖每圖只辨識一次、保存3份短ID筆記，沒有重讀；但一次DRT峰4×10⁻⁵被初次辨識為4×10⁻⁶，筆記及答案保留錯誤。程式、Word及多成果任務未全面交付。這些是合成資料，不是公司模型或使用者原圖。

本次進版只更新版本、發行公告與非CAD發行腳本；不冒稱在0.8.76重新跑Luna。原圖與原文保留，允許必要定向查證，不保證摘要能修正首次誤讀。

## 交付

- EXE及Setup版本：0.8.76.0。
- EXE SHA256：`6236a49d61d7434c1a033efce6c2527ffc683a991888b1ac99e27a3abfb95d51`。
- Setup SHA256：`d2e7c51e979d327ea10e67f69fce7a2c13d1cb88809dd7a3088a67b688276d5a`。
- 單次正式命令：`scripts/Build.ps1 -EmptyCargoCache -SkipCad -IncludeInstaller -TestOffice -TestVnc -NetworkTestRoot <本機SMB測試目錄>`，16:02退出0。
- MSVC 14.29.30133，實際探針`_MSC_FULL_VER=192930159 x64`；空Cargo快取與frozen/vendor、格式、Clippy、394個非CAD Rust測試通過，50個CAD測試明確跳過。
- 原生代理28案、斷線恢復9案、圖片14案、舊契約、PDF、歷程恢復、真Python及完整Office、圖片嵌入、報表綁定、來源重跑、本機SMB與兩種VNC模式全部通過。VNC未另做畫面目視驗收，公司網路／信箱／模型未測。
- 本輪重新執行設定頁及55項互動教學、版本公告窄視窗、21項工作介面檢查，全部通過；其他既有UI結果依前一輪候選紀錄，不冒稱全數重新執行。
- NSIS安裝／更新／自動重啟／鎖檔失敗回復／三種Python修復／解除安裝與使用者資料保留全部通過，實際發行Setup也完成安裝與移除。隨附CAD runtime完整性核對通過，不啟動CAD或驗證圖面。
- 更新清單簽章由正式應用驗證通過，兩份更新清單指向相同NSIS與上述雜湊。

公開證據：[建置](../offline/exe-verification-0.8.76.json)、[NSIS](../offline/installer-verification-0.8.76.json)、[功能與模型限制](../offline/feature-verification-0.8.76.json)。本機完整原始紀錄`.build/release-0.8.76-r1.log`不納入發行包。

交付使用既有origin/main與Git LFS；來源、文件、EXE、Setup、更新清單及本輪公開驗證一起提交。遠端完成狀態以提交及LFS重新下載雜湊核對為準。不更新歷史CompanyAI-offline.zip，不納入私人測試資料、登入資訊或個人設定。
