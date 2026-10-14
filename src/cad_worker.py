"""CAD 固定操作工作程序。只接受 JSON 幾何，不執行模型提供的 Python 或 CAD 指令。"""
import ast
import json
import math
import operator
import os
from pathlib import Path
import sys
import traceback
import uuid

import pythoncom
from win32com.client import VARIANT, dynamic
import registry
from cad import identify_product

sys.dont_write_bytecode = True


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(value):
    require(type(value) in (int, float), "座標／尺寸必須是數字")
    value = float(value)
    require(math.isfinite(value) and abs(value) <= 1_000_000, "數值必須有限且不超過一百萬 mm")
    return value


def point_value(value):
    require(isinstance(value, list) and len(value) == 2, "2D 座標需為 [x,y]")
    return [number(value[0]), number(value[1]), 0.0]


def coordinate(value):
    if isinstance(value, (list, tuple)):
        return [float(v) for v in value]
    return [float(value.x), float(value.y), float(value.z)]


def equal(actual, expected):
    if isinstance(expected, list):
        actual = coordinate(actual)
        return len(actual) == len(expected) and all(equal(a, b) for a, b in zip(actual, expected))
    if isinstance(expected, str):
        return str(actual) == expected
    return math.isclose(float(actual), float(expected), rel_tol=1e-7, abs_tol=1e-6)


class Formula:
    """僅解讀算術 AST；不使用 eval，也不允許屬性、索引或任意函式呼叫。"""
    functions = {name: getattr(math, name) for name in ("sin", "cos", "tan", "sqrt", "exp", "log")}
    functions["abs"] = abs
    binary = {ast.Add: operator.add, ast.Sub: operator.sub, ast.Mult: operator.mul, ast.Div: operator.truediv}

    def __init__(self, source):
        require(isinstance(source, str) and 0 < len(source) <= 256, "公式長度需為 1–256 字元")
        self.tree = ast.parse(source, mode="eval").body
        require(sum(1 for _ in ast.walk(self.tree)) <= 80, "公式過於複雜")

    def at(self, t):
        def read(node):
            if isinstance(node, ast.Constant):
                return number(node.value)
            if isinstance(node, ast.Name) and node.id in ("t", "pi", "e"):
                return {"t": t, "pi": math.pi, "e": math.e}[node.id]
            if isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.UAdd, ast.USub)):
                value = read(node.operand)
                return value if isinstance(node.op, ast.UAdd) else -value
            if isinstance(node, ast.BinOp):
                left, right = read(node.left), read(node.right)
                if isinstance(node.op, ast.Pow):
                    require(abs(right) <= 16, "次方指數不得超過 ±16")
                    return number(left ** right)
                require(type(node.op) in self.binary, "不支援此算術運算")
                return number(self.binary[type(node.op)](left, right))
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
                require(node.func.id in self.functions and len(node.args) == 1 and not node.keywords, "只支援單參數數學函式")
                return number(self.functions[node.func.id](read(node.args[0])))
            raise ValueError("公式只允許 t、pi、e、算術及指定數學函式")
        return number(read(self.tree))


