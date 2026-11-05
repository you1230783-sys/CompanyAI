"""隔離的單次分析入口；資料只走管道，輸出由 Rust 驗證後發布。

此程式不把 Python 的物件限制當成安全沙箱；真正邊界是 Windows
AppContainer（無網路能力）與 Job Object（單一程序、記憶體及逾時限制）。
"""
import contextlib
import ctypes
from ctypes import wintypes
import io
import hashlib
import json
import struct
import sys
import traceback


def require_container():
    """拒絕一般權限直接啟動，避免測試或錯誤路由解除隔離。"""
    token = wintypes.HANDLE()
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    advapi = ctypes.WinDLL("advapi32", use_last_error=True)
    # x64 HANDLE 不能沿用 ctypes 的預設 int 參數，否則偽控制代碼會溢位。
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    advapi.OpenProcessToken.argtypes = [wintypes.HANDLE, wintypes.DWORD, ctypes.POINTER(wintypes.HANDLE)]
    advapi.GetTokenInformation.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
    if not advapi.OpenProcessToken(kernel.GetCurrentProcess(), 8, ctypes.byref(token)):
        raise RuntimeError("Cannot query process token")
    try:
        value = wintypes.DWORD()
        length = wintypes.DWORD()
        if not advapi.GetTokenInformation(token, 29, ctypes.byref(value), 4, ctypes.byref(length)) or value.value != 1:
            raise RuntimeError("Python worker requires AppContainer")
    finally:
        kernel.CloseHandle(token)


def frame(stream, payload):
    stream.write(struct.pack("<I", len(payload)))
    stream.write(payload)
    stream.flush()


class BoundedLog(io.StringIO):
    """只保留少量 print 輸出；大量資料應透過成果表交付。"""
    def write(self, text):
        remaining = max(0, 8000 - self.tell())
        super().write(text[:remaining])
        return len(text)


