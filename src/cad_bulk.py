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
    # ASCII JSON也能無損保留原生ANSI匯出中無法解碼的位元組代理字元。
    # 不用replacement，避免不同原始資料得到同一個版本指紋。
    return hashlib.sha256(json.dumps(data, ensure_ascii=True, sort_keys=True).encode("ascii")).hexdigest()


def has_undecodable_bytes(value):
    """檢查surrogateescape保留的原始位元組；這些內容只能交由原生COM詳讀。"""
    if isinstance(value, str):
        return any(0xDC80 <= ord(character) <= 0xDCFF for character in value)
    if isinstance(value, list):
        return any(has_undecodable_bytes(item) for item in value)
    return False


def ansi_lead_bytes(codepage):
    """取得Windows實際DBCS前導位元組範圍，不把Big5尾碼0x5C誤當反斜線。

    單位元組／UTF-8字碼頁沒有這些範圍，回傳空清單；查詢失敗即停止，
    不以猜測的範圍繼續輸出可能毀損的資料。
    """
    from ctypes import wintypes

    class CodePageInfo(ctypes.Structure):
        _fields_ = [('max_char_size', wintypes.UINT), ('default_char', ctypes.c_ubyte * 2),
                    ('lead_bytes', ctypes.c_ubyte * 12)]

    info = CodePageInfo()
    get_info = ctypes.windll.kernel32.GetCPInfo
    get_info.argtypes = [wintypes.UINT, ctypes.POINTER(CodePageInfo)]
    get_info.restype = wintypes.BOOL
    if not get_info(codepage, ctypes.byref(info)):
        raise ValueError('無法確認CAD匯出字碼頁的雙位元組範圍')
    ranges = []
    for index in range(0, len(info.lead_bytes), 2):
        low, high = info.lead_bytes[index:index + 2]
        if low == 0 and high == 0:
            break
        if not 128 <= low <= high <= 255:
            raise ValueError('CAD匯出字碼頁的雙位元組範圍不合法')
        ranges.extend((low, high))
    return ranges


def convert(data, children, legacy):
    """只轉換能與既有 COM 屬性對應的類型；傾斜 OCS 由 COM 讀取 WCS。"""
    # 部分ANSI LISP會把m²中的²寫成單一B2，與Big5中文字混用。
    # 僅這筆主／子圖元沿用COM描述器；不能猜字碼、刪字或重讀整張圖。
    if has_undecodable_bytes(data) or has_undecodable_bytes(children):
        return None
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
        # DXF可能保留CAD的Unicode表示法，例如\U+89C6；COM TextString則會回傳實際字元。
        # 僅對這類文字沿用COM描述器，不自行展開跳脫碼，以免誤改字面反斜線或MTEXT控制碼。
        # 一般圖形與不含這種表示法的文字仍走批次路徑。
        if "\\u+" in text.lower():
            return None
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
    # 保留每個無法解碼的位元組，不讓一個文字圖元阻斷整張幾何索引。
    # JSON結構仍嚴格解析；出問題的圖元不使用這份文字，而由COM取得原值。
    lines = [json.loads(line, parse_constant=invalid)
             for line in raw.decode(encoding, errors="surrogateescape").splitlines() if line]
    if not lines or lines[0][:3] != ["HEADER", 2, nonce] or len(lines[0]) != 8:
        raise ValueError("CAD 匯出標頭不符")
    if has_undecodable_bytes(lines[0]) or has_undecodable_bytes(lines[-1]):
        raise ValueError("CAD 匯出文件標頭或結尾字碼不符")
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


def ansi_lisp_representation(value, encoding):
    """由COM原值正向產生已實測的ANSI LISP表示法，未知表示法回傳None。

    實測2022的DWGNAME會將CP950以外的BMP字元回傳成\\U+XXXX，
    COM則保留原Unicode。以COM原值正向產生可接受的表示法，避免猜字、
    replacement或把其他文件當成同一份。尚未驗證的非BMP表示法仍拒絕。
    """
    encoded = []
    for character in value:
        try:
            character.encode(encoding)
        except UnicodeEncodeError:
            codepoint = ord(character)
            if not 0 <= codepoint <= 0xFFFF or 0xD800 <= codepoint <= 0xDFFF:
                return None
            encoded.append("\\U+" + format(codepoint, "04X"))
        else:
            encoded.append(character)
    return "".join(encoded)


