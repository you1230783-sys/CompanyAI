; -*- coding: utf-8 -*-
; 使用者端只執行 NSIS 內建操作與 Windows API；WebView2 由公司另外提供。
Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
!include "FileFunc.nsh"
!ifndef APP_VERSION
!error "APP_VERSION is required"
!endif
!ifndef APP_SOURCE
!define APP_SOURCE "..\dist\LM_AI.exe"
!endif
!ifndef SETUP_OUTPUT
!define SETUP_OUTPUT "..\dist\LM_AI_Setup.exe"
!endif
; 測試包可使用獨立目錄；正式包不接受編譯參數或 /D 改變公司指定位置。
!ifdef TEST_PACKAGE
!ifndef PRODUCT_DIR
!error "TEST_PACKAGE requires an isolated PRODUCT_DIR"
!endif
!else
!define PRODUCT_DIR "LM_AI"
!endif
!define INSTALL_ROOT "C:\largan"
!define INSTALL_PATH "${INSTALL_ROOT}\${PRODUCT_DIR}"
!define PUBLISHER "Largan, Inc."
; 舊登錄鍵與實例鎖是相容識別碼，保留以避免重複登錄或破壞更新交接。
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN.${PRODUCT_DIR}"
Name "LM_AI 公司 AI 助理"
OutFile "${SETUP_OUTPUT}"
InstallDir "${INSTALL_PATH}"
; 保持與一般權限 Outlook 相同的使用者環境；目錄權限由公司 IT 配置。
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 32
ShowInstDetails show
ShowUninstDetails show
VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey /LANG=1028 "CompanyName" "${PUBLISHER}"
VIAddVersionKey /LANG=1028 "ProductName" "LM_AI"
VIAddVersionKey /LANG=1028 "FileDescription" "公司 AI 助理安裝程式（Dev: 1230783）"
VIAddVersionKey /LANG=1028 "FileVersion" "${APP_VERSION}.0"
VIAddVersionKey /LANG=1028 "LegalCopyright" "Copyright © 2026 ${PUBLISHER} All rights reserved."
!define MUI_ICON "..\assets\app.ico"
!define MUI_UNICON "..\assets\app.ico"
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipUpdatePage
!define MUI_WELCOMEPAGE_TEXT "固定安裝至 ${INSTALL_PATH}\，並為目前使用者建立捷徑。聊天、登入及偏好資料將保留。$\r$\n$\r$\n請以一般權限安裝及執行；若無目錄寫入權限，請聯絡 IT 配置。本安裝程式只安裝 LM_AI，不檢查或安裝 WebView2。完成後請使用捷徑開啟 LM_AI；若缺少 WebView2，請使用公司另外提供的 x64 安裝程式。"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipUpdatePage
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH
!insertmacro MUI_LANGUAGE "TradChinese"
Var UpdatePid
Var Restart
Var InstanceLock
Var HadPrevious

; 同時檢查父目錄與產品目錄，避免固定路徑被 junction/symlink 導向別處。
; 安裝及解除安裝都先檢查，再進行檔案操作。
!macro CheckInstallDirectory path
    System::Call 'kernel32::GetFileAttributesW(w "${path}") i.r0'
    ${If} $0 != -1
        IntOp $1 $0 & 0x400
        ${If} $1 != 0
            MessageBox MB_ICONSTOP "安裝目錄不可為連結：${path}" /SD IDOK
            SetErrorLevel 1
            Abort
        ${EndIf}
    ${EndIf}
!macroend

Function SkipUpdatePage
    StrCmp $UpdatePid "" +2
    Abort
FunctionEnd