def prepare(operations):
    """整份計畫先驗證、計算公式點，再接觸 CAD，避免明顯錯誤造成部分繪圖。"""
    require(isinstance(operations, list) and 1 <= len(operations) <= 100, "每份圖面需 1–100 個操作")
    prepared = []
    budget = 0
    fields = {
        "line": "start end", "circle": "center radius", "text": "position text height",
        "mtext": "position text height width", "leader": "points text height",
        "aligned_dimension": "start end location", "diameter_dimension": "start end leader_length",
        "hatch": "boundary holes seed scale", "curve": "x y t_start t_end segments",
    }
    for source in operations:
        require(isinstance(source, dict), "每個操作必須是物件")
        op = dict(source)
        kind = op.get("kind")
        require(isinstance(kind, str) and kind in fields, "未知繪圖操作")
        require(set(op) == {"kind", "color"} | set(fields[kind].split()), "繪圖操作欄位缺漏或包含未知欄位")
        require(type(op["color"]) is int and 1 <= op["color"] <= 255, "顏色需為 ACI 1–255")
        for key in ("start", "end", "center", "position", "location"):
            if key in op:
                op[key] = point_value(op[key])
        for key in ("radius", "height", "width", "scale", "leader_length"):
            if key in op:
                op[key] = number(op[key])
                require(op[key] > 0, key + " 必須大於零")
        if kind in ("text", "mtext", "leader"):
            require(isinstance(op["text"], str) and 0 < len(op["text"]) <= 1000 and "\x00" not in op["text"], "文字需為 1–1000 字")
        if kind in ("line", "aligned_dimension", "diameter_dimension"):
            require(op["start"] != op["end"], "直線兩端不能相同")
        if kind == "leader":
            require(isinstance(op["points"], list) and len(op["points"]) == 3, "引線需三個點：箭頭、轉折、水平段末端")
            op["points"] = [point_value(p) for p in op["points"]]
            require(op["points"][0] != op["points"][1] and op["points"][1] != op["points"][2], "引線線段不能為零")
            require(op["points"][1][1] == op["points"][2][1], "引線末段需為水平線")
        if kind == "hatch":
            require(isinstance(op["boundary"], list) and 3 <= len(op["boundary"]) <= 100, "剖面線外框需 3–100 個頂點，不重複首點")
            op["boundary"] = [point_value(p) for p in op["boundary"]]
            require(len({tuple(p) for p in op["boundary"]}) == len(op["boundary"]), "剖面外框不可有重複頂點")
            require(isinstance(op["holes"], list) and len(op["holes"]) <= 20, "剖面最多 20 個圓孔")
            require(all(isinstance(h, dict) and set(h) == {"center", "radius"} for h in op["holes"]), "圓孔欄位不符")
            op["holes"] = [{"center": point_value(h["center"]), "radius": number(h["radius"])} for h in op["holes"]]
            require(all(h["radius"] > 0 for h in op["holes"]), "圓孔半徑需大於零")
            op["seed"] = point_value(op["seed"])
            budget += len(op["boundary"]) + len(op["holes"])
        if kind == "curve":
            count = op["segments"]
            require(type(count) is int and 2 <= count <= 1000, "公式曲線分段數需為 2–1000")
            low, high = number(op["t_start"]), number(op["t_end"])
            require(low < high, "公式參數終點需大於起點")
            fx, fy = Formula(op["x"]), Formula(op["y"])
            op["samples"] = [[fx.at(low + (high-low)*i/count), fy.at(low + (high-low)*i/count), 0.0] for i in range(count+1)]
            require(all(a != b for a, b in zip(op["samples"], op["samples"][1:])), "公式包含零長線段")
            budget += count
        budget += 2 if kind == "leader" else 1
        prepared.append(op)
    require(budget <= 4000, "總圖元數超過 4000 上限")
    return prepared


