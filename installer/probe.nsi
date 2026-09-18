; -*- coding: utf-8 -*-
; 全部安裝操作使用 NSIS 內建指令；不呼叫命令列、腳本或第三方元件。
Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
Name "LARGAN 安裝測試小工具"
OutFile "..\dist\LARGAN_NSIS_Probe_Setup.exe"
InstallDir "$LOCALAPPDATA\Programs\LARGAN_NSIS_Probe"
RequestExecutionLevel user
SetCompressor /SOLID lzma
ShowInstDetails show
ShowUninstDetails show
VIProductVersion "1.0.0.0"
VIAddVersionKey /LANG=1028 "CompanyName" "LARGAN"
VIAddVersionKey /LANG=1028 "ProductName" "LARGAN NSIS Probe"
VIAddVersionKey /LANG=1028 "FileDescription" "NSIS 安裝驗證包（Dev: 1230783）"
VIAddVersionKey /LANG=1028 "FileVersion" "1.0.0.0"
VIAddVersionKey /LANG=1028 "LegalCopyright" "Copyright © 2026 LARGAN. All rights reserved."

!define MUI_WELCOMEPAGE_TEXT "這是獨立的安裝測試包，只安裝顯示本機時間的小工具。$\r$\n$\r$\n不連網，不安裝 WebView2，不呼叫 PowerShell 或 CMD，也不修改 LM_AI。$\r$\n$\r$\n將安裝至目前使用者的應用程式目錄，並建立開始功能表與解除安裝項目。"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\LARGAN_NSIS_Probe.exe"
!define MUI_FINISHPAGE_RUN_TEXT "開啟測試小工具"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH
!insertmacro MUI_LANGUAGE "TradChinese"

!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\LARGAN_NSIS_Probe"
Function .onInit
    ${IfNot} ${RunningX64}
        MessageBox MB_ICONSTOP "此測試工具需要 Windows x64。"
        Abort
    ${EndIf}
    ; 不接受任意安裝路徑，降低解除安裝測試影響到其他檔案的可能。
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\LARGAN_NSIS_Probe"
    SetShellVarContext current
    SetRegView 64
FunctionEnd

Section "安裝小工具"
    SetOutPath "$INSTDIR"
    File "..\dist\LARGAN_NSIS_Probe.exe"
    File /oname=測試說明.txt "..\dist\測試說明.txt"
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    CreateDirectory "$SMPROGRAMS\LARGAN NSIS Probe"
    CreateShortcut "$SMPROGRAMS\LARGAN NSIS Probe\安裝測試小工具.lnk" "$INSTDIR\LARGAN_NSIS_Probe.exe"
    CreateShortcut "$SMPROGRAMS\LARGAN NSIS Probe\解除安裝.lnk" "$INSTDIR\Uninstall.exe"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "LARGAN 安裝測試小工具"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "LARGAN"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "1.0.0"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\LARGAN_NSIS_Probe.exe"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Function un.onInit
    SetShellVarContext current
    SetRegView 64
    StrCmp $INSTDIR "$LOCALAPPDATA\Programs\LARGAN_NSIS_Probe" correct
    MessageBox MB_ICONSTOP "解除安裝程式的位置不正確，請從原安裝目錄執行。"
    Abort
    correct:
FunctionEnd

Section "Uninstall"
    ; 只刪本測試安裝的檔案，RMDir 不遞迴，額外放入的檔案會保留。
    Delete "$INSTDIR\LARGAN_NSIS_Probe.exe"
    Delete "$INSTDIR\測試說明.txt"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR"
    Delete "$SMPROGRAMS\LARGAN NSIS Probe\安裝測試小工具.lnk"
    Delete "$SMPROGRAMS\LARGAN NSIS Probe\解除安裝.lnk"
    RMDir "$SMPROGRAMS\LARGAN NSIS Probe"
    DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
