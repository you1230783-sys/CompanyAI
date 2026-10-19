"""目前圖面入口：只操作固定文件，沿用副本的修改引擎，絕不 Save／Close。"""
import hashlib
import json
import os
import ctypes
from ctypes import wintypes
import time
import win32api
import win32process
import win32gui
from cad_existing import snapshot, edit_document, optional, iter_entities


def process_record(pid):
    """以有限查詢權限取得程序路徑／建立時間；可跨 64 位 Python 與 32 位 CAD。"""
    process = win32api.OpenProcess(0x1000, False, pid)
    try:
        query = ctypes.WinDLL("kernel32", use_last_error=True).QueryFullProcessImageNameW
        query.argtypes = [wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]
        query.restype = wintypes.BOOL
        buffer = ctypes.create_unicode_buffer(32768)
        length = wintypes.DWORD(len(buffer))
        if not query(int(process), 0, buffer, ctypes.byref(length)):
            raise ctypes.WinError(ctypes.get_last_error())
        return {"pid":int(pid), "created":str(win32process.GetProcessTimes(process)["CreationTime"]),
                "executable":os.path.normcase(os.path.abspath(buffer.value))}
    finally:
        process.Close()


def discover_process(app):
    """COM 不提供 HWND 時，只接受同一路徑的唯一 CAD 程序，且必須有桌面視窗。"""
    expected = optional(app, "FullName")
    if not isinstance(expected, str) or not os.path.isabs(expected):
        return None, "缺少 CAD 執行檔完整路徑"
    expected = os.path.normcase(os.path.abspath(expected))
    matches = []
    deadline = time.monotonic() + 3.0
    try:
        for index, pid in enumerate(win32process.EnumProcesses()):
            if index >= 8192 or time.monotonic() > deadline:
                return None, "CAD 程序辨識超過時間或數量上限"
            try:
                record = process_record(pid)
            except (OSError, win32api.error):
                continue  # 其他帳號／系統程序可能無查詢權限，不記錄其資訊。
            if record["executable"] == expected:
                matches.append(record)
        if len(matches) != 1:
            return None, "找不到唯一 CAD 程序（相同路徑符合數：%d）" % len(matches)
        windows = []
        def collect(hwnd, _):
            try:
                if win32process.GetWindowThreadProcessId(hwnd)[1] == matches[0]["pid"]:
                    windows.append(hwnd)
            except win32gui.error:
                pass  # 列舉期間視窗可能已關閉。
            return True
        win32gui.EnumWindows(collect, None)
        if not windows:
            return None, "符合的 CAD 程序沒有可辨識桌面視窗"
        return matches[0], None
    except (OSError, win32api.error):
        return None, "無法查詢 CAD 程序或桌面視窗"


def identity(app, doc, api):
    """優先使用文件視窗；舊版只有應用視窗時搭配文件名稱與內容快照。

    iCAD 2008 沒有 Document.HWND。此屬性不是閱讀圖元的必要條件；
    缺少 COM 視窗識別時改查 Windows 程序；程序／文件有歧義則只允許唯讀。
    """
    name, full_name = str(doc.Name), str(optional(doc, "FullName") or "")
    hwnd = optional(doc, "HWND")
    scope = "document_window"
    if not hwnd:
        hwnd = optional(app, "HWND")
        scope = "application_window_and_snapshot"
    if not hwnd or not win32gui.IsWindow(int(hwnd)):
        record, reason = discover_process(app)
        if record is None:
            return {"name": name, "full_name": full_name, "editable_session": False,
                    "identity_scope": "read_only_no_window", "identity_reason":reason}
        pid, created, hwnd = record["pid"], record["created"], None
        scope = "process_and_document_snapshot"
    else:
        hwnd = int(hwnd)
        _, pid = win32process.GetWindowThreadProcessId(hwnd)
        try:
            process = win32api.OpenProcess(0x1000, False, pid)
            try:
                created = str(win32process.GetProcessTimes(process)["CreationTime"])
            finally:
                process.Close()
        except win32api.error:
            return {"name": name, "full_name": full_name, "editable_session": False,
                    "identity_scope": "read_only_process_unavailable", "identity_reason":"無法查詢程序建立時間"}
    # 應用視窗不能區分同名文件。若名稱／路徑重複，僅提供唯讀結果。
    unique = scope == "document_window" or sum(
        str(item.Name) == name and str(optional(item, "FullName") or "") == full_name
        for item in iter_entities(app.Documents)) == 1
    return {"pid": pid, "created": created, "hwnd": hwnd,
            "name": name, "full_name": full_name, "editable_session": unique,
            "identity_scope": scope,
            "identity_reason":None if unique else "目前文件名稱／路徑不唯一"}


