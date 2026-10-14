"""目前圖面入口：只操作固定文件，沿用副本的修改引擎，絕不 Save／Close。"""
import hashlib
import json
import win32api
import win32process
import win32gui
from cad_existing import snapshot, edit_document, optional


def identity(app, doc, api):
    """程序建立時間與文件視窗共同識別工作階段；不以檔名作為唯一身分。"""
    hwnd = int(doc.HWND)
    api.require(hwnd != 0 and win32gui.IsWindow(hwnd), "CAD 不提供有效文件視窗，無法安全編輯目前圖面")
    _, pid = win32process.GetWindowThreadProcessId(hwnd)
    process = win32api.OpenProcess(0x0400, False, pid)
    try:
        created = str(win32process.GetProcessTimes(process)["CreationTime"])
    finally:
        process.Close()
    return {"pid": pid, "created": created, "hwnd": hwnd,
            "name": str(doc.Name), "full_name": str(optional(doc, "FullName") or "")}


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
    api.require(request["document"] == current and request["revision"] == stamp,
                "文件身分或內容已變更；未修改，請重新讀取目前圖面")
    report.update(existing_documents_targeted=True, document=current)
    # guard 在首個寫入前及各操作前執行；失敗不自動 Undo，避免撤銷使用者的操作。
    result = edit_document(app, doc, legacy, request, report, api,
                           entities, rows, guard=guard)
    result["document"] = current
    return result