def exported_name_matches(actual, expected, encoding):
    """文件名稱只接受原字串或該原值的表示法，不做一般反斜線解碼。"""
    if not isinstance(actual, str) or not isinstance(expected, str):
        return False
    if actual.casefold() == expected.casefold():
        return True
    encoded = ansi_lisp_representation(expected, encoding)
    return encoded is not None and actual.casefold() == encoded.casefold()


def exported_path_matches(name, prefix, full_name, encoding):
    """分開比對文件名稱及目錄，保留完整文件身分檢查。

    名稱可能以\\U+開頭，不能先用os.path.join：Windows會把開頭反斜線
    誤當根目錄而丟棄DWGPREFIX。目錄仍須絕對路徑，並完整核對正規化後的值。
    """
    if not isinstance(prefix, str) or not os.path.isabs(prefix):
        return False
    expected = os.path.abspath(full_name)
    if not exported_name_matches(name, os.path.basename(expected), encoding):
        return False
    expected_prefix = os.path.dirname(expected)
    actual_prefix = os.path.normcase(os.path.normpath(prefix))
    if actual_prefix == os.path.normcase(expected_prefix):
        return True
    encoded_prefix = ansi_lisp_representation(expected_prefix, encoding)
    # 路徑分隔符與字元表示法可能產生相鄰反斜線，兩邊套相同路徑正規化。
    return encoded_prefix is not None and actual_prefix == os.path.normcase(os.path.normpath(encoded_prefix))


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
    codepage = ctypes.windll.kernel32.GetACP()
    encoding = "cp" + str(codepage)
    # 實測2008與2022均為ANSI LISP；支援LISPSYS的Unicode引擎不套用位元組切分。
    # 只查詢，不變更使用者的LISP環境。
    try:
        unicode_strings = int(doc.GetVariable('LISPSYS')) in (1, 2)
    except Exception:
        unicode_strings = False
    leads = [] if unicode_strings else ansi_lead_bytes(codepage)
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
        source = source.replace('@@LEAD_BYTES@@', '(' + ' '.join(map(str, leads)) + ')' if leads else 'nil')
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
            api.require(exported_path_matches(header[3], header[4], before[1], encoding), "CAD 內部匯出文件路徑不符")
        else:
            api.require(exported_name_matches(os.path.splitext(header[3])[0],
                                             os.path.splitext(before[0])[0], encoding),
                        "CAD 未儲存文件名稱不符")
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
            # 固定原生欄位供2008原位改字／直徑標註使用；不把一般尺寸誤當直徑。
            d = fields(data)
            if d.get(0) in ('TEXT', 'MTEXT'):
                row['native_text'] = ''.join(v for k, v in data if k == 3) + d.get(1, '')
            if (d.get(0) == 'DIMENSION' and d.get(70, -1) & 7 == 3
                    and d.get(210, [0., 0., 1.]) == [0., 0., 1.]
                    and 10 in d and 15 in d and d.get(1, '') in ('', '<>')):
                start, end = api.coordinate(d[10]), api.coordinate(d[15])
                if math.dist(start, end) > 0 and abs(start[2] - end[2]) < 1e-9:
                    row['properties'].update(DiameterStart=start, DiameterEnd=end,
                        MeasuredDiameter=math.dist(start, end), DimensionTextOverride=d.get(1, ''))
                    row['allowed_edits'] = ['diameter'] if legacy and guard is not None else ['delete']
            if legacy and guard is not None:
                # 2008備援沒有刪除／新增；逐圖元公告可執行動作，避免模型提出不可行的刪除方案。
                allowed = {'circle':['radius', 'color'], 'line':['color'], 'arc':['color'],
                           'text':['text', 'color'], 'mtext':['text', 'color']}.get(row['kind'], [])
                if row['kind'] == 'dimension' and 'DiameterStart' in row['properties']:
                    allowed = ['diameter']
                row['allowed_edits'] = allowed
                row['editable'] = bool(allowed) and row['editable']
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
