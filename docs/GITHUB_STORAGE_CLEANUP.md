# GitHub 儲存空間整理

## 本輪範圍

本輪依使用者授權將LM_AI的main改為0.8.75起點，保留之後原始碼變更至0.9.1；停止在Git追蹤發行EXE、NSIS、WebView2與建置ZIP，必要字型改普通Git。安裝包與更新驗證JSON另放Releases。實際完成狀態及提交對照以 `offline/history-cleanup-0.9.1.json` 為準。

獨立的 `nsis-probe` 分支與 `nsis-probe-v1.0.0` Release不是舊LM_AI歷史，保留；它的二進位檔為小型相容性測試工具，沒有使用LFS。

## 遠端LFS回收仍需平台處理

[GitHub官方說明](https://docs.github.com/en/repositories/working-with-files/managing-large-files/removing-files-from-git-large-file-storage)指出，刪除Git歷史中的LFS引用後，舊LFS物件仍在遠端並計入額度。若不刪除／重建整個儲存庫，需聯絡GitHub支援清除。

本輪不刪除整個儲存庫，避免連帶刪除Release、議題、stars與fork關係；也沒有代使用者提交客服訊息。完成Git核對後，可由擁有者透過[GitHub Support](https://support.github.com/)提出申請。Git一般物件的伺服器快取／垃圾回收也由GitHub控制，force-push成功不代表舊SHA立即無法存取。

## 可供擁有者使用的申請文字

> Repository: https://github.com/you1230783-sys/CompanyAI
>
> We have migrated all retained source assets away from Git LFS and rewritten the application's main history to begin at version 0.8.75, retaining subsequent source changes through 0.9.1. Obsolete application branch references have been removed. Installer and update-manifest distribution now uses GitHub Releases.
>
> Please help purge the obsolete Git LFS objects that are no longer referenced by the repository's retained branches/tags, and confirm the resulting LFS storage usage. Please also advise whether old unreachable Git objects or cached commit views need additional server-side cleanup. Please preserve the repository, its Releases, and the independent nsis-probe branch/tag.
>
> We retain a local backup. The local LFS inventory contains 190 objects totaling 10,470,665,103 bytes; this is a local inventory, not a claim about the current billed server storage.

可附上 `offline/history-cleanup-0.9.1.json`。不要附上Token、私鑰、機台檔、個人設定或整份本機備份。

改寫後已有clone請重新clone，不要把舊歷史merge或force-push回main。原始碼clone不包含vendor與建置依賴，離線重建另見[離線建置說明](OFFLINE_AND_GIT.md)。