def analyze(request):
    """穩定資料介面：tables、texts、metadata、emit_table、emit_excel。"""
    import pandas as pd
    import numpy as np
    import openpyxl
    import scipy
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from pathlib import Path
    font_path = Path(__file__).parent / "LMFonts" / "jf-openhuninn-2.1.ttf"
    matplotlib.font_manager.fontManager.addfont(str(font_path))
    matplotlib.rcParams["font.family"] = matplotlib.font_manager.FontProperties(fname=str(font_path)).get_name()
    matplotlib.rcParams["axes.unicode_minus"] = False

    tables, texts, metadata = {}, {}, {}
    for item in request["inputs"]:
        name = item["name"]
        metadata[name] = {k: v for k, v in item.items() if k not in ("rows", "text", "hex")}
        if item["kind"] == "dataset":
            # 先用 object 保留文字、空白、布林與錯誤；模型依明確用途轉換欄位。
            tables[name] = pd.DataFrame(item["rows"], columns=item["columns"], dtype=object)
        elif item["kind"] == "csv":
            texts[name] = item["text"]
            tables[name] = pd.read_csv(io.StringIO(item["text"]), dtype=str, keep_default_na=False)
        elif item["kind"] == "xlsx":
            # 只接受 broker 核准的生成檔；公司原始 Office 必須先走 COM。
            book = openpyxl.load_workbook(io.BytesIO(bytes.fromhex(item["hex"])), read_only=True, data_only=True)
            try:
                metadata[name]["sheets"] = book.sheetnames
                for sheet in book:
                    values = sheet.iter_rows(values_only=True)
                    columns = next(values, ())
                    rows = []
                    for row in values:
                        if len(rows) >= 100000:
                            raise ValueError("XLSX exceeds 100000 rows; select a smaller source")
                        rows.append(row)
                    tables[name + "/" + sheet.title] = pd.DataFrame(rows, columns=columns, dtype=object)
            finally:
                book.close()
        else:
            texts[name] = item["text"]
    outputs = []

    def scalar(value):
        """JSON 不接受 NaN／Infinity，缺值保留 null，不當成零。"""
        if value is None or pd.isna(value):
            return None
        if isinstance(value, np.generic):
            value = value.item()
        if isinstance(value, (str, bool, int, float)):
            if isinstance(value, float) and not np.isfinite(value):
                raise ValueError("Infinite result is not supported")
            return value
        if hasattr(value, "isoformat"):
            return value.isoformat()
        raise ValueError("Unsupported cell type: " + type(value).__name__)

    def emit_table(name, data):
        """交付可供既有圖表工具讀取的 CSV 資料集，完整資料不回傳模型。"""
        if len(outputs) >= 8:
            raise ValueError("At most 8 outputs")
        df = pd.DataFrame(data)
        if len(df) > 100000 or not 1 <= len(df.columns) <= 16:
            raise ValueError("Table limit: 100000 rows / 16 columns")
        outputs.append({"kind": "table", "name": name, "columns": [str(c) for c in df.columns],
                        "rows": [[scalar(v) for v in row] for row in df.itertuples(index=False, name=None)]})

    def emit_excel(name, sheets):
        """以 openpyxl 產生多工作表；字串永遠寫成文字，不意外執行公式。"""
        if len(outputs) >= 8 or not 1 <= len(sheets) <= 12:
            raise ValueError("Output/workbook sheet limit exceeded")
        book = openpyxl.Workbook()
        book.remove(book.active)
        # 進度指紋保留工作表、型別與儲存格值，忽略檔名及ZIP建立時間。
        # 僅供停滯提示；實體交付仍由Rust依完整位元組雜湊核對。
        content_hash = hashlib.sha256()
        def remember(value):
            content_hash.update(json.dumps(value, ensure_ascii=False, allow_nan=False).encode("utf-8"))
            content_hash.update(b"\n")
        for title, data in sheets.items():
            df = pd.DataFrame(data)
            if len(df) > 100000 or not 1 <= len(df.columns) <= 32:
                raise ValueError("Excel limit: 100000 rows / 32 columns")
            sheet = book.create_sheet(str(title))
            remember(["sheet", sheet.title])
            for row in [[str(c) for c in df.columns]]:
                sheet.append(row)
                remember(["header", row])
            for row in df.itertuples(index=False, name=None):
                values = [scalar(v) for v in row]
                sheet.append(values)
                remember(["row", values])
            for row in sheet:
                for cell in row:
                    if isinstance(cell.value, str):
                        cell.data_type = "s"
            sheet.freeze_panes = "A2"
            sheet.auto_filter.ref = sheet.dimensions
            for cell in sheet[1]:
                cell.font = openpyxl.styles.Font(bold=True)
        buffer = io.BytesIO()
        book.save(buffer)
        if buffer.tell() > 8 * 1024 * 1024:
            raise ValueError("Excel output exceeds 8 MiB")
        # 同程序再開啟驗證容器與工作表，Rust 另核對實際寫入 bytes。
        check = openpyxl.load_workbook(io.BytesIO(buffer.getvalue()), read_only=True)
        names = check.sheetnames
        check.close()
        outputs.append({"kind": "xlsx", "name": name, "hex": buffer.getvalue().hex(),
                        "sheets": names, "content_sha256": content_hash.hexdigest()})

    def emit_figure(name, figure, dpi=150):
        """圖片只經記憶體管道交給原生層驗證／發布，不能自行寫專案路徑。"""
        if len(outputs) >= 8:
            raise ValueError("At most 8 outputs")
        dpi = float(dpi)
        if not np.isfinite(dpi) or not 50 <= dpi <= 300:
            raise ValueError("Figure DPI must be 50–300")
        size = figure.get_size_inches() * dpi
        if not np.all(np.isfinite(size)) or np.any(size < 1) or np.any(size > 4096):
            raise ValueError("Figure dimensions must be 1–4096 pixels")
        buffer = io.BytesIO()
        figure.savefig(buffer, format="png", dpi=dpi)
        payload = buffer.getvalue()
        if len(payload) > 8 * 1024 * 1024:
            raise ValueError("PNG output exceeds 8 MiB")
        outputs.append({"kind":"png", "name":name, "hex":payload.hex(),
                        "content_sha256":hashlib.sha256(payload).hexdigest()})
        plt.close(figure)

    scope = {"pd": pd, "np": np, "openpyxl": openpyxl, "scipy": scipy, "plt": plt, "io": io,
             "tables": tables, "texts": texts, "metadata": metadata,
             "emit_table": emit_table, "emit_excel": emit_excel, "emit_figure": emit_figure, "result": {}}
    log = BoundedLog()
    with contextlib.redirect_stdout(log), contextlib.redirect_stderr(log):
        exec(compile(request["code"], "<company-ai-analysis>", "exec"), scope)
    summary = normalize_summary(scope.get("result", {}))
    return {"ok": True, "summary": summary, "stdout": log.getvalue(), "outputs": outputs,
            "versions": {"python": sys.version.split()[0], "pandas": pd.__version__,
                         "numpy": np.__version__, "openpyxl": openpyxl.__version__,
                         "scipy": scipy.__version__, "matplotlib": matplotlib.__version__}}