def revision(doc, rows):
    """每輪重讀可讀屬性與圖層鎖定，拒絕在使用者手動改圖後套用舊計畫。"""
    layers = sorted({str(row["properties"].get("Layer", "")) for row in rows})
    state = {"rows": sorted(rows, key=lambda row: row["handle"]),
             "units": doc.GetVariable("INSUNITS"),
             "locks": {name: bool(doc.Layers.Item(name).Lock) for name in layers}}
    return hashlib.sha256(json.dumps(state, sort_keys=True, ensure_ascii=False).encode("utf-8")).hexdigest()


def execute_active(app, legacy, request, report, api):
    report["step"] = "read_active"
    doc = app.ActiveDocument
    current = identity(app, doc, api)

    def guard():
        api.require(app.ActiveDocument._oleobj_ == doc._oleobj_
                    and identity(app, doc, api) == current,
                    "作用中圖面已切換或關閉，停止操作；請重新讀取")

    guard()
    if request["action"] == "active_probe":
        # 查詢快照前只核對文件身分／數量／單位，不再掃描 ModelSpace。
        # 同數量的手動編輯無可靠通用變更序號；快照標為歷史資料，寫入仍完整核對 revision。
        return {"status": "ACTIVE_PROBE", "document": current,
                "entity_count": int(doc.ModelSpace.Count), "units_code": doc.GetVariable("INSUNITS")}
    # 批次之間只核對 COM 文件物件，完整程序／文件身分仍於前後核對。
    def batch_guard():
        api.require(app.ActiveDocument._oleobj_ == doc._oleobj_,
                    "讀取期間作用中圖面已切換或關閉；部分索引已捨棄")

    entities, rows = snapshot(doc, api, guard=batch_guard)
    stamp = revision(doc, rows)
    guard()
    if request["action"] == "active_snapshot":
        return {"status": "READ_ACTIVE", "document": current, "revision": stamp,
                "edit_capabilities": edit_capabilities(app, doc, legacy),
                "units_code": doc.GetVariable("INSUNITS"), "entity_count": len(rows),
                "reported_count": int(doc.ModelSpace.Count), "entities": rows,
                "saved": False, "scope": "ModelSpace; currently readable properties only"}
    if request["action"] == "active_read":
        offset = request["offset"]
        api.require(type(offset) is int and 0 <= offset <= len(rows), "分頁超出圖元數量")
        if offset:
            api.require(request["document"] == current and request["revision"] == stamp,
                        "目前圖面已變更，請重新讀取第一頁")
        return {"status": "READ_ACTIVE", "document": current, "revision": stamp,
                "units_code": doc.GetVariable("INSUNITS"), "entity_count": len(rows),
                "entities": rows[offset:offset+100],
                "next_offset": offset+100 if offset+100 < len(rows) else None,
                "saved": False, "scope": "ModelSpace; currently readable properties only",
                "advice": ("此為目前圖面（含未儲存內容）。縮放等全圖指令只需本頁文件／revision，不必讀完全部圖元或工作紀錄。"
                           + ("修改前桌面確認，不自動儲存。" if current.get("editable_session") else
                              "目前無法授權操作：" + str(current.get("identity_reason")) + "。請直接回報，不重送編輯、不逐頁回讀紀錄；不自行判定使用者視窗操作錯誤。"))}
    api.require(request["action"] == "active_edit", "未知目前圖面操作")
    api.require(current.get("editable_session", True),
                "此 CAD 未提供可區分文件的視窗身分；可以閱讀，請改用 DWG 副本修改")
    api.require(request["document"] == current and request["revision"] == stamp,
                "文件身分或內容已變更；未修改，請重新讀取目前圖面")
    report.update(existing_documents_targeted=True, document=current)
    # guard 在首個寫入前及各操作前執行；失敗不自動 Undo，避免撤銷使用者的操作。
    result = edit_document(app, doc, legacy, request, report, api,
                           entities, rows, guard=guard)
    result["document"] = current
    return result


def edit_capabilities(app, doc, legacy):
    """只查詢介面，不執行 Undo；真正寫入仍由編輯引擎完整前置檢查。"""
    if not legacy and callable(optional(doc, 'SendCommand')):
        return dict(method='native_command_group_with_com_edits', scope='原有已讀圖元修改及新增；群組涵蓋完整核對')
    if callable(optional(doc, 'StartUndoMark')) and callable(optional(doc, 'EndUndoMark')):
        return dict(method='com_start_end_marks', scope='原有已讀圖元修改及新增；仍需確認與原生核對')
    if legacy and callable(optional(app, 'RunCommand')) and callable(optional(app, 'LoadLISP')):
        return dict(method='icad_runcommand_native', edits={'radius':['circle'],
            'color':['line','circle','arc','text','mtext']}, additions=False,
            notice='舊版原生Undo備援僅支援圓半徑及ACI顏色；不支援TrueColor顏色覆寫、其他修改或混合操作')
    return dict(method='unavailable', notice='缺少已驗證的Undo方法，不可修改目前圖面')
