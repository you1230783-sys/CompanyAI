"""共用圖元讀取／修改引擎；磁碟入口只接收 Rust 建立的私有副本路徑。"""
from collections import Counter
from pathlib import Path
import math
import json
import hashlib
import time
from itertools import islice
from contextlib import ExitStack

# 只縮減已知幾何類型的無關探測；文字、尺寸及未知圖元仍使用完整備援。
# 不快取「某屬性讀不到」的結果，避免把暫時 COM 錯誤套用到整張圖。
SCALAR_FIELDS = ("Layer", "Color", "Linetype", "LinetypeScale", "StyleName", "TextString", "Height", "Radius", "Rotation", "AttachmentPoint", "Alignment", "Measurement", "Name", "StartAngle", "EndAngle", "Thickness")
POINT_FIELDS = ("StartPoint", "EndPoint", "Center", "InsertionPoint", "Normal")
COMMON_FIELDS = ("Layer", "Color", "Linetype", "LinetypeScale", "Thickness")
GEOMETRY_FIELDS = {
    "line": (COMMON_FIELDS, ("StartPoint", "EndPoint", "Normal")),
    "circle": (COMMON_FIELDS + ("Radius",), ("Center", "Normal")),
    "arc": (COMMON_FIELDS + ("Radius", "StartAngle", "EndAngle"), ("StartPoint", "EndPoint", "Center", "Normal")),
}
READ_BATCH_SIZE = 50


def iter_entities(collection):
    """每次重設 COM 列舉器；舊版可能重用已走到尾端的 IEnumVARIANT。"""
    ole = getattr(collection, "_oleobj_", None)
    if ole is None:
        yield from collection
        return
    import pythoncom
    from win32com.client import util
    enum = ole.InvokeTypes(pythoncom.DISPID_NEWENUM, 0,
                           pythoncom.DISPATCH_METHOD | pythoncom.DISPATCH_PROPERTYGET,
                           (13, 10), ()).QueryInterface(pythoncom.IID_IEnumVARIANT)
    enum.Reset()
    yield from util.Iterator(enum)


def optional(obj, name):
    """舊版未實作的唯讀屬性明示為缺值，不推測幾何。"""
    try:
        return getattr(obj, name)
    except Exception:
        # COM 包裝庫會使用不同例外類型；只在唯讀探測處容許缺值。
        return None


def describe(entity, api):
    name = optional(entity, "ObjectName") or optional(entity, "EntityName") or "unknown"
    name = str(name)
    lower = name.lower()
    kind = next((key for key in ("mtext", "text", "circle", "polyline", "line", "arc", "dimension", "leader", "blockreference")
                 if key in lower), "unknown")
    if lower.startswith("dim"):
        kind = "dimension"  # 佑達 2008 使用 DimAligned／DimDiametric 名稱。
    row = {"handle": str(entity.Handle).upper(), "type": name, "kind": kind, "properties": {}}
    # 名稱含有 line 不一定是直線（例如 Spline／外掛圖元）；只有已知原生名稱縮減欄位。
    native_geometry = lower in ("line", "acdbline", "circle", "acdbcircle", "arc", "acdbarc")
    scalar_fields, point_fields = GEOMETRY_FIELDS.get(kind, (SCALAR_FIELDS, POINT_FIELDS)) if native_geometry else (SCALAR_FIELDS, POINT_FIELDS)
    for field in scalar_fields:
        value = optional(entity, field)
        if isinstance(value, (str, int, float, bool)):
            if isinstance(value, float) and not math.isfinite(value):
                continue
            if isinstance(value, str) and len(value) > 2000:
                row.setdefault("unavailable_properties", []).append(field)
            else:
                row["properties"][field] = value
    for field in point_fields:
        value = optional(entity, field)
        if value is not None:
            try:
                row["properties"][field] = api.coordinate(value)
            except (TypeError, ValueError, AttributeError):
                pass
    # 圖塊只讀名稱／位置／屬性。定義與巢狀內容不展開、不修改。
    if kind == "polyline":
        # 只接受可直接解釋為 WCS XY 的直線折線；有 bulge／傾斜法向量不猜圓弧。
        # 3D 折線、不可讀屬性及過大頂點集合仍保留原圖元摘要。
        normal = row["properties"].get("Normal")
        if normal and all(abs(normal[i] - (1 if i == 2 else 0)) < 1e-9 for i in range(3)):
            try:
                coords = list(entity.Coordinates)
                stride = 2 if "lwpolyline" in lower else 3
                count = len(coords) // stride
                if 2 <= count <= 2048 and len(coords) % stride == 0 and all(
                        isinstance(v, (float, int)) and math.isfinite(v) and abs(v) <= 1e12 for v in coords):
                    if all(abs(float(entity.GetBulge(i))) < 1e-12 for i in range(count)):
                        elevation = float(entity.Elevation) if stride == 2 else 0.0
                        if math.isfinite(elevation):
                            points = [[coords[i], coords[i+1], elevation if stride == 2 else coords[i+2]]
                                      for i in range(0, len(coords), stride)]
                            if len(points) > 2 and points[0] == points[-1]:
                                points.pop()
                            row["geometry"] = {"vertices": points, "closed": bool(entity.Closed),
                                               "coordinates": "WCS", "straight_segments": True}
            except Exception:
                row["geometry_unavailable"] = True
    if kind == "blockreference":
        row["attributes"] = []
        try:
            for index, attr in enumerate(entity.GetAttributes()):
                if index == 100:
                    row["attributes_truncated"] = True
                    break
                row["attributes"].append({"tag": str(attr.TagString)[:200], "text": str(attr.TextString)[:2000]})
        except Exception:
            row["attributes_unavailable"] = True
    row["editable"] = kind in ("line", "circle", "text", "mtext", "arc", "dimension", "leader") and not row.get("unavailable_properties")
    if kind in ("dimension", "leader"):
        row["allowed_edits"] = ["delete"]
    return row