def normalize_summary(value):
    """將小型 NumPy 結果轉成 JSON；保留錯誤位置，不把未知物件變成文字。"""
    import math
    import numpy as np

    active = set()
    remaining = 12000

    def convert(item, path, depth=0):
        nonlocal remaining
        remaining -= 1
        if remaining < 0 or depth > 64:
            raise ValueError(f"{path}: result 太大或巢狀過深；請用 emit_table 保存完整資料")
        if isinstance(item, np.bool_):
            item = bool(item)
        elif isinstance(item, np.integer):
            item = int(item)
        elif isinstance(item, np.floating):
            item = float(item)
        if item is None or isinstance(item, (bool, int)):
            return item
        if isinstance(item, float):
            if not math.isfinite(item):
                raise ValueError(f"{path}: NaN／Inf 不可作為結果；請轉成 None 或字串，並註明缺值原因")
            return item
        if isinstance(item, str):
            remaining -= len(item)
            if remaining < 0:
                raise ValueError(f"{path}: result exceeds 12000 characters; use emit_table for full data")
            return item
        if not isinstance(item, (dict, list, tuple, np.ndarray)):
            raise TypeError(f"{path}: 不支援 {type(item).__name__}；請明確轉成 JSON 值或使用 emit_table")
        identity = id(item)
        if identity in active:
            raise ValueError(f"{path}: result 不可循環引用")
        active.add(identity)
        try:
            if isinstance(item, np.ndarray):
                # 先限元素數，避免為最終必定超限的大陣列配置整份 Python list。
                if item.size > 12000:
                    raise ValueError(f"{path}: ndarray 太大；result 上限 12000 字元，請使用 emit_table")
                return convert(item.tolist(), path, depth + 1)
            if isinstance(item, dict):
                output = {}
                for key, child in item.items():
                    # 保留原本 JSON 可接受的鍵；非有限數鍵也不能繞過 NaN 檢查。
                    if not isinstance(key, (str, int, float, bool, type(None))):
                        raise TypeError(f"{path}: 結果鍵必須是 JSON 支援的型別")
                    if isinstance(key, float) and not math.isfinite(key):
                        raise ValueError(f"{path}[{key!r}]: NaN／Inf 鍵請轉成字串，並註明缺值原因")
                    remaining -= len(str(key))
                    output[key] = convert(child, f"{path}[{key!r}]", depth + 1)
                return output
            return [convert(child, f"{path}[{index}]", depth + 1)
                    for index, child in enumerate(item)]
        finally:
            active.remove(identity)

    normalized = convert(value, "result")
    encoded = json.dumps(normalized, ensure_ascii=False, allow_nan=False)
    if len(encoded) > 12000:
        raise ValueError("result exceeds 12000 characters; use emit_table for full data")
    return json.loads(encoded)


def main():
    require_container()
    reader, writer = sys.stdin.buffer, sys.stdout.buffer
    frame(writer, b"appcontainer-ready")
    length = struct.unpack("<I", reader.read(4))[0]
    if length > 64 * 1024 * 1024:
        raise ValueError("Input exceeds 64 MiB")
    payload = reader.read(length)
    if len(payload) != length:
        raise ValueError("Incomplete input")
    try:
        response = analyze(json.loads(payload))
    except BaseException:
        response = {"ok": False, "error": traceback.format_exc(limit=6)[-8000:]}
    encoded = json.dumps(response, ensure_ascii=False, allow_nan=False).encode("utf-8")
    if len(encoded) > 32 * 1024 * 1024:
        encoded = b'{"ok":false,"error":"Python output exceeds 32 MiB"}'
    frame(writer, encoded)


if __name__ == "__main__":
    main()