class Drawing:
    """所有 COM 寫入限於 Documents.Add 回傳的新圖面。"""
    def __init__(self, app, legacy):
        self.app, self.legacy = app, legacy
        self.doc = app.Documents.Add()
        self.model = self.doc.ModelSpace
        self.expected = []
        require(int(self.model.Count) == 0, "新圖面範本不為空，停止繪圖")
        for name, value in (("INSUNITS", 4), ("LUNITS", 2), ("DIMSCALE", 1.0), ("DIMTXT", 2.5), ("DIMASZ", 3.0), ("DIMTSZ", 0.0), ("DIMLFAC", 1.0)):
            self.doc.SetVariable(name, value)

    def point(self, point):
        if self.legacy:
            return self.app.Library.CreatePoint(*point)
        return VARIANT(pythoncom.VT_ARRAY | pythoncom.VT_R8, tuple(point))

    def points(self, points):
        if not self.legacy:
            return VARIANT(pythoncom.VT_ARRAY | pythoncom.VT_R8, tuple(v for p in points for v in p))
        result = self.app.Library.CreatePoints()
        for values in points:
            # IntelliCAD Points.Add 接受座標；不假設 Item 是零起始索引。
            # 亦供舊版剖面線種子點使用，不能只修正引線呼叫處。
            result.Add(*values)
        return result

    def remember(self, entity, color, **properties):
        entity.Color = color
        properties["Color"] = color
        self.expected.append({"handle": str(entity.Handle).upper(), "properties": properties})
        return entity

    def line(self, start, end, color):
        return self.remember(self.model.AddLine(self.point(start), self.point(end)), color, StartPoint=start, EndPoint=end)

    def circle(self, center, radius, color):
        return self.remember(self.model.AddCircle(self.point(center), radius), color, Center=center, Radius=radius)

    def draw(self, op):
        kind, color = op["kind"], op["color"]
        if kind == "line":
            self.line(op["start"], op["end"], color)
        elif kind == "circle":
            self.circle(op["center"], op["radius"], color)
        elif kind in ("text", "mtext"):
            if kind == "text":
                obj = self.model.AddText(op["text"], self.point(op["position"]), op["height"])
            else:
                obj = self.model.AddMText(self.point(op["position"]), op["width"], op["text"])
                obj.Height = op["height"]
            self.remember(obj, color, TextString=op["text"], Height=op["height"])
        elif kind == "leader":
            # 兩種介面帶箭頭直線都是 2；0 是無箭頭，不能混淆 ArrowheadType 的 0。
            leader = self.model.AddLeader(self.points(op["points"]), 0 if self.legacy else None, 2)
            properties = {"Type": 2}
            if not self.legacy:
                leader.ArrowheadSize = 3.0
                leader.ArrowheadType = 0
                properties.update(ArrowheadSize=3.0, ArrowheadType=0)
            self.remember(leader, color, **properties)
            # 文字放在水平線的左端上方，不建立虛假的關聯標註。
            position = [min(op["points"][1][0], op["points"][2][0])+2, op["points"][1][1]+3, 0.0]
            self.remember(self.model.AddText(op["text"], self.point(position), op["height"]), color,
                          TextString=op["text"], InsertionPoint=position, Height=op["height"])
        elif kind == "curve":
            for start, end in zip(op["samples"], op["samples"][1:]):
                self.line(start, end, color)
        elif kind in ("aligned_dimension", "diameter_dimension"):
            start, end = self.point(op["start"]), self.point(op["end"])
            if kind == "aligned_dimension":
                obj = self.model.AddDimAligned(start, end, self.point(op["location"]))
            else:
                obj = self.model.AddDimDiametric(start, end, op["leader_length"])
            self.remember(obj, color)
            self.expected[-1]["measurement"] = math.dist(op["start"], op["end"])
        elif kind == "hatch":
            outer = [self.line(a, b, color) for a, b in zip(op["boundary"], op["boundary"][1:] + op["boundary"][:1])]
            holes = [self.circle(h["center"], h["radius"], color) for h in op["holes"]]
            hatch = self.model.AddHatch(1, "ANSI31", False)
            hatch.PatternScale = op["scale"]
            def array(items):
                return VARIANT(pythoncom.VT_ARRAY | pythoncom.VT_DISPATCH, tuple(items))
            if self.legacy:
                selection = self.doc.SelectionSets.Add("lm_" + uuid.uuid4().hex[:12])
                try:
                    selection.AddItems(array(outer + holes))
                    hatch.ApplyHatch(selection, self.points([op["seed"]]))
                finally:
                    selection.Delete()
            else:
                hatch.AppendOuterLoop(array(outer))
                for hole in holes:
                    hatch.AppendInnerLoop(array([hole]))
                hatch.Evaluate()
            self.remember(hatch, color, PatternScale=op["scale"])

    def verify(self, doc):
        require(int(doc.GetVariable("INSUNITS")) == 4, "圖面單位不是毫米")
        entities = {str(entity.Handle).upper(): entity for entity in doc.ModelSpace}
        require(len(entities) == int(doc.ModelSpace.Count) == len(self.expected), "圖元數量不符")
        unavailable = []
        for row in self.expected:
            require(row["handle"] in entities, "存檔後圖元 Handle 遺失")
            entity = entities[row["handle"]]
            for key, value in row["properties"].items():
                require(equal(getattr(entity, key), value), "圖元屬性不符：" + key)
            if "measurement" in row:
                try:
                    actual = float(entity.Measurement)
                except pythoncom.com_error as exc:
                    # 舊版 ICAD 明確回 E_NOTIMPL；只把該值標為未驗證，不捏造通過。
                    if not (self.legacy and exc.excepinfo and exc.excepinfo[5] == -2147467263):
                        raise
                    unavailable.append(row["handle"])
                else:
                    require(equal(actual, row["measurement"]), "標註量測值不符")
        return {"entity_count": len(entities), "geometry_and_colors_verified": True,
                "dimension_measurement_unavailable": unavailable}