Function .onInit
    ${IfNot} ${RunningX64}
        MessageBox MB_ICONSTOP "LM_AI 需要 Windows x64。" /SD IDOK
        Abort
    ${EndIf}
    SetShellVarContext current
    SetRegView 64
    ; 固定位置，不把任意伺服器路徑交給安裝器。
    StrCpy $INSTDIR "${INSTALL_PATH}"
    !insertmacro CheckInstallDirectory "${INSTALL_ROOT}"
    !insertmacro CheckInstallDirectory "$INSTDIR"
    ${GetParameters} $0
    ${GetOptions} $0 "/UPDATEPID=" $UpdatePid
    ClearErrors
    ${GetOptions} $0 "/RESTART" $1
    IfErrors +2 0
    StrCpy $Restart "yes"
    StrCmp $UpdatePid "" lock_instance
    SetAutoClose true
    ; 不強制終止程序；60 秒仍未退出時保留舊版並顯示原因。
    System::Call 'kernel32::OpenProcess(i 0x100000, i 0, i $UpdatePid) p.r0 ?e'
    Pop $2
    StrCmp $0 0 process_gone
    System::Call 'kernel32::WaitForSingleObject(p r0, i 60000) i.r1'
    System::Call 'kernel32::CloseHandle(p r0)'
    StrCmp $1 0 lock_instance
    MessageBox MB_ICONSTOP "LM_AI 尚未結束，更新未執行。請退出後重新安裝。" /SD IDOK
    Abort
    process_gone:
    StrCmp $2 87 lock_instance
    MessageBox MB_ICONSTOP "無法確認主程式已退出，更新未執行。" /SD IDOK
    Abort
    lock_instance:
    ; 與 Rust 單一實例共用同一把鎖，防止替換期間重新開啟。
    System::Call 'kernel32::CreateMutexW(p 0, i 0, w "Local\LARGAN.${PRODUCT_DIR}.Desktop") p.r0 ?e'
    Pop $1
    StrCpy $InstanceLock $0
    StrCmp $0 0 lock_failed
    StrCmp $1 183 lock_failed
    Return
    lock_failed:
    MessageBox MB_ICONSTOP "LM_AI 正在執行或另一個安裝正在進行。請從系統托盤退出後再試。" /SD IDOK
    Abort
FunctionEnd

Section "安裝 LM_AI"
    !insertmacro CheckInstallDirectory "${INSTALL_ROOT}"
    !insertmacro CheckInstallDirectory "$INSTDIR"
    InitPluginsDir
    SetOutPath "$PLUGINSDIR"
    File /oname=LM_AI.exe "${APP_SOURCE}"
    ClearErrors
    ; NSIS CreateDirectory 會逐層建立缺少的父目錄；已存在的目錄可直接沿用。
    ; 先確認成功再準備替換檔案，不刪除原目錄或其中的其他資料。
    CreateDirectory "$INSTDIR"
    IfErrors failed
    CopyFiles /SILENT "$PLUGINSDIR\LM_AI.exe" "$INSTDIR\LM_AI.pending.exe"
    WriteUninstaller "$INSTDIR\Uninstall.pending.exe"
    IfErrors failed
    StrCpy $HadPrevious "no"
    IfFileExists "$INSTDIR\LM_AI.exe" 0 replace_app
    Delete "$INSTDIR\LM_AI.previous.exe"
    ClearErrors
    Rename "$INSTDIR\LM_AI.exe" "$INSTDIR\LM_AI.previous.exe"
    IfErrors failed
    StrCpy $HadPrevious "yes"
    replace_app:
    ClearErrors
    Rename "$INSTDIR\LM_AI.pending.exe" "$INSTDIR\LM_AI.exe"
    IfErrors rollback
    Delete "$INSTDIR\Uninstall.previous.exe"
    IfFileExists "$INSTDIR\Uninstall.exe" 0 replace_uninstaller
    ClearErrors
    Rename "$INSTDIR\Uninstall.exe" "$INSTDIR\Uninstall.previous.exe"
    IfErrors rollback
    replace_uninstaller:
    ClearErrors
    Rename "$INSTDIR\Uninstall.pending.exe" "$INSTDIR\Uninstall.exe"
    IfErrors rollback
    Delete "$INSTDIR\Uninstall.previous.exe"
    Delete "$INSTDIR\LM_AI.previous.exe"
    ; 清除舊的腳本式解除安裝入口，所有捷徑與登錄改為 NSIS 原生解除安裝。
    Delete "$INSTDIR\LM_AI_Uninstall.exe"
    ; 捷徑及重新啟動共用 $OUTDIR 作為工作目錄，不能指向稍後會刪除的解壓暫存。
    SetOutPath "$INSTDIR"
    CreateShortcut "$SMPROGRAMS\${PRODUCT_DIR}.lnk" "$INSTDIR\LM_AI.exe"
    CreateShortcut "$DESKTOP\${PRODUCT_DIR}.lnk" "$INSTDIR\LM_AI.exe"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${PRODUCT_DIR} 公司 AI 助理"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "${PUBLISHER}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${APP_VERSION}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\LM_AI.exe"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
    Goto finished
    rollback:
    Delete "$INSTDIR\LM_AI.exe"
    StrCmp $HadPrevious "yes" 0 +2
    Rename "$INSTDIR\LM_AI.previous.exe" "$INSTDIR\LM_AI.exe"
    IfFileExists "$INSTDIR\Uninstall.previous.exe" 0 failed
    Rename "$INSTDIR\Uninstall.previous.exe" "$INSTDIR\Uninstall.exe"
    failed:
    Delete "$INSTDIR\LM_AI.pending.exe"
    Delete "$INSTDIR\Uninstall.pending.exe"
    MessageBox MB_ICONSTOP "安裝未完成。舊版檔案已保留；請檢查磁碟空間及防毒紀錄。若無法寫入 ${INSTALL_PATH}\，請聯絡 IT 配置該目錄權限，再以一般權限重試。" /SD IDOK
    SetErrorLevel 1
    Abort
    finished:
