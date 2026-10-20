"""唯讀枚舉 CAD COM 註冊；產品年份與內部 ProgID 版本明確分開。"""
import os
import re
import winreg

APPLICATION = re.compile(r"^(?:gstarcad|gstaricad|gcad|intellicad|icad)\.application(?:\.\d+)*$", re.I)


def year_from(*texts):
    """只採用登錄／產品名稱／安裝路徑明示的四位年份，不將 .27 猜成 2027。"""
    years = [int(y) for text in texts for y in re.findall(r"(?<!\d)(20\d{2}|199\d)(?!\d)", str(text))]
    return max(years, default=0)


def preference(item):
    """預設新版優先；無年份的項目依內部版本排序並保留未知標示。"""
    version = tuple(int(n) for n in re.findall(r"\d+", item["progid"]))
    return (item["year"], version, item["registry_bits"] == 64, item["progid"].lower())


def executable(command):
    command = os.path.expandvars(command.strip())
    match = re.match(r'^"([^"]+\.exe)"', command, re.I)
    if not match:
        match = re.match(r"^(.+?\.exe)(?:\s|$)", command, re.I)
    return match.group(1) if match else ""


def discover():
    """枚舉 HKCR 的32／64位元視圖，不啟動、不註冊或修復任何 CAD。"""
    import win32api
    found = []
    errors = []
    for bits, view in [(64, winreg.KEY_WOW64_64KEY), (32, winreg.KEY_WOW64_32KEY)]:
        def value(path):
            try:
                with winreg.OpenKey(winreg.HKEY_CLASSES_ROOT, path, 0, winreg.KEY_READ | view) as key:
                    return str(winreg.QueryValueEx(key, "")[0])
            except OSError:
                return ""
        try:
            with winreg.OpenKey(winreg.HKEY_CLASSES_ROOT, "", 0, winreg.KEY_READ | view) as root:
                for index in range(winreg.QueryInfoKey(root)[0]):
                    progid = winreg.EnumKey(root, index)
                    if not APPLICATION.fullmatch(progid):
                        continue
                    clsid = value(progid + r"\CLSID")
                    server = executable(value("CLSID\\" + clsid + r"\LocalServer32"))
                    description = value(progid)
                    product = ""
                    product_version = ""
                    if server and os.path.isfile(server):
                        try:
                            translations = win32api.GetFileVersionInfo(server, r"\VarFileInfo\Translation")
                            language, codepage = translations[0]
                            prefix = f"\\StringFileInfo\\{language:04x}{codepage:04x}\\"
                            product = win32api.GetFileVersionInfo(server, prefix + "ProductName")
                            product_version = win32api.GetFileVersionInfo(server, prefix + "ProductVersion")
                        except (OSError, win32api.error, IndexError):
                            pass
                    found.append(dict(progid=progid, clsid=clsid, registry_bits=bits,
                        server=server, server_exists=os.path.isfile(server), description=description,
                        product=product, product_version=product_version,
                        year=year_from(description, product, server, product_version)))
        except OSError as error:
            errors.append(dict(registry_bits=bits, error=str(error)))
    # 同一CLSID與伺服器的別名只顯示最高順位，避免使用者誤以為安裝了多套。
    unique = {}
    for item in sorted(found, key=preference, reverse=True):
        key = (item["clsid"].lower(), item["server"].lower())
        unique.setdefault(key, item)
    return list(unique.values()), errors
