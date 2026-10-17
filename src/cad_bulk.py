"""固定 CAD 內部匯出與本機索引轉換；不接受模型程式或任意 LISP 路徑。

已核對的原生類型由 DXF 轉換；其餘圖元使用既有 COM 描述器。
COM 物件按 Handle 延後取得，查詢整張圖不需要建立數萬個 Python COM 包裝。
"""
from collections.abc import Mapping
import ctypes
import hashlib
import json
import math
import os
from pathlib import Path
import tempfile
import time
import uuid


class Entities(Mapping):
    """集合／數量操作只使用本機 Handle；僅編輯與必要詳讀才呼叫 CAD。"""
    def __init__(self, doc, handles):
        self.doc, self.handles, self.objects = doc, dict.fromkeys(handles), {}

    def __iter__(self):
        return iter(self.handles)

    def __len__(self):
        return len(self.handles)

    def __getitem__(self, handle):
        if handle not in self.handles:
            raise KeyError(handle)
        if handle not in self.objects:
            self.objects[handle] = self.doc.HandleToObject(handle)
        return self.objects[handle]


def fields(data):
    if not isinstance(data, list) or any(not isinstance(p, list) or len(p) != 2 or type(p[0]) is not int for p in data):
        raise ValueError("CAD 批次圖元群組格式不符")
    return dict(data)


def digest(data):
    return hashlib.sha256(json.dumps(data, ensure_ascii=False, sort_keys=True).encode("utf-8")).hexdigest()


def convert(data, children, legacy):
    """只轉換能與既有 COM 屬性對應的類型；傾斜 OCS 由 COM 讀取 WCS。"""
    d = fields(data)
    kind = {"LINE":"line", "CIRCLE":"circle", "ARC":"arc", "TEXT":"text", "MTEXT":"mtext", "ATTRIB":"attribute",
            "LWPOLYLINE":"polyline", "POLYLINE":"polyline", "INSERT":"blockreference"}.get(d.get(0))
    normal = d.get(210, [0., 0., 1.])
    if kind is None or normal != [0., 0., 1.] or (kind in ("text", "attribute") and (d.get(72, 0) or d.get(74 if kind == "attribute" else 73, 0))):
        return None
    names = {"line":"Line", "circle":"Circle", "arc":"Arc", "text":"Text", "mtext":"Mtext" if legacy else "MText", "attribute":"Attribute",
             "polyline":"LightWeightPolyline" if d[0] == "LWPOLYLINE" else "Polyline", "blockreference":"BlockInsert" if legacy else "BlockReference"}
    name = names[kind] if legacy else "AcDb" + names[kind]
    if not legacy and kind == "polyline":
        name = "AcDbPolyline" if d[0] == "LWPOLYLINE" else "AcDb3dPolyline" if d.get(70,0) & 8 else "AcDb2dPolyline"
    p = dict(Layer=d.get(8, "0"), Color=d.get(62, 256), Linetype=d.get(6, "ByLayer"),
             LinetypeScale=d.get(48, 1.0), Normal=normal)
    row = dict(handle=str(d[5]).upper(), type=name, kind=kind, properties=p,
               editable=kind in ("line", "circle", "arc", "text", "mtext"))
    if kind != "mtext" and kind != "blockreference":
        p["Thickness"] = d.get(39, 0.0)
    if kind == "line":
        p.update(StartPoint=d[10], EndPoint=d[11])
    elif kind in ("circle", "arc"):
        p.update(Center=d[10], Radius=d[40])
        if kind == "arc":
            p.update(StartAngle=d[50], EndAngle=d[51])
            for field, angle in (("StartPoint", d[50]), ("EndPoint", d[51])):
                p[field] = [d[10][0]+d[40]*math.cos(angle), d[10][1]+d[40]*math.sin(angle), d[10][2]]
    elif kind in ("text", "mtext", "attribute"):
        text = ("".join(v for k, v in data if k == 3) if kind == "mtext" else "") + d.get(1, "")
        if len(text) > 2000:
            row.update(editable=False, unavailable_properties=["TextString"])
        else:
            p["TextString"] = text
        p.update(Height=d[40], InsertionPoint=d[10], StyleName=d.get(7, "STANDARD"), Rotation=d.get(50, 0.0))
        if kind in ("text", "attribute"):
            p["Alignment"] = 0
        else:
            p["AttachmentPoint"] = d.get(71, 1)
            if 50 not in d and 11 in d:
                p["Rotation"] = math.atan2(d[11][1], d[11][0]) % (2*math.pi)
    elif kind == "blockreference":
        p.update(Name=d[2], InsertionPoint=d[10], Rotation=d.get(50, 0.0))
        attrs = [fields(child) for child in children if fields(child).get(0) == "ATTRIB"]
        row["attributes"] = [{"tag":a.get(2, "")[:200], "text":a.get(1, "")[:2000]} for a in attrs[:100]]
        if len(attrs) > 100:
            row["attributes_truncated"] = True
    elif kind == "polyline":
        if d[0] == "LWPOLYLINE":
            points = [[v[0], v[1], d.get(38, 0.)] for k, v in data if k == 10]
            straight = all(abs(v) < 1e-12 for k, v in data if k == 42)
        else:
            vertices = [fields(child) for child in children if fields(child).get(0) == "VERTEX"]
            # 網格／polyface 不當成一般折線；保留摘要並退回 COM。
            if d.get(70, 0) & (2 | 4 | 16 | 64):
                return None
            points = [v[10] if d.get(70, 0) & 8 else [v[10][0], v[10][1], d.get(10, [0.,0.,0.])[2]] for v in vertices]
            straight = all(abs(v.get(42, 0.)) < 1e-12 for v in vertices)
        if straight and 2 <= len(points) <= 2048:
            if len(points) > 2 and points[0] == points[-1]:
                points.pop()
            row["geometry"] = dict(vertices=points, closed=bool(d.get(70, 0) & 1), coordinates="WCS", straight_segments=True)
        else:
            row["geometry_unavailable"] = True
    if kind == "attribute":
        row["kind"] = "unknown"  # 保留舊版唯讀範圍，不因此增加屬性修改授權。
    return row