def snapshot(doc, api, guard=None):
    """正式入口：批次建立完整索引；沒有原生介面的合成測試仍使用 COM。"""
    if getattr(api, "BULK_ENABLED", False):
        from cad_bulk import snapshot as bulk_snapshot
        return bulk_snapshot(doc, api, guard)
    return com_snapshot(doc, api, guard)


def com_snapshot(doc, api, guard=None):
    """同一 COM 執行緒分批累積索引；失敗時不交付部分結果、不自動續讀。

    每批檢查可觀察的文件／數量／單位／DBMOD 變更。DBMOD 不是遞增版號，
    因此仍不能偵測所有同數量編輯；修改前必須重新核對完整 revision。
    """
    entities, rows = {}, []
    size = 0
    model = doc.ModelSpace

    def observable_state():
        def variable(name):
            try:
                return doc.GetVariable(name)
            except Exception:
                return None  # 舊版缺少變更旗標時不捏造保障。
        return (optional(doc, "Name"), optional(doc, "FullName"), optional(model, "Count"),
                variable("INSUNITS"), variable("DBMOD"))

    initial = observable_state()
    progress = getattr(api, "read_progress", lambda *_: None)
    progress(0, initial[2], "reading")
    last_update = time.monotonic()
    iterator = iter(iter_entities(model))
    while True:
        if guard:
            guard()
        api.require(observable_state() == initial, "讀取期間圖面狀態已變更；部分索引已捨棄，請停止操作 CAD 後重新讀取")
        # 列舉器只重設一次；分批不再從第 0 個圖元掃描，也不把 COM 物件傳給其他執行緒。
        batch = list(islice(iterator, READ_BATCH_SIZE))
        if not batch:
            break
        for entity in batch:
            api.require(len(rows) < 200000, "初版最多讀取 200000 個 ModelSpace 圖元；請分割圖面")
            row = describe(entity, api)
            size += len(json.dumps(row, ensure_ascii=False).encode("utf-8"))
            api.require(size <= 64 * 1024 * 1024, "圖元索引超過 64 MB，請分割圖面")
            api.require(row["handle"] not in entities, "CAD 枚舉出重複 Handle，停止以免定位錯誤")
            entities[row["handle"]] = entity
            rows.append(row)
            if time.monotonic() - last_update >= 1.0:
                progress(len(rows), initial[2], "reading")
                last_update = time.monotonic()
        progress(len(rows), initial[2], "reading")
    progress(len(rows), initial[2], "checking")
    return entities, rows