def execute(request, report):
    report.update(step="validate", drawing_started=False, completed_operations=0,
                  saved=False, file_exists=False)
    found, errors = registry.discover()
    if request["action"] == "inventory":
        return {"status": "INVENTORY", "versions": found, "errors": errors}
    operations = prepare(request["operations"])
    version = request["version"]
    candidates = [v for v in found if v["server_exists"] and (version == "latest" or v["progid"] == version)]
    require(candidates, "找不到所選 CAD 的 COM 登錄；請先安裝並開啟 CAD")
    selected = candidates[0]
    report.update(selected=selected, step="connect")
    clsid = pythoncom.MakeIID(selected["clsid"])
    try:
        raw = pythoncom.GetActiveObject(clsid).QueryInterface(pythoncom.IID_IDispatch)
    except pythoncom.com_error:
        raw = pythoncom.CoCreateInstance(clsid, None, pythoncom.CLSCTX_LOCAL_SERVER, pythoncom.IID_IDispatch)
    app = dynamic.Dispatch(raw)
    report["step"] = "identity"
    identity = {key: str(getattr(app, key)) for key in ("Name", "Version", "FullName")}
    require(os.path.normcase(os.path.abspath(identity["FullName"])) == os.path.normcase(os.path.abspath(selected["server"])), "連接的 CAD 路徑不符合所選版本")
    family = identify_product(selected, identity)
    require(family != "unknown", "此 CAD 身份尚未核對，不建立文件")
    report.update(application=identity, step="create_document")
    app.Visible = True
    # 呼叫 Documents.Add 前先標記；即使 COM 回覆遺失，也不能當作未建立文件。
    report["drawing_started"] = True
    drawing = Drawing(app, family == "yoda_icad_2008")
    report["owned_document_created"] = True
    for index, op in enumerate(operations):
        report.update(step=op["kind"], operation_index=index)
        drawing.draw(op)
        report["completed_operations"] = index + 1
    report["step"] = "verify_before_save"
    drawing.doc.Regen() if drawing.legacy else drawing.doc.Regen(1)
    before = drawing.verify(drawing.doc)
    output = Path(request["output"])
    require(output.is_absolute() and output.suffix.lower() == ".dwg" and not os.path.lexists(output), "輸出位置不合法或已存在")
    report["step"] = "save"
    drawing.doc.SaveAs(str(output), 9) if drawing.legacy else drawing.doc.SaveAs(str(output))
    report["saved"] = True
    require(output.is_file() and output.stat().st_size > 0, "未產生 DWG")
    report["step"] = "reopen"
    drawing.doc.Close(False)
    reopened = app.Documents.Open(str(output))
    require(os.path.normcase(os.path.abspath(str(reopened.FullName))) == os.path.normcase(str(output)), "重開圖面路徑不符")
    after = drawing.verify(reopened)
    # 只關閉已重開核對的自建文件，供 Rust 讀回檔案並核對交付雜湊。
    reopened.Close(False)
    return {"status": "DATA_VERIFIED_VISUAL_PENDING", "before_save": before, "after_reopen": after,
            "visual_check_required": True, "curve_representation": "sampled_line_segments_no_error_bound",
            "leader_text_associative": False, "units": "mm", "selected": selected,
            "application": identity}


def failure_details(report, exc):
    """只允許有明確證據的復原；逾時／未知 COM 結果仍交由人工確認。"""
    info = getattr(exc, "excepinfo", None)
    code = info[5] if info and len(info) > 5 else getattr(exc, "hresult", None)
    before_drawing = report.get("drawing_started") is False
    known_partial = (report.get("owned_document_created") is True
                     and code == -2147024809 and report.get("step") not in ("save", "reopen"))
    recovery = "correct_and_retry" if before_drawing else "revised_new_document" if known_partial else "manual_check"
    advice = {
        "correct_and_retry": "尚未建立圖面；補正參數或確認指定 CAD 已就緒後，可在本任務有限次重試。",
        "revised_new_document": "本次新圖面有未完成內容，未必已存檔。可提交修改後的計畫另建新圖面；不重播相同計畫、不修改或關閉部分圖面。",
        "manual_check": "CAD 結果未確認；停止後續繪圖，請人工檢查 CAD 與輸出目錄。",
    }[recovery]
    return dict(recovery=recovery, advice=advice)


def main():
    report = {"status": "FAILED", "existing_documents_targeted": False}
    pythoncom.CoInitialize()
    try:
        request = json.loads(sys.stdin.buffer.read(1_000_001).decode("utf-8"))
        report.update(execute(request, report))
    except Exception as exc:
        report.update(status="FAILED", error=str(exc), traceback=traceback.format_exc(),
                      **failure_details(report, exc))
    finally:
        # 預定輸出路徑不等於已存在檔案，更不等於已通過驗證的成果。
        if "request" in locals() and request.get("output"):
            report["file_exists"] = Path(request["output"]).is_file()
        pythoncom.CoUninitialize()
    sys.stdout.buffer.write(json.dumps(report, ensure_ascii=False).encode("utf-8"))


if __name__ == "__main__":
    main()
