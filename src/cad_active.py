"""目前圖面入口：只操作固定文件，沿用副本的修改引擎，絕不 Save／Close。"""
import hashlib
import json
import win32api
import win32process
import win32gui
from cad_existing import snapshot, edit_document, optional


def identity(app, doc, api):
    """優先使用文件視窗；舊版只有應用視窗時搭配文件名稱與內容快照。

    iCAD 2008 沒有 Document.HWND。此屬性不是閱讀圖元的必要條件；
    完全缺少視窗識別時仍允許唯讀，但不能取得目前文件寫入授權。
    """
    name, full_name = str(doc.Name), str(optional(doc, "FullName") or "")
    hwnd = optional(doc, "HWND")
    scope = "document_window"
    if not hwnd:
        hwnd = optional(app, "HWND")
        scope = "application_window_and_snapshot"
    if not hwnd or not win32gui.IsWindow(int(hwnd)):
        return {"name": name, "full_name": full_name, "editable_session": False,
                "identity_scope": "read_only_no_window"}
    hwnd = int(hwnd)
    _, pid = win32process.GetWindowThreadProcessId(hwnd)
    try:
        process = win32api.OpenProcess(0x0400, False, pid)
        try:
            created = str(win32process.GetProcessTimes(process)["CreationTime"])
        finally:
            process.Close()
    except win32api.error:
        # 程序權限不足也不能妨礙唯讀；不以缺少建立時間的 PID 授權寫入。
        return {"name": name, "full_name": full_name, "editable_session": False,
                "identity_scope": "read_only_process_unavailable"}
    # 應用視窗不能區分同名文件。若名稱／路徑重複，僅提供唯讀結果。
    unique = scope == "document_window" or sum(
        str(item.Name) == name and str(optional(item, "FullName") or "") == full_name
        for item in app.Documents) == 1
    return {"pid": pid, "created": created, "hwnd": hwnd,
            "name": name, "full_name": full_name, "editable_session": unique,
            "identity_scope": scope}


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
    entities, rows = snapshot(doc, api)
    stamp = revision(doc, rows)
    guard()
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
                "advice": "此為目前圖面（含未儲存內容）；修改前桌面會確認文件，不自動儲存。"}
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