def read_export(output, nonce, encoding):
    """完成後才解析；限制檔案大小，拒絕非有限數字、錯誤 nonce 與部分結果。"""
    def invalid(value):
        raise ValueError("CAD 匯出含非有限數值：" + value)
    with output.open("rb") as file:
        raw = file.read(128*1024*1024+1)
    if len(raw) > 128*1024*1024:
        raise ValueError("CAD 批次匯出超過 128 MiB，請分割圖面")
    lines = [json.loads(line, parse_constant=invalid) for line in raw.decode(encoding).splitlines() if line]
    if not lines or lines[0][:3] != ["HEADER", 2, nonce] or len(lines[0]) != 8:
        raise ValueError("CAD 匯出標頭不符")
    if lines[-1][:2] != ["COMPLETE", nonce] or len(lines[-1]) != 4 or lines[-1][2] != len(lines)-2:
        raise ValueError("CAD 匯出沒有完整結尾或數量不符")
    if lines[0][6] != lines[-1][3]:
        raise ValueError("CAD 匯出期間圖面已變更")
    rows = {}
    for record in lines[1:-1]:
        if len(record) != 3 or record[0] != "ENTITY" or (record[2] is not None and not isinstance(record[2], list)):
            raise ValueError("CAD 匯出圖元格式不符")
        record[2] = record[2] or []  # AutoLISP 的空清單以 JSON null 表示。
        d = fields(record[1])
        handle = str(d.get(5, "")).upper()
        if not handle or any(c not in "0123456789ABCDEF" for c in handle) or handle in rows:
            raise ValueError("CAD 匯出 Handle 缺少、重複或不合法")
        rows[handle] = record[1:]
    return lines[0], rows


