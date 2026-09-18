// 最小 Win32 時鐘：不連網、不讀剪貼簿、不啟動其他程序、不寫入使用者資料。
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <strsafe.h>
#include <shellapi.h>
#include <cstdio>
#include "resource.h"

#if !defined(_MSC_VER) || _MSC_VER < 1920 || _MSC_VER > 1929
#error This project requires MSVC v142.
#endif
#ifndef _M_X64
#error This project requires an x64 compiler.
#endif
#define STRINGIFY_INNER(x) #x
#define STRINGIFY(x) STRINGIFY_INNER(x)
#pragma message("Verified _MSC_FULL_VER=" STRINGIFY(_MSC_FULL_VER) " x64")

namespace {
// 使用系統本機時間，不查詢網路時間服務。
void RefreshTime(HWND dialog) {
    SYSTEMTIME now{};
    GetLocalTime(&now);
    wchar_t value[80]{};
    if (SUCCEEDED(StringCchPrintfW(value, ARRAYSIZE(value),
        L"%04u-%02u-%02u  %02u:%02u:%02u",
        now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond))) {
        SetDlgItemTextW(dialog, IDC_TIME, value);
    }
}

INT_PTR CALLBACK DialogProc(HWND dialog, UINT message, WPARAM wparam, LPARAM) {
    switch (message) {
    case WM_INITDIALOG:
        SendMessageW(dialog, WM_SETICON, ICON_SMALL,
            reinterpret_cast<LPARAM>(LoadIconW(nullptr, IDI_INFORMATION)));
        RefreshTime(dialog);
        SetTimer(dialog, 1, 1000, nullptr);
        return TRUE;
    case WM_TIMER:
        RefreshTime(dialog);
        return TRUE;
    case WM_COMMAND:
        if (LOWORD(wparam) == IDC_REFRESH) {
            RefreshTime(dialog);
            return TRUE;
        }
        if (LOWORD(wparam) == IDCANCEL || LOWORD(wparam) == IDOK) {
            EndDialog(dialog, 0);
            return TRUE;
        }
        break;
    case WM_CLOSE:
        EndDialog(dialog, 0);
        return TRUE;
    case WM_DESTROY:
        KillTimer(dialog, 1);
        return TRUE;
    }
    return FALSE;
}
}

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE, PWSTR, int) {
    // 由 Windows 解析含引號的參數，避免啟動器替參數加引號後被誤當成正常開啟。
    int count = 0;
    PWSTR* arguments = CommandLineToArgvW(GetCommandLineW(), &count);
    const bool selfCheck = arguments && count == 2 && lstrcmpW(arguments[1], L"--self-check") == 0;
    if (arguments) LocalFree(arguments);
    if (selfCheck) {
        // 開發驗證實際建立隱藏的對話框並更新控制項；不對使用者顯示視窗。
        HWND dialog = CreateDialogParamW(instance, MAKEINTRESOURCEW(IDD_MAIN),
            nullptr, DialogProc, 0);
        if (!dialog) {
            std::fprintf(stderr, "CreateDialogParamW failed: %lu\n", GetLastError());
            return 1;
        }
        SendMessageW(dialog, WM_COMMAND, IDC_REFRESH, 0);
        wchar_t text[80]{};
        const int length = GetDlgItemTextW(dialog, IDC_TIME, text, ARRAYSIZE(text));
        DestroyWindow(dialog);
        std::fprintf(stderr, "Time control character count: %d (expected 20)\n", length);
        return length == 20 ? 0 : 2;
    }
    const INT_PTR result = DialogBoxParamW(instance, MAKEINTRESOURCEW(IDD_MAIN),
        nullptr, DialogProc, 0);
    if (result == -1) {
        MessageBoxW(nullptr, L"無法建立測試視窗。", L"LARGAN 安裝測試小工具", MB_ICONERROR);
        return 1;
    }
    return 0;
}
