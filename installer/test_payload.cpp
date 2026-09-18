// 安裝交接測試專用，不加入正式安裝包。以 v142 x64 /MT 編譯。
#include <windows.h>
#include <string>
#include <cwchar>
#if !defined(_MSC_VER) || _MSC_VER < 1920 || _MSC_VER > 1929 || !defined(_M_X64)
#error Installer tests require MSVC v142 x64.
#endif
#ifndef TEST_GENERATION
#define TEST_GENERATION one
#endif
#define STRINGIFY_INNER(value) #value
#define STRINGIFY(value) STRINGIFY_INNER(value)
int wmain(int argc, wchar_t** argv) {
    HANDLE lock = CreateMutexW(nullptr, FALSE, L"Local\\LARGAN.LM_AI_Installer_Test.Desktop");
    if (!lock || GetLastError() == ERROR_ALREADY_EXISTS) return 2;
    if (argc == 2 && std::wcscmp(argv[1], L"--hold") == 0) {
        Sleep(2500);
    } else {
        wchar_t path[32768]{};
        if (!GetModuleFileNameW(nullptr, path, 32768)) return 3;
        std::wstring marker(path);
        marker += L".started";
        HANDLE file = CreateFileW(marker.c_str(), GENERIC_WRITE, FILE_SHARE_READ, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
        if (file == INVALID_HANDLE_VALUE) return 4;
        DWORD written = 0;
        const char generation[] = STRINGIFY(TEST_GENERATION);
        const BOOL success = WriteFile(file, generation, sizeof(generation) - 1, &written, nullptr);
        CloseHandle(file);
        if (!success) return 5;
    }
    CloseHandle(lock);
    return 0;
}
