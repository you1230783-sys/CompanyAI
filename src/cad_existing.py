"""既有 DWG 的固定副本操作。來源檔由 Rust 保護，本程序只接收私有副本路徑。"""
from collections import Counter
from pathlib import Path
import math
import json
import hashlib


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
    kind = next((key for key in ("mtext", "text", "circle", "polyline", "line", "arc", "dimension", "blockreference")
                 if key in lower), "unknown")
    row = {"handle": str(entity.Handle).upper(), "type": name, "kind": kind, "properties": {}}
    for field in ("Layer", "Color", "TextString", "Height", "Radius", "Rotation", "Measurement", "Name"):
        value = optional(entity, field)
        if isinstance(value, (str, int, float, bool)):
            if isinstance(value, float) and not math.isfinite(value):
                continue
            if isinstance(value, str) and len(value) > 2000:
                row.setdefault("unavailable_properties", []).append(field)
            else:
                row["properties"][field] = value
    for field in ("StartPoint", "EndPoint", "Center", "InsertionPoint"):
        value = optional(entity, field)
        if value is not None:
            try:
                row["properties"][field] = api.coordinate(value)
            except (TypeError, ValueError, AttributeError):
                pass
    # 圖塊只讀名稱／位置／屬性。定義與巢狀內容不展開、不修改。
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
    row["editable"] = kind in ("line", "circle", "text", "mtext", "arc") and not row.get("unavailable_properties")
    return row


def snapshot(doc, api):
    entities, rows = {}, []
    size = 0
    for index, entity in enumerate(doc.ModelSpace):
        api.require(index < 200000, "初版最多讀取 200000 個 ModelSpace 圖元；請分割圖面")
        row = describe(entity, api)
        size += len(json.dumps(row, ensure_ascii=False).encode("utf-8"))
        api.require(size <= 64 * 1024 * 1024, "圖元索引超過 64 MB，請分割圖面")
        api.require(row["handle"] not in entities, "CAD 枚舉出重複 Handle，停止以免定位錯誤")
        entities[row["handle"]] = entity
        rows.append(row)
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
    report["step"] = "validate_edit"
    edits = request["edits"]
    api.require(isinstance(edits, list) and len(edits) <= 100, "每次最多修改 100 個圖元")
    additions = api.prepare(request["operations"]) if request["operations"] else []
    api.require(all(op["kind"] in ("line", "circle", "text", "leader") for op in additions),
                "副本初版只新增線、圓、文字及引線")
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
        if kind == "text":
            api.require(rows[handle]["kind"] in ("text", "mtext") and isinstance(value, str)
                        and 0 < len(value) <= 1000 and "\x00" not in value, "只能修改文字圖元的文字內容")
        elif kind == "color":
            api.require(type(value) is int and 1 <= value <= 255, "顏色需為 ACI 1–255")
        elif kind == "move":
            api.require(rows[handle]["kind"] in ("line", "circle", "text", "mtext"), "初版移動支援線／圓／文字")
            api.point_value(value)
            api.require(any(p in rows[handle]["properties"] for p in ("StartPoint", "Center", "InsertionPoint")), "缺少可核對的位置")
        else:
            raise ValueError("初版只支援 text/color/move")
        api.require(not bool(doc.Layers.Item(rows[handle]["properties"]["Layer"]).Lock), "圖元所在圖層已鎖定")
    drawing = api.Drawing.__new__(api.Drawing)
    drawing.app, drawing.doc, drawing.model = app, doc, doc.ModelSpace
    drawing.legacy, drawing.expected = legacy, []
    for index, edit in enumerate(edits):
        report.update(step="edit_entity", operation_index=index)
        entity, row = entities[edit["handle"]], rows[edit["handle"]]
        kind, value = edit["kind"], edit["value"]
        if kind == "text":
            entity.TextString = api.cad_text(value)
            row["properties"]["TextString"] = api.cad_text(value)
        elif kind == "color":
            entity.Color = value
            row["properties"]["Color"] = value
        else:
            delta = api.point_value(value)
            entity.Move(drawing.point([0., 0., 0.]), drawing.point(delta))
            for field in ("StartPoint", "EndPoint", "Center", "InsertionPoint"):
                if field in row["properties"]:
                    row["properties"][field] = [a+b for a,b in zip(row["properties"][field], delta)]
        report["completed_operations"] += 1
    for index, op in enumerate(additions):
        report.update(step=op["kind"], operation_index=len(edits)+index)
        drawing.draw(op)
        report["completed_operations"] += 1
    expected = {row["handle"]: row["properties"] for row in before}
    expected.update({row["handle"]: row["properties"] for row in drawing.expected})

    def verify(target):
        actual, _ = snapshot(target, api)
        api.require(set(actual) == set(expected), "副本 Handle 集合不符，不能宣稱保存完整")
        for handle, properties in expected.items():
            for field, value in properties.items():
                compare = api.text_equal if field == "TextString" else api.equal
                api.require(compare(getattr(actual[handle], field), value), f"副本屬性核對失敗：{handle}.{field}")
        return {"handles_verified": len(actual), "available_properties_verified": True,
                "unavailable_properties_not_verified": True}

    report["step"] = "verify_before_save"
    before_save = verify(doc)
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