def execute_existing(app, legacy, request, report, api):
    """開啟全新路徑的工作副本，不接管 ActiveDocument，不呼叫 Quit。"""
    output = Path(request["output"])
    api.require(output.is_absolute() and output.suffix.lower() == ".dwg" and output.is_file(), "缺少私有 DWG 副本")
    with output.open("rb") as source:
        api.require(hashlib.file_digest(source, "sha256").hexdigest() == request["source_revision"], "等待期間工作副本已變更，請重新建立副本")
    for existing in app.Documents:
        filename = optional(existing, "FullName")
        api.require(not filename or Path(str(filename)).resolve() != output.resolve(), "工作副本已被其他視窗開啟，未接管該文件")
    report.update(step="open_copy", drawing_started=True)
    doc = app.Documents.Open(str(output))
    api.require(Path(str(doc.FullName)).resolve() == output.resolve(), "CAD 開啟的不是指定副本")
    report["owned_document_created"] = True
    report["step"] = "read_copy"
    entities, before = snapshot(doc, api)
    if request["action"] == "read":
        offset, limit = request["offset"], request["limit"]
        api.require(type(offset) is int and offset >= 0 and type(limit) is int and 1 <= limit <= 100,
                    "分頁 offset >= 0，limit 需為 1–100")
        result = {"status": "READ_COPY", "scope": "ModelSpace only; no visual layout interpretation",
                  "units_code": doc.GetVariable("INSUNITS"),
                  "entity_count": len(before), "reported_count": int(doc.ModelSpace.Count),
                  "types": dict(Counter(row["type"] for row in before)),
                  "layers": dict(Counter(str(row["properties"].get("Layer", "unknown")) for row in before)),
                  "entities": before[offset:offset+limit],
                  "next_offset": offset+limit if offset+limit < len(before) else None}
        doc.Close(False)
        # 只建立一次完整本機索引；後續分頁不重複複製／開啟大型 DWG。
        index = dict(result, entities=before)
        output.with_name("cad-index.json").write_text(json.dumps(index, ensure_ascii=False), encoding="utf-8")
        return result
    return edit_document(app, doc, legacy, request, report, api, entities, before, output)


def edit_document(app, doc, legacy, request, report, api, entities, before, output=None, guard=None):
    """目前文件與磁碟副本共用同一驗證／修改引擎；output=None 絕不儲存或關閉。"""
    # 舊版原生群組必須涵蓋 Regen 與修改後核對；否則這些後續步驟可能
    # 成為新的 Undo 項目，一次復原便無法還原圖元。例外也必須結束本輪群組。
    with ExitStack() as cleanup:
        return _edit_document(app, doc, legacy, request, report, api, entities, before,
                              output, guard, cleanup)


