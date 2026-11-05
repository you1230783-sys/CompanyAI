# 0.9.8 驗證與發行紀錄

狀態：配置量測完成，選定ThinLTO／16 units；正式完整驗證、封裝與發行尚未完成。不得以BuildOnly量測結果宣稱發行驗收通過。

本版D02b：詳見[配置選擇與可重跑方式](RELEASE_PROFILES.md)及[三組原始量測](../offline/profile-measurements-0.9.8.json)。保留panic unwind、v142 x64、靜態CRT及既有功能。C01／F08／F09排入0.9.9，D03排入0.9.10。

預定正式命令：`scripts/Build.ps1 -EmptyCargoCache -IncludeInstaller -TestOffice`。來源／成品、測試、Office／NSIS、GitHub同提交CI及公開下載結果須在實際完成後填入。

公司模型、真實信箱、加密Office、公司SMB／VNC／防毒與人工DPI仍屬公司實機驗收；不以本機合成契約測試冒充。ttf-parser停止維護例外與期限依既有CI政策。