def snapshot(doc, api, guard=None, handles_only=False, exclude_handles=frozenset()):
    """執行固定匯出器；逾時／格式錯誤不重送，明確數量不符才走 COM。"""
    import pythoncom
    from cad_existing import describe
    app, legacy = api.BULK_APP, api.BULK_LEGACY
    api.require(app.ActiveDocument._oleobj_ == doc._oleobj_, "CAD 文件已切換，未執行匯出")
    api.require(int(doc.GetVariable("CMDACTIVE")) == 0, "CAD 正在執行其他指令，請完成後重試")
    before = (str(doc.Name), str(doc.FullName or ""), int(doc.ModelSpace.Count),
              doc.GetVariable("INSUNITS"), doc.GetVariable("DBMOD"))
    api.require(before[2] <= 200000, "最多讀取 200000 個 ModelSpace 圖元，請分割圖面")
    # CAD 依本機字碼頁寫出；嚴格解碼，不用 replacement 隱藏中文字資料損失。
    encoding = "cp" + str(ctypes.windll.kernel32.GetACP())
    folder = Path(tempfile.mkdtemp(prefix="LM_AI_CAD_"))
    output, progress, script = folder/"entities.jsonl", folder/"progress.json", folder/"export.lsp"
    nonce = uuid.uuid4().hex
    notify = getattr(api, "read_progress", lambda *_: None)
    started = time.monotonic()
    try:
        source = Path(__file__).with_name("cad_export.lsp").read_text(encoding="utf-8")
        source = "\n".join(line for line in source.splitlines() if not line.lstrip().startswith(";"))
        # 固定模板、固定檔名；插入字串經 JSON 規則跳脫，不允許模型提供 LISP。
        def path_text(path):
            return json.dumps(path.as_posix(), ensure_ascii=False)[1:-1]
        source = source.replace("@@OUTPUT@@", path_text(output)).replace("@@PROGRESS@@", path_text(progress)).replace("@@NONCE@@", nonce).replace("@@HANDLES@@", "T" if handles_only else "nil")
        script.write_text(source, encoding=encoding)
        notify(0, before[2], "reading")
        if guard:
            guard()
        if legacy:
            app.LoadLISP(str(script))
        else:
            doc.SendCommand('(load "' + path_text(script) + '")\n')
        completed = 0
        while True:
            if progress.is_file():
                try:
                    p = json.loads(progress.read_text(encoding="ascii"))
                    if p[:2] == ["PROGRESS", nonce] and type(p[2]) is int:
                        completed = p[2]
                except (ValueError, OSError):
                    pass  # 可能正好在 CAD 重寫進度檔；不當成完成。
            notify(completed, before[2], "reading")
            if output.is_file():
                with output.open("rb") as file:
                    file.seek(max(0, output.stat().st_size-200))
                    if ('["COMPLETE","'+nonce+'"').encode("ascii") in file.read():
                        break
            api.require(time.monotonic()-started < 570, "CAD 內部匯出逾時；不自動重送，請檢查 CAD")
            pythoncom.PumpWaitingMessages()
            time.sleep(.1)
        header, native = read_export(output, nonce, encoding)
        if guard:
            guard()
        after = (str(doc.Name), str(doc.FullName or ""), int(doc.ModelSpace.Count),
                 doc.GetVariable("INSUNITS"), doc.GetVariable("DBMOD"))
        api.require(before == after and app.ActiveDocument._oleobj_ == doc._oleobj_, "CAD 匯出期間文件或狀態已變更")
        api.require(header[5] == before[3], "CAD 內部匯出單位不符")
        if before[1] and os.path.isabs(before[1]):
            api.require(os.path.normcase(os.path.abspath(os.path.join(header[4], header[3]))) == os.path.normcase(os.path.abspath(before[1])), "CAD 內部匯出文件路徑不符")
        else:
            api.require(Path(header[3]).stem == Path(before[0]).stem, "CAD 未儲存文件名稱不符")
        if legacy:
            # iCAD 2008 的 ModelSpace COM 另外列出附屬 ATTRIB；ssget 只列主 INSERT。
            # 依已匯出的真實子 Handle 補齊，不能用虛構數量或重新編號湊數。
            attached = {}
            for _, children in native.values():
                for child in children:
                    d = fields(child)
                    if d.get(0) == "ATTRIB":
                        attached[str(d[5]).upper()] = [child, []]
            for handle, record in attached.items():
                native.setdefault(handle, record)
        if len(native) != before[2]:
            # ssget 可能不包含特殊、不可選取的代理物件；不能把缺少的圖元當作不存在。
            # 只在明確數量不符時使用舊描述器，不因逾時／格式錯誤重送匯出。
            from cad_existing import com_snapshot
            objects, fallback = com_snapshot(doc, api, guard)
            return objects, [] if handles_only else [r for r in fallback if r["handle"] not in exclude_handles]
        entities, rows = Entities(doc, native), []
        if handles_only:
            return entities, rows
        size = 0
        for handle, (data, children) in native.items():
            if handle in exclude_handles:
                continue
            row = convert(data, children, legacy)
            if row is None:
                row = describe(entities[handle], api)
            row["native_revision"] = digest([data, children])
            size += len(json.dumps(row, ensure_ascii=False).encode("utf-8"))
            api.require(size <= 64*1024*1024, "CAD 本機索引超過 64 MiB，請分割圖面")
            rows.append(row)
        notify(len(rows), before[2], "checking")
        return entities, rows
    finally:
        notify(0, before[2], "idle")
        # 只刪除這次建立的固定檔案；不遞迴處理 CAD 或使用者資料夾。
        for path in (output, progress, script):
            try:
                path.unlink(missing_ok=True)
            except OSError:
                pass
        try:
            folder.rmdir()
        except OSError:
            pass
