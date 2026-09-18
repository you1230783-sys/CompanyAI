; -*- coding: utf-8 -*-
; 使用者端只執行 NSIS 內建操作、Windows API 與已隨包附上的原生 EXE。
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
!ifndef PRODUCT_DIR
!define PRODUCT_DIR "LM_AI"
!endif
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN.${PRODUCT_DIR}"
Name "LM_AI 公司 AI 助理"
OutFile "${SETUP_OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\${PRODUCT_DIR}"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 32
ShowInstDetails show
ShowUninstDetails show
VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey /LANG=1028 "CompanyName" "LARGAN"
VIAddVersionKey /LANG=1028 "ProductName" "LM_AI"
VIAddVersionKey /LANG=1028 "FileDescription" "公司 AI 助理安裝程式（Dev: 1230783）"
VIAddVersionKey /LANG=1028 "FileVersion" "${APP_VERSION}.0"
VIAddVersionKey /LANG=1028 "LegalCopyright" "Copyright © 2026 LARGAN. All rights reserved."
!define MUI_ICON "..\assets\app.ico"
!define MUI_UNICON "..\assets\app.ico"
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipUpdatePage
!define MUI_WELCOMEPAGE_TEXT "將安裝至目前使用者的應用程式目錄並建立捷徑。聊天、登入及偏好資料將保留。$\r$\n$\r$\n若電腦缺少 WebView2，會使用隨附的 Microsoft 離線安裝程式。"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipUpdatePage
!define MUI_FINISHPAGE_RUN "$INSTDIR\LM_AI.exe"
!define MUI_FINISHPAGE_RUN_TEXT "開啟 LM_AI"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH
!insertmacro MUI_LANGUAGE "TradChinese"
Var UpdatePid
Var Restart
Var InstanceLock
Var HadPrevious

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
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\${PRODUCT_DIR}"
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
    ; 不允許固定目錄是 junction/symlink，避免寫入使用者預期以外的位置。
    System::Call 'kernel32::GetFileAttributesW(w "$INSTDIR") i.r0'
    IntCmp $0 -1 directory_ok
    IntOp $1 $0 & 0x400
    IntCmp $1 0 directory_ok
    MessageBox MB_ICONSTOP "安裝目錄不可為連結。" /SD IDOK
    Abort
    directory_ok:
    InitPluginsDir
    SetOutPath "$PLUGINSDIR"
    File /oname=LM_AI.exe "${APP_SOURCE}"
    ; 測試包使用同一安裝程式邏輯，只以小型測試 EXE 替代 payload。
!ifndef TEST_PACKAGE
    ReadRegStr $0 HKCU "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
    StrCmp $0 "" 0 runtime_ready
    SetRegView 32
    ReadRegStr $0 HKLM "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
    SetRegView 64
    StrCmp $0 "" 0 runtime_ready
    File /oname=WebView2.exe "..\dist\MicrosoftEdgeWebView2RuntimeInstallerX64.exe"
    ExecWait '"$PLUGINSDIR\WebView2.exe" /silent /install' $0
    StrCmp $0 0 runtime_ready
    StrCmp $0 3010 runtime_ready
    MessageBox MB_ICONSTOP "WebView2 安裝失敗（$0），LM_AI 尚未替換。" /SD IDOK
    Abort
    runtime_ready:
!endif
    ClearErrors
    CreateDirectory "$INSTDIR"
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
    WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "LARGAN"
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
    MessageBox MB_ICONSTOP "安裝未完成。舊版檔案已保留；請檢查磁碟空間、寫入權限或防毒紀錄後重試。" /SD IDOK
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
    StrCmp $INSTDIR "$LOCALAPPDATA\Programs\${PRODUCT_DIR}" +3
    MessageBox MB_ICONSTOP "解除安裝位置不正確。" /SD IDOK
    Abort
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