def _edit_document(app, doc, legacy, request, report, api, entities, before, output, guard, cleanup):
    """執行已限定的修改；cleanup讓原生群組持續到整個核對流程結束。"""
    report["step"] = "validate_edit"
    edits = request["edits"]
    api.require(isinstance(edits, list) and len(edits) <= 100, "每次最多修改 100 個圖元")
    additions = api.prepare(request["operations"]) if request["operations"] else []
    api.require(edits or additions, "請提供修改或新增操作")
    # 先驗證整份修改計畫；同一 Handle 只能一項修改，避免順序歧義。
    rows = {row["handle"]: row for row in before}
    used = set()
    for edit in edits:
        api.require(set(edit) == {"handle", "kind", "value"}, "修改需 handle/kind/value")
        handle = edit["handle"]
        api.require(handle in rows and handle not in used and rows[handle]["editable"], "Handle 不存在、重複或不支援修改")
        used.add(handle)
        kind, value = edit["kind"], edit["value"]
        api.require("allowed_edits" not in rows[handle] or kind in rows[handle]["allowed_edits"],
                    "尺寸／引線目前僅支援依已讀 Handle 刪除後重建")
        if kind == "text":
            api.require(rows[handle]["kind"] in ("text", "mtext") and isinstance(value, str)
                        and 0 < len(value) <= 1000 and "\x00" not in value, "只能修改文字圖元的文字內容")
        elif kind == "color":
            api.require(type(value) is int and 1 <= value <= 255, "顏色需為 ACI 1–255")
        elif kind == "linetype":
            api.require(rows[handle]["kind"] in ("line", "circle", "arc"), "線型修改目前支援線／圓／弧")
            api.require(all(field in rows[handle]["properties"] for field in ("Linetype", "LinetypeScale")),
                        "缺少可核對的線型屬性，不能修改")
            api.prepare_line_style(value)
        elif kind == "move":
            api.require(rows[handle]["kind"] in ("line", "circle", "text", "mtext"), "初版移動支援線／圓／文字")
            api.point_value(value)
            api.require(any(p in rows[handle]["properties"] for p in ("StartPoint", "Center", "InsertionPoint")), "缺少可核對的位置")
        elif kind in ("radius", "height"):
            field = "Radius" if kind == "radius" else "Height"
            allowed = ("circle", "arc") if kind == "radius" else ("text", "mtext")
            api.require(rows[handle]["kind"] in allowed and field in rows[handle]["properties"], "缺少可核對的尺寸屬性")
            api.require(api.number(value) > 0, "尺寸必須大於零")
            if kind == "radius" and rows[handle]["kind"] == "arc":
                api.require(rows[handle]["properties"]["Radius"] > 0
                            and "Center" in rows[handle]["properties"], "缺少弧的中心／有效半徑")
        elif kind == "delete":
            api.require(value is None, "刪除的 value 需為 null")
        else:
            raise ValueError("只支援 text/color/linetype/move/radius/height/delete")
        api.require(not bool(doc.Layers.Item(rows[handle]["properties"]["Layer"]).Lock), "圖元所在圖層已鎖定")
    drawing = api.Drawing.__new__(api.Drawing)
    drawing.app, drawing.doc, drawing.model = app, doc, doc.ModelSpace
    drawing.legacy, drawing.expected = legacy, []
    drawing.report = report
    drawing.preflight_commands(additions)
    # 縮放只調整視野，不需要圖元編輯的 Undo 群組。嚴格限定目前文件的
    # 純 ZOOM 計畫；混入任何圖元操作、PURGE／AUDIT／REGEN 仍走下方保護。
    # 保留原生確認、文件／revision 核對及每次指令前後的文件身分檢查。
    view_only = (guard is not None and output is None and not edits and bool(additions)
                 and all(op["kind"] == "command" and op["command"] in ("ZOOM EXTENTS", "ZOOM ALL")
                         for op in additions))
    if view_only:
        report.update(view_only=True, undo_group_required=False)
        for index, op in enumerate(additions):
            guard()
            report.update(step="view_command", operation_index=index, drawing_started=True)
            # run_command 自行核對 ModelSpace／PaperSpace 的可讀屬性未改變。
            drawing.run_command(op["command"])
            guard()
            report["completed_operations"] += 1
        return {"status": "EDITED_UNSAVED", "saved": False, "view_only": True,
                "modified_handles": [], "deleted_handles": [], "added_entities": 0,
                "visual_check_required": True,
                "verification": {"readable_entities_unchanged": True},
                "advice": "縮放指令完成，未儲存、未關閉；可讀圖元內容核對未變。請目視確認顯示範圍。本輪未建立圖元 Undo 群組。"}
    deleted = set()
    native_applied = False
    # 目前圖面必須有可確認的 Undo 群組；原生指令需等待群組旗標，不能只看COM返回。
    if guard:
        guard()
        report["step"] = "prepare_undo_group"
        # 開始前先確認起訖方法都存在，避免只能開始、不能結束的群組。
        if not legacy and callable(optional(doc, 'SendCommand')):
            # 批次核對會進入CAD內部LISP，2022的COM Undo標記無法跨過它；
            # 使用已驗證的原生Begin／End，持續涵蓋寫入、Regen與完整核對。
            from cad_undo import begin_group
            cleanup.callback(begin_group(app, doc, report, guard))
        else:
            try:
                start_undo, end_undo = doc.StartUndoMark, doc.EndUndoMark
            except AttributeError:
                if not legacy:
                    raise
                if additions:
                    raise AttributeError("ICAD 2008 原生 Undo 備援不支援新增或混合指令，尚未修改")
                from cad_undo import apply as apply_legacy_edits
                cleanup.callback(apply_legacy_edits(app, doc, edits, rows, report, guard))
                for edit in edits:
                    field = "Radius" if edit["kind"] == "radius" else "Color"
                    rows[edit["handle"]]["properties"][field] = edit["value"]
                native_applied = True
            else:
                api.require(callable(start_undo) and callable(end_undo), "目前 CAD 不支援完整 Undo 群組，尚未修改圖元")
                start_undo()
                report.update(undo_group_started=True, undo_group_closed=False, undo_method="com_start_end_marks")
                def finish_com_group():
                    guard()
                    end_undo()
                    report["undo_group_closed"] = True
                cleanup.callback(finish_com_group)
    # 線型載入也屬於文件變更，須留在 Undo 群組內且先於任何圖元修改。
    drawing.prepare_linetypes(
        [op["line_style"] for op in additions if "line_style" in op]
        + [edit["value"] for edit in edits if edit["kind"] == "linetype"])
    if additions:
        # 樣式建立也是寫入：保留在目前圖面的 Undo 群組內，且先於刪改既有圖元。
        report.update(step="prepare_annotation_styles", drawing_started=True)
        drawing.prepare_styles(additions)
    for index, edit in enumerate([] if native_applied else edits):
        if guard:
            guard()
        report["drawing_started"] = True
        report.update(step="edit_entity", operation_index=index)
        entity, row = entities[edit["handle"]], rows[edit["handle"]]
        kind, value = edit["kind"], edit["value"]
        if kind == "text":
            entity.TextString = api.cad_text(value)
            row["properties"]["TextString"] = api.cad_text(value)
        elif kind == "color":
            entity.Color = value
            row["properties"]["Color"] = value
        elif kind == "linetype":
            row["properties"].update(drawing.apply_line_style(entity, value))
        elif kind in ("radius", "height"):
            field = "Radius" if kind == "radius" else "Height"
            if kind == "radius" and row["kind"] == "arc":
                # 弧的唯讀端點也隨半徑改變；依中心與原向量同比縮放，保留所在平面。
                old_radius = row["properties"]["Radius"]
                api.require(old_radius > 0 and "Center" in row["properties"], "缺少弧的中心／有效半徑")
                center = row["properties"]["Center"]
                for endpoint in ("StartPoint", "EndPoint"):
                    if endpoint in row["properties"]:
                        row["properties"][endpoint] = [c + (v-c)*value/old_radius
                            for c,v in zip(center,row["properties"][endpoint])]
            setattr(entity, field, api.number(value))
            row["properties"][field] = api.number(value)
        elif kind == "delete":
            entity.Delete()
            deleted.add(edit["handle"])
        else:
            delta = api.point_value(value)
            entity.Move(drawing.point([0., 0., 0.]), drawing.point(delta))
            for field in ("StartPoint", "EndPoint", "Center", "InsertionPoint"):
                if field in row["properties"]:
                    row["properties"][field] = [a+b for a,b in zip(row["properties"][field], delta)]
        report["completed_operations"] += 1
    for index, op in enumerate(additions):
        if guard:
            guard()
        report["drawing_started"] = True
        report.update(step=op["kind"], operation_index=len(edits)+index)
        drawing.draw(op)
        report["completed_operations"] += 1
    expected = {row["handle"]: row["properties"] for row in before if row["handle"] not in deleted}
    expected.update({row["handle"]: row["properties"] for row in drawing.expected})

    def verify(target):
        drawing.verify_styles(target)
        actual, actual_rows = snapshot(target, api)
        described = {r["handle"]: r for r in actual_rows}
        api.require(set(actual) == set(expected), "圖面 Handle 集合不符，不能宣稱操作完成")
        for row in drawing.expected:
            if "leader_handle" in row:
                linked = actual[row["leader_handle"]].Annotation
                api.require(str(linked.Handle).upper() == row["handle"], "引線附屬文字關聯不符")
        for handle, properties in expected.items():
            for field, value in properties.items():
                compare = api.text_equal if field == "TextString" else api.equal
                available = described[handle]["properties"]
                observed = available[field] if field in available else getattr(actual[handle], field)
                api.require(compare(observed, value), f"圖面屬性核對失敗：{handle}.{field}")
        # 未指定修改的主圖元及其子圖元原生資料也須保持一致。
        for previous in before:
            handle = previous["handle"]
            if handle not in used and previous.get("native_revision"):
                api.require(described[handle].get("native_revision") == previous["native_revision"],
                            f"未指定圖元的原生資料已變更：{handle}")
        return {"handles_verified": len(actual), "available_properties_verified": True,
                "unavailable_properties_not_verified": True}

    report["step"] = "verify_before_save"
    if guard:
        guard()
    doc.Regen() if legacy else doc.Regen(1)
    before_save = verify(doc)
    if output is None:
        return {"status": "EDITED_UNSAVED", "saved": False, "verification": before_save,
                "modified_handles": sorted(used), "deleted_handles": sorted(deleted),
                "added_entities": len(drawing.expected), "visual_check_required": True,
                "advice": "修改完成，尚未儲存。請查看 CAD；可用一次 Undo 撤銷本輪，儲存由使用者決定。"}
    report["step"] = "save"
    doc.Save()
    report["saved"] = True
    doc.Close(False)
    report["step"] = "reopen"
    reopened = app.Documents.Open(str(output))
    api.require(Path(str(reopened.FullName)).resolve() == output.resolve(), "重開圖面路徑不符")
    after_save = verify(reopened)
    reopened.Close(False)
    return {"status": "DATA_VERIFIED_VISUAL_PENDING", "before_save": before_save, "after_reopen": after_save,
            "visual_check_required": True, "modified_handles": sorted(used), "added_entities": len(drawing.expected),
            "scope": "ModelSpace only; block definitions, paper space and unavailable properties not verified"}