SectionEnd

Function .onInstSuccess
    System::Call 'kernel32::CloseHandle(p $InstanceLock)'
    StrCpy $InstanceLock 0
    StrCmp $Restart "yes" 0 done
    Exec '"$INSTDIR\LM_AI.exe"'
    IfErrors 0 done
    MessageBox MB_ICONEXCLAMATION "更新已安裝，但無法自動開啟，請使用開始功能表的 LM_AI 捷徑。" /SD IDOK
    done:
FunctionEnd

Function un.onInit
    SetShellVarContext current
    SetRegView 64
    StrCmp $INSTDIR "${INSTALL_PATH}" +3
    MessageBox MB_ICONSTOP "解除安裝位置不正確。" /SD IDOK
    Abort
    !insertmacro CheckInstallDirectory "${INSTALL_ROOT}"
    !insertmacro CheckInstallDirectory "$INSTDIR"
    System::Call 'kernel32::CreateMutexW(p 0, i 0, w "Local\LARGAN.${PRODUCT_DIR}.Desktop") p.r0 ?e'
    Pop $1
    StrCpy $InstanceLock $0
    StrCmp $0 0 blocked
    StrCmp $1 183 blocked done
    blocked:
    MessageBox MB_ICONSTOP "請先從系統托盤退出 LM_AI，再解除安裝。" /SD IDOK
    Abort
    done:
FunctionEnd

Section "Uninstall"
    ClearErrors
    Delete "$INSTDIR\LM_AI.exe"
    IfErrors 0 +3
    MessageBox MB_ICONSTOP "無法刪除主程式，請退出 LM_AI 後重試。" /SD IDOK
    Abort
    Delete "$INSTDIR\LM_AI.previous.exe"
    Delete "$INSTDIR\LM_AI.pending.exe"
    Delete "$INSTDIR\LM_AI_Uninstall.exe"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR"
    Delete "$SMPROGRAMS\${PRODUCT_DIR}.lnk"
    Delete "$DESKTOP\${PRODUCT_DIR}.lnk"
    DeleteRegKey HKCU "${UNINSTALL_KEY}"
    ; 一律保留 CompanyAI 使用者資料，不遞迴刪除未知檔案。
SectionEnd
