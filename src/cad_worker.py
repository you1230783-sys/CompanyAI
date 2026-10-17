"""CAD 固定操作工作程序。只接受 JSON 幾何，不執行模型提供的 Python 或 CAD 指令。"""
import ast
import json
import math
import operator
import os
import re
from pathlib import Path
import sys
import time
import traceback
import uuid

import pythoncom
from win32com.client import VARIANT, dynamic
import registry
from cad import identify_product
from cad_existing import iter_entities

sys.dont_write_bytecode = True


def require(condition, message):
    if not condition:
        raise ValueError(message)


class CommandCapabilityError(ValueError):
    """唯讀前置檢查已確認缺介面；不代表指令曾經執行。"""


class CadInstallationError(RuntimeError):
    """COM 已回應但原廠型別庫缺漏；不能當作模型參數或不支援屬性重試。"""


def check_type_library(raw):
    """pywin32 可能將型別庫錯誤隱藏成 AttributeError，先保留真正成因。"""
    try:
        raw.GetTypeInfo()
    except pythoncom.com_error as exc:
        if exc.hresult in (-2147319779, -2147312566):
            raise CadInstallationError(
                f"CAD COM 型別庫未登錄或無法載入（HRESULT {exc.hresult}）；"
                "請修復該版本的原廠安裝／COM 登錄後重新開啟 CAD。尚未操作圖面。") from exc
        # 合法 IDispatch 可以不提供型別資訊，仍允許原本的動態名稱查詢。
    except AttributeError:
        pass


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


def cad_text(value):
    """只轉換數字前的直徑前綴；不改寫一般文字中的希臘字母／外語字元。"""
    return re.sub(r"[ØøΦφ⌀](?=\s*\d)", "%%c", value)


def text_equal(actual, expected):
    return cad_text(str(actual)).replace("%%C", "%%c") == cad_text(str(expected)).replace("%%C", "%%c")


def equal(actual, expected):
    if isinstance(expected, list):
        actual = coordinate(actual)
        return len(actual) == len(expected) and all(equal(a, b) for a, b in zip(actual, expected))
    if isinstance(expected, str):
        return str(actual) == expected
    return math.isclose(float(actual), float(expected), rel_tol=1e-7, abs_tol=1e-6)


def command_snapshot_equal(actual, expected):
    """縮放可能改變浮點數末位；結構、字串與可讀欄位仍須完全一致。"""
    if isinstance(expected, dict):
        return (isinstance(actual, dict) and actual.keys() == expected.keys()
                and all(command_snapshot_equal(actual[k], v) for k, v in expected.items()))
    if isinstance(expected, list):
        return (isinstance(actual, list) and len(actual) == len(expected)
                and all(command_snapshot_equal(a, b) for a, b in zip(actual, expected)))
    if type(expected) in (int, float) and type(actual) in (int, float):
        return math.isclose(actual, expected, rel_tol=1e-10, abs_tol=1e-8)
    return actual == expected


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
        if kind == "command":
            require(set(op) == {"kind", "command"}, "指令操作只接受 kind/command")
            op["command"] = parse_command(op["command"])
            prepared.append(op)
            continue
        require(not any(item["kind"] == "command" for item in prepared), "CAD 指令需放在本批幾何操作之後")
        require(isinstance(kind, str) and kind in fields, "未知繪圖操作")
        optional_fields = {"line_style"} if kind in ("line", "circle", "curve") else set()
        require(set(op) - optional_fields == {"kind", "color"} | set(fields[kind].split()), "繪圖操作欄位缺漏或包含未知欄位")
        if "line_style" in op:
            op["line_style"] = prepare_line_style(op["line_style"])
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
            op["text"] = cad_text(op["text"])
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
        budget += 3 if kind == "leader" else 1
        prepared.append(op)
    require(budget <= 4000, "總圖元數超過 4000 上限")
    return prepared


def prepare_line_style(value):
    """固定線型名稱及圖元比例；不接受模型提供 LIN 路徑或全圖變數。"""
    require(isinstance(value, dict) and set(value) == {"name", "scale"}, "線型需 name/scale")
    require(isinstance(value["name"], str), "線型名稱必須是字串")
    name = value["name"].upper()
    require(name in ("CONTINUOUS", "DASHED", "HIDDEN", "CENTER", "DASHDOT", "DOT", "BYLAYER", "BYBLOCK"), "不支援此線型名稱")
    scale = number(value["scale"])
    require(0.000001 <= scale <= 1000000, "線型比例需介於 0.000001 與 1000000")
    return {"name": name, "scale": scale}


def parse_command(value):
    """完整比對指令語法；不將模型字串交給 SendCommand／LISP／Shell。"""
    require(isinstance(value, str) and 0 < len(value) <= 80, "CAD 指令長度需為 1–80 字元")
    require(not any(c in value for c in "\r\n\x00;()\""), "CAD 指令不接受多行、串接或程式運算式")
    words = [word.lstrip("_.").upper() for word in value.strip().split()]
    aliases = {
        ("ZOOM", "E"): "ZOOM EXTENTS", ("ZOOM", "EXTENTS"): "ZOOM EXTENTS",
        ("ZOOM", "A"): "ZOOM ALL", ("ZOOM", "ALL"): "ZOOM ALL",
        ("REGEN",): "REGEN", ("PURGE",): "PURGE",
        ("-PURGE", "A", "*", "N"): "PURGE",
        ("AUDIT", "N"): "AUDIT N",
    }
    require(tuple(words) in aliases, "尚未支援此 CAD 指令；允許 ZOOM E/A、REGEN、PURGE、AUDIT N")
    return aliases[tuple(words)]


class Drawing:
    """所有 COM 寫入限於 Documents.Add 回傳的新圖面。"""
    def __init__(self, app, legacy):
        self.app, self.legacy = app, legacy
        self.doc = app.Documents.Add()
        self.model = self.doc.ModelSpace
        self.expected = []
        require(int(self.model.Count) == 0, "新圖面範本不為空，停止繪圖")
        for name, value in (("INSUNITS", 4), ("LUNITS", 2), ("DIMSCALE", 1.0), ("DIMTXT", 3.0), ("DIMASZ", 3.0), ("DIMTSZ", 0.0), ("DIMLFAC", 1.0)):
            self.doc.SetVariable(name, value)

    def annotation_style(self):
        """建立本輪專用樣式，不改使用者既有文字／尺寸樣式。

        simplex 不含完整中文字集，沿用目前文字樣式的 BigFont；沒有 BigFont
        時明示需人工核對中文，不以屬性設定成功宣稱字形顯示正常。
        """
        if hasattr(self, "annotation_style_name"):
            return self.annotation_style_name
        from cad_existing import optional
        name = "LM_AI_" + uuid.uuid4().hex[:12]
        style = self.doc.TextStyles.Add(name)
        style.FontFile = "simplex.shx"
        style.Height = 0.0
        current_style = self.doc.TextStyles.Item(str(self.doc.GetVariable("DIMTXSTY")))
        bigfont = optional(current_style, "BigFontFile") or optional(self.doc.ActiveTextStyle, "BigFontFile")
        if not bigfont:
            # 只採用已安裝的繁體中文字型；不下載字型、不猜測不存在的檔案。
            install = optional(self.app, "FullName")
            if install:
                directory = Path(str(install)).parent
                for candidate in (directory / "Fonts" / "chineset.shx", directory / "chineset.shx"):
                    if candidate.is_file():
                        bigfont = str(candidate)
                        break
        if bigfont:
            style.BigFontFile = bigfont
        self.annotation_style_name = name
        if hasattr(self, "report"):
            self.report["annotation_style"] = dict(name=name, font="simplex.shx",
                big_font=bigfont or None, text_height=3.0, arrow_size=3.0,
                text_gap=1.0, alignment="ISO", visual_check_required=True,
                chinese_note="使用原樣式或已安裝的繁體中文 BigFont；中文仍需目視核對" if bigfont
                else "原樣式無 BigFont；simplex.shx 不保證中文字形，請在 CAD 指定中文字型或 BigFont")
        return name

    def dimension_style(self):
        """尺寸與引線才需要尺寸樣式；一般文字不依賴尺寸集合介面。

        佑達 iCAD 使用 IntelliCAD 原生 DimensionStyles／Dim* 屬性。
        GstarCAD 使用 AutoCAD 相容的 DimStyles／CopyFrom，不混用兩套介面。
        """
        if hasattr(self, "dimension_style_name"):
            return self.dimension_style_name
        name = self.annotation_style()
        values = {"DimTxt": 3.0, "DimAsz": 3.0, "DimGap": 1.0,
                  "DimTih": 0, "DimToh": 1, "DimTad": 1, "DimTsz": 0.0,
                  "DimScale": 1.0, "DimTxSty": name}
        if self.legacy:
            # 不先嘗試已知不支援的別名，也不呼叫原生樣式沒有的 CopyFrom。
            style = self.doc.DimensionStyles.Add(name)
            for key, value in values.items():
                setattr(style, key, value)
                require(equal(getattr(style, key), value), "舊版尺寸樣式設定讀回不符：" + key)
            self.dimension_style_expected = values
            mode = "DimensionStyles.Add/native_properties"
        else:
            # 將暫時的尺寸變數固化在新樣式；無論成功與否都還原文件預設值。
            previous = {key.upper(): self.doc.GetVariable(key.upper()) for key in values}
            try:
                for key, value in values.items():
                    self.doc.SetVariable(key.upper(), value)
                style = self.doc.DimStyles.Add(name)
                style.CopyFrom(self.doc)
            finally:
                for key, value in previous.items():
                    self.doc.SetVariable(key, value)
            mode = "DimStyles.Add/CopyFrom"
        self.dimension_style_name = name
        if hasattr(self, "report"):
            self.report.setdefault("annotation_style", {})["dimension_style_api"] = mode
        return name

    def prepare_linetypes(self, styles):
        """先載入缺少的標準線型；只查 CAD 安裝目錄，不變更既有定義或 LTSCALE。

        Load 失敗即停止，不重播可能已改變文件的 COM 呼叫。使用者自行載入的
        同名定義優先沿用；線型外觀仍受圖面比例與定義影響，需實機目視確認。
        """
        if not styles:
            return
        names = {prepare_line_style(style)["name"] for style in styles}
        table = self.doc.Linetypes
        loaded = {}
        for index, item in enumerate(iter_entities(table)):
            require(index < 10000, "線型定義超過讀取上限")
            loaded[str(item.Name).upper()] = str(item.Name)
        root = Path(str(self.app.FullName)).parent.resolve()
        for name in sorted(names):
            if name in loaded:
                continue
            # ByLayer／ByBlock 屬於原生特殊值，不從外部 LIN 載入。
            if name in ("BYLAYER", "BYBLOCK"):
                loaded[name] = "ByLayer" if name == "BYLAYER" else "ByBlock"
                continue
            source = None
            # GstarCAD 2022 安裝包將預設支援檔放在 UserDataCache/Support。
            # 只擴充已知的安裝目錄，不搜尋整顆磁碟或使用模型提供的路徑。
            for folder in (root, root / "Support", root / "UserDataCache" / "Support"):
                for filename in ("icadiso.lin", "gcadiso.lin", "acadiso.lin", "icad.lin", "gcad.lin", "acad.lin"):
                    candidate = (folder / filename).resolve()
                    if not candidate.is_relative_to(root) or not candidate.is_file():
                        continue
                    require(candidate.stat().st_size <= 4 * 1024 * 1024, "標準線型檔超過 4 MB")
                    # 標準名稱為 ASCII；只尋找定義標頭，不解譯任何 LIN 程式內容。
                    content = candidate.read_bytes().decode("latin-1")
                    if re.search(r"(?im)^\s*\*" + re.escape(name) + r"\s*,", content):
                        source = candidate
                        break
                if source is not None:
                    break
            require(source is not None, "找不到標準線型 " + name + "；請先在 CAD 載入此線型後再執行")
            self.report.update(step="prepare_linetypes", drawing_started=True)
            table.Load(name, str(source))
            loaded[name] = str(table.Item(name).Name)
            require(loaded[name].upper() == name, "載入後線型名稱不符")
            self.report.setdefault("linetypes_loaded", []).append({"name": name, "source": str(source)})
        self.linetype_names = loaded

    def apply_line_style(self, entity, style):
        """寫入並讀回原生圖元線型，回傳供存檔前後驗證的屬性。"""
        style = prepare_line_style(style)
        name = self.linetype_names[style["name"]]
        entity.Linetype = name
        entity.LinetypeScale = style["scale"]
        require(str(entity.Linetype).upper() == name.upper(), "圖元線型讀回不符")
        require(equal(entity.LinetypeScale, style["scale"]), "圖元線型比例讀回不符")
        return {"Linetype": str(entity.Linetype), "LinetypeScale": style["scale"]}

    def prepare_styles(self, operations):
        """先準備整批所需樣式，避免完成部分幾何才發現樣式介面缺失。"""
        kinds = {op["kind"] for op in operations}
        if kinds & {"aligned_dimension", "diameter_dimension", "leader"}:
            self.dimension_style()
        elif kinds & {"text", "mtext"}:
            self.annotation_style()

    def verify_styles(self, doc):
        """舊版獨立尺寸樣式在儲存前與重開後都讀回，不把設定成功當作持久化。"""
        values = getattr(self, "dimension_style_expected", {})
        if values:
            style = doc.DimensionStyles.Item(self.dimension_style_name)
            for key, value in values.items():
                require(equal(getattr(style, key), value), "尺寸樣式核對失敗：" + key)

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
        self.expected.append({"handle": str(entity.Handle).upper(), "properties": properties,
                              "operation_index": getattr(self, "report", {}).get("operation_index")})
        return entity

    def line(self, start, end, color):
        return self.remember(self.model.AddLine(self.point(start), self.point(end)), color, StartPoint=start, EndPoint=end)

    def circle(self, center, radius, color):
        return self.remember(self.model.AddCircle(self.point(center), radius), color, Center=center, Radius=radius)

    def draw(self, op):
        """記錄每項新增 Handle，讓未知圖元能追溯到產生的操作。"""
        if BULK_ENABLED:
            from cad_bulk import snapshot as native_snapshot
            before = set(native_snapshot(self.doc, sys.modules[__name__], handles_only=True)[0])
        else:
            before = {str(entity.Handle).upper() for entity in iter_entities(self.model)}
        expected_start = len(self.expected)
        try:
            self.draw_entity(op)
            if "line_style" in op:
                # 曲線目前是取樣線段，逐段套用同一原生線型；不改動其他操作。
                for row in self.expected[expected_start:]:
                    entity = self.doc.HandleToObject(row["handle"])
                    row["properties"].update(self.apply_line_style(entity, op["line_style"]))
        finally:
            if hasattr(self, "report"):
                from cad_existing import describe
                audit = {"operation_index": self.report.get("operation_index"), "kind": op["kind"],
                         "expected_entities": self.expected[expected_start:]}
                self.report.setdefault("operation_entities", []).append(audit)
                try:
                    if BULK_ENABLED:
                        after_entities, added = native_snapshot(self.doc, sys.modules[__name__], exclude_handles=before)
                        after_handles = set(after_entities)
                    else:
                        after = list(iter_entities(self.model))
                        added = [describe(entity, sys.modules[__name__]) for entity in after
                                 if str(entity.Handle).upper() not in before]
                        after_handles = {str(e.Handle).upper() for e in after}
                    audit.update(entities=added,
                                 removed_handles=sorted(before - after_handles))
                except Exception as diagnostic_error:
                    # 診斷讀取不能蓋過原始繪圖錯誤；最終 verify 仍嚴格核對集合。
                    self.report.setdefault("audit_errors", []).append(str(diagnostic_error))
                    audit["enumeration_error"] = str(diagnostic_error)

    def draw_entity(self, op):
        if op["kind"] == "command":
            self.run_command(op["command"])
            return
        kind, color = op["kind"], op["color"]
        if kind == "line":
            self.line(op["start"], op["end"], color)
        elif kind == "circle":
            self.circle(op["center"], op["radius"], color)
        elif kind in ("text", "mtext"):
            style_name = self.annotation_style()
            if kind == "text":
                obj = self.model.AddText(op["text"], self.point(op["position"]), op["height"])
            else:
                obj = self.model.AddMText(self.point(op["position"]), op["width"], op["text"])
                obj.Height = op["height"]
            obj.StyleName = style_name
            self.remember(obj, color, TextString=op["text"], Height=op["height"], StyleName=style_name)
        elif kind == "leader":
            style_name = self.annotation_style()
            self.native_leader(op)
            if self.legacy:
                return  # 原生 AddLeader 已建立並設定註解，不重複插入文字。
            # 固定字高 3、距水平尺寸線 1；字串的直徑符號已由 prepare 正規化。
            position = [min(op["points"][1][0], op["points"][2][0])+1, op["points"][1][1]+1, 0.0]
            text = self.model.AddText(op["text"], self.point(position), 3.0)
            text.StyleName = style_name
            text.Alignment = 0  # 左對齊基線；不繼承使用者先前的中間對齊。
            text.Rotation = 0.0
            text.InsertionPoint = self.point(position)
            self.remember(text, color, TextString=op["text"], InsertionPoint=position,
                          Height=3.0, StyleName=style_name, Alignment=0, Rotation=0.0)
        elif kind == "curve":
            for start, end in zip(op["samples"], op["samples"][1:]):
                self.line(start, end, color)
        elif kind in ("aligned_dimension", "diameter_dimension"):
            style_name = self.dimension_style()
            start, end = self.point(op["start"]), self.point(op["end"])
            if kind == "aligned_dimension":
                obj = self.model.AddDimAligned(start, end, self.point(op["location"]))
            else:
                obj = self.model.AddDimDiametric(start, end, op["leader_length"])
            obj.StyleName = style_name
            self.remember(obj, color, StyleName=style_name)
            self.expected[-1]["measurement"] = math.dist(op["start"], op["end"])
        elif kind == "hatch":
            self.draw_hatch(op)

    def command_method(self, command):
        """每個允許指令對應固定同步 COM 方法，版本不支援時不偷換成字串注入。"""
        if command == "ZOOM EXTENTS":
            return self.doc.ActiveViewport.ZoomExtents if self.legacy else self.app.ZoomExtents
        if command == "ZOOM ALL":
            return self.doc.ActiveViewport.ZoomAll if self.legacy else self.app.ZoomAll
        if command == "REGEN":
            return self.doc.Regen
        if command == "PURGE":
            return self.doc.PurgeAll
        if command == "AUDIT N":
            return self.doc.AuditInfo
        raise ValueError("未知 CAD 指令")

    def preflight_commands(self, operations):
        """只查詢方法，不執行；無文件時將文件方法留待新空圖建立後檢查。"""
        commands = [op["command"] for op in operations if op["kind"] == "command"]
        if not commands:
            return
        self.report["step"] = "validate_commands"
        checks = self.report.setdefault("command_capabilities", [])
        missing = []
        for command in commands:
            if self.doc is None and (self.legacy or not command.startswith("ZOOM ")):
                checks.append({"command":command, "status":"DEFERRED_NO_DOCUMENT", "executed":False})
                continue
            try:
                method = self.command_method(command)
                require(callable(method), "COM 成員不是可呼叫的方法")
            except (AttributeError, pythoncom.com_error) as exc:
                if not isinstance(exc, AttributeError) and getattr(exc, "hresult", None) not in (-2147352573, -2147352570, -2147467263):
                    raise  # RPC／逾時等未知結果不是「不支援」，不得誤放行重試。
                missing.append(command)
                checks.append({"command":command, "status":"UNSUPPORTED", "executed":False, "error":str(exc)})
            else:
                checks.append({"command":command, "status":"AVAILABLE", "executed":False,
                               "zoom_owner":"Document.ActiveViewport" if self.legacy else "Application"})
        if missing:
            self.report["unsupported_commands"] = missing
            raise CommandCapabilityError("CAD 缺少指令介面，尚未執行指令；請移除或更換：" + ", ".join(missing))

    def run_command(self, command):
        """固定文件內執行維護指令；核對可讀圖元，不把 AUDIT 呼叫返回當作無錯誤。"""
        from cad_existing import describe
        method = self.command_method(command)
        require(int(self.doc.GetVariable("CMDACTIVE")) == 0, "CAD 正在執行其他指令，請先完成或取消")

        def entities():
            result = {}
            size = 0
            for name in ("ModelSpace", "PaperSpace"):
                rows = []
                if BULK_ENABLED and name == "ModelSpace":
                    from cad_existing import snapshot
                    _, rows = snapshot(self.doc, sys.modules[__name__])
                    # 縮放核對沿用原有可讀屬性範圍；native_revision 用於修改前版本核對。
                    rows = [{k:v for k,v in row.items() if k != "native_revision"} for row in rows]
                    result[name] = sorted(rows, key=lambda row:row["handle"])
                    continue
                for index, entity in enumerate(iter_entities(getattr(self.doc, name))):
                    require(index < 200000, "指令核對圖元超過上限")
                    row = describe(entity, sys.modules[__name__])
                    size += len(json.dumps(row, ensure_ascii=False).encode("utf-8"))
                    require(size <= 64 * 1024 * 1024, "指令核對內容超過 64 MB")
                    rows.append(row)
                result[name] = sorted(rows, key=lambda row:row["handle"])
            return result

        def definitions():
            result = {}
            for name in ("Blocks", "Layers", "Linetypes", "TextStyles",
                         "DimensionStyles" if self.legacy else "DimStyles"):
                rows = []
                for index, item in enumerate(iter_entities(getattr(self.doc, name))):
                    require(index < 10000, "未使用定義清理核對超過上限")
                    rows.append(str(item.Name))
                result[name] = sorted(rows)
            return result

        before = entities()
        tables = definitions() if command == "PURGE" else {}
        entry = {"command":command, "status":"STARTED", "execution":"synchronous_com",
                 "definitions_before":tables}
        self.report.setdefault("commands", []).append(entry)
        self.doc.Activate()
        if command == "AUDIT N":
            method(False)
        elif command == "REGEN" and not self.legacy:
            method(1)
        else:
            method()
        require(int(self.doc.GetVariable("CMDACTIVE")) == 0, "指令結束狀態未確認")
        after_entities = entities()
        # iCAD 2008 原生尺寸在第一次重繪後才提供計算好的 TextString。
        # 僅容許從「不可讀」變成可讀；既有文字改變、欄位消失及圖元增減仍拒絕。
        late_text = []
        if self.legacy:
            for space, rows in before.items():
                old = {row["handle"]: row for row in rows}
                for row in after_entities[space]:
                    prior = old.get(row["handle"])
                    if (prior and prior["type"] in ("DimAligned", "DimDiametric")
                            and "TextString" not in prior["properties"]
                            and "TextString" in row["properties"]):
                        late_text.append({"space":space, "handle":row["handle"],
                                          "TextString":row["properties"].pop("TextString")})
        entry["dimension_text_first_readable"] = late_text
        if not command_snapshot_equal(after_entities, before):
            # 保留差異，避免只能請使用者猜測是否少了圖元。
            entry["entity_differences"] = {
                space: {"before": before[space], "after": after_entities[space]}
                for space in before if not command_snapshot_equal(after_entities[space], before[space])}
            raise ValueError("維護指令後圖元可讀內容不符；停止後續操作")
        if tables:
            after = definitions()
            entry["removed_definitions"] = {key:sorted(set(values)-set(after[key])) for key,values in tables.items()}
        entry.update(status="COMPLETED", readable_entities_unchanged=True,
                     audit_clean_not_verified=command == "AUDIT N", visual_check_required=True)

    def native_leader(self, op):
        # 帶箭頭直線是 2；0 是無箭頭，不能混淆 ArrowheadType 的 0。
        before = {str(entity.Handle).upper() for entity in iter_entities(self.model)}
        # IntelliCAD 原生第二參數是文字；AutoCAD 相容介面才是註解物件。
        leader = self.model.AddLeader(self.points(op["points"]), op["text"] if self.legacy else None, 2)
        leader.StyleName = self.dimension_style()
        properties = dict(Type=2, StyleName=leader.StyleName)
        if not self.legacy:
            leader.ArrowheadSize = 3.0
            leader.ArrowheadType = 0
            properties.update(ArrowheadSize=3.0, ArrowheadType=0)
        self.remember(leader, op["color"], **properties)
        if self.legacy:
            from cad_existing import describe
            created = [e for e in iter_entities(self.model) if str(e.Handle).upper() not in before
                       and str(e.Handle).upper() != str(leader.Handle).upper()]
            # 僅接受本次同步呼叫新建、內容和端點皆吻合的唯一註解。
            # 不靠總數吞掉未知物件，也不刪除使用者的文字。
            require(len(created) == 1, "原生引線註解數量不符")
            annotation = created[0]
            row = describe(annotation, sys.modules[__name__])
            require(row["kind"] in ("text", "mtext")
                    and text_equal(row["properties"].get("TextString", ""), op["text"])
                    and equal(row["properties"].get("InsertionPoint"), op["points"][-1]),
                    "原生引線新建註解內容或位置不符")
            position = [min(op["points"][1][0], op["points"][2][0])+1, op["points"][1][1]+1, 0.0]
            annotation.StyleName = self.annotation_style()
            annotation.Height = 3.0
            layout = {"Rotation": 0.0}
            annotation.Rotation = 0.0
            if row["kind"] == "mtext":
                # MText 預設可能以上緣或中心為插入點：只加 y=1 會向下穿過引線。
                # 先固定左下錨點，再設插入點，讓整個文字框從水平線上方1mm向上延伸。
                annotation.AttachmentPoint = 7
                layout["AttachmentPoint"] = 7
            else:
                annotation.Alignment = 0
                layout["Alignment"] = 0
            annotation.InsertionPoint = self.point(position)
            self.remember(annotation, op["color"], TextString=op["text"], Height=3.0,
                          StyleName=annotation.StyleName, InsertionPoint=position, **layout)
            return
        # 某些版本會自動建立空白註解。只有 COM 明示關聯、此次新建且位於
        # ModelSpace 的文字才納入預期集合；絕不按數量猜測／忽略未知物件。
        from cad_existing import optional, describe
        annotation = optional(leader, "Annotation")
        handle = optional(annotation, "Handle") if annotation is not None else None
        if handle:
            handle = str(handle).upper()
            members = {str(entity.Handle).upper() for entity in iter_entities(self.model)}
            row = describe(annotation, sys.modules[__name__])
            if hasattr(self, "report"):
                self.report.setdefault("leader_annotations", []).append({
                    "leader_handle": str(leader.Handle).upper(), "annotation": row,
                    "created_in_this_operation": handle not in before})
            if (handle not in before and handle in members and row["kind"] in ("text", "mtext")
                    and row["properties"].get("TextString") in ("", op["text"])
                    and not row.get("unavailable_properties")):
                self.expected.append({"handle": handle, "properties": row["properties"],
                                      "leader_handle": str(leader.Handle).upper()})

    def draw_hatch(self, op):
        color = op["color"]
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
                # IntelliCAD Delete 會刪除集合內圖元；先清空成員才可刪除容器。
                # Clear 失敗時不執行 Delete，保留圖面供診斷。
                selection.Clear()
                require(int(selection.Count) == 0, "剖面暫存選取集未清空；保留集合與邊界")
                selection.Delete()
        else:
            hatch.AppendOuterLoop(array(outer))
            for hole in holes:
                hatch.AppendInnerLoop(array([hole]))
            hatch.Evaluate()
        self.remember(hatch, color, PatternScale=op["scale"])

    def verify(self, doc):
        self.verify_styles(doc)
        require(int(doc.GetVariable("INSUNITS")) == 4, "圖面單位不是毫米")
        described = {}
        if BULK_ENABLED:
            from cad_existing import snapshot
            entities, rows = snapshot(doc, sys.modules[__name__])
            described = {row["handle"]: row["properties"] for row in rows}
        else:
            rows = list(iter_entities(doc.ModelSpace))
            entities = {str(entity.Handle).upper(): entity for entity in rows}
        expected = {row["handle"] for row in self.expected}
        missing, extra = sorted(expected - entities.keys()), sorted(entities.keys() - expected)
        self.last_audit = dict(expected_count=len(self.expected), reported_count=int(doc.ModelSpace.Count),
                               enumerated_count=len(rows), unique_handles=len(entities),
                               missing_handles=missing[:100], unexpected_handles=extra[:100])
        self.last_audit["missing_expected_entities"] = [row for row in self.expected if row["handle"] in missing][:100]
        if extra:
            from cad_existing import describe
            self.last_audit["unexpected_entities"] = [describe(entities[h], sys.modules[__name__]) for h in extra[:100]]
        if hasattr(self, "report"):
            self.report["entity_audit"] = self.last_audit
        # Count 在舊版可能含內部項目；只有唯一 Handle 集合完全符合才接受計數差異。
        require(not missing and not extra and len(rows) == len(entities) == len(self.expected),
                "圖元集合不符：" + json.dumps(self.last_audit, ensure_ascii=False))
        unavailable = []
        for row in self.expected:
            require(row["handle"] in entities, "存檔後圖元 Handle 遺失")
            if "leader_handle" in row:
                linked = entities[row["leader_handle"]].Annotation
                require(str(linked.Handle).upper() == row["handle"], "引線附屬文字關聯不符")
            for key, value in row["properties"].items():
                compare = text_equal if key == "TextString" else equal
                available = described.get(row["handle"], {})
                observed = available[key] if key in available else getattr(entities[row["handle"]], key)
                require(compare(observed, value), "圖元屬性不符：" + key)
            if "measurement" in row:
                try:
                    actual = float(entities[row["handle"]].Measurement)
                except pythoncom.com_error as exc:
                    # 舊版 ICAD 明確回 E_NOTIMPL；只把該值標為未驗證，不捏造通過。
                    if not (self.legacy and exc.excepinfo and exc.excepinfo[5] == -2147467263):
                        raise
                    unavailable.append(row["handle"])
                else:
                    require(equal(actual, row["measurement"]), "標註量測值不符")
        return {"entity_count": len(entities), "geometry_and_colors_verified": True,
                "entity_audit": self.last_audit,
                "dimension_measurement_unavailable": unavailable}


def connect_application(selected, request, report):
    """在同一工作程序保留 COM 物件至操作結束；就緒等待只重試連線／身分讀取。

    不重試 CoCreateInstance，不把啟動失敗當作可任意再開一個 CAD；未知錯誤
    直接回報。只有使用者點選開啟時才允許啟動，手動重連只取得既有物件。
    """
    clsid = pythoncom.MakeIID(selected["clsid"])
    allow_launch = request.get("launch_if_missing") is True or request["action"] == "launch"
    deadline = time.monotonic() + (15.0 if request.get("wait_for_ready") is True else 0.0)
    launch_attempted = False
    app = None
    report["connection"] = {"same_worker": True, "attempts": 0, "launch_attempted": False}
    while True:
        report["connection"]["attempts"] += 1
        try:
            if app is None:
                try:
                    raw = pythoncom.GetActiveObject(clsid).QueryInterface(pythoncom.IID_IDispatch)
                except pythoncom.com_error as exc:
                    # MK_E_UNAVAILABLE 才代表目前 ROT 沒有此物件；其他錯誤不啟動新程序。
                    if not (allow_launch and not launch_attempted and exc.hresult == -2147221021):
                        raise
                    launch_attempted = True
                    report["connection"]["launch_attempted"] = True
                    raw = pythoncom.CoCreateInstance(clsid, None, pythoncom.CLSCTX_LOCAL_SERVER,
                                                     pythoncom.IID_IDispatch)
                check_type_library(raw)
                app = dynamic.Dispatch(raw)
            identity = {key: str(getattr(app, key)) for key in ("Name", "Version", "FullName")}
            if launch_attempted:
                require(os.path.normcase(os.path.abspath(identity["FullName"]))
                        == os.path.normcase(os.path.abspath(selected["server"])),
                        "連接的 CAD 路徑不符合所選版本")
                app.Visible = True
            return app, identity
        except pythoncom.com_error as exc:
            report["connection"]["last_hresult"] = exc.hresult
            # 已取得 app 時只重讀其身分，不重新 CoCreate 或改接另一個物件。
            transient = exc.hresult in (-2147221021, -2147418111, -2147417846)
            if not transient or time.monotonic() >= deadline or (launch_attempted and app is None):
                raise
            pythoncom.PumpWaitingMessages()
            time.sleep(min(0.5, max(0.0, deadline - time.monotonic())))


def execute(request, report):
    global BULK_APP, BULK_LEGACY
    report.update(step="validate", drawing_started=False, completed_operations=0,
                  saved=False, file_exists=False)
    found, errors = registry.discover()
    if request["action"] == "inventory":
        return {"status": "INVENTORY", "versions": found, "errors": errors}
    operations = prepare(request["operations"]) if request["action"] == "draw" else []
    version = request["version"]
    require(isinstance(version, str), "CAD 版本必須是 inventory 回傳的 ProgID")
    # Windows ProgID 本身不區分大小寫；仍以已登錄 CLSID／執行檔核對實際連線。
    candidates = [v for v in found if v["server_exists"] and (version.casefold() == "latest" or v["progid"].casefold() == version.casefold())]
    require(candidates, "找不到所選 CAD 的 COM 登錄；請先安裝並開啟 CAD")
    selected = candidates[0]
    report.update(selected=selected, step="connect")
    app, identity = connect_application(selected, request, report)
    report["step"] = "identity"
    require(os.path.normcase(os.path.abspath(identity["FullName"])) == os.path.normcase(os.path.abspath(selected["server"])), "連接的 CAD 路徑不符合所選版本")
    family = identify_product(selected, identity)
    require(family != "unknown", "此 CAD 身份尚未核對，不建立文件")
    BULK_APP, BULK_LEGACY = app, family == "yoda_icad_2008"
    if request["action"] in ("connect", "launch"):
        if request["action"] == "launch":
            app.Visible = True
        return {"status": "CONNECTED", "selected": selected, "application": identity}
    if request["action"] in ("active_read", "active_snapshot", "active_probe", "active_edit"):
        from cad_active import execute_active
        report["application"] = identity
        return execute_active(app, family == "yoda_icad_2008", request, report, sys.modules[__name__])
    if request["action"] in ("read", "edit"):
        from cad_existing import execute_existing
        report["application"] = identity
        return execute_existing(app, family == "yoda_icad_2008", request, report, sys.modules[__name__])
    require(request["action"] == "draw", "未知 CAD 操作")
    report.update(application=identity, step="validate_commands")
    # 有現成文件時只用它查詢介面，不縮放、不修改；避免已知不支援仍先建空圖。
    if any(op["kind"] == "command" for op in operations):
        probe = Drawing.__new__(Drawing)
        probe.app, probe.legacy, probe.report = app, family == "yoda_icad_2008", report
        probe.doc = app.ActiveDocument if int(app.Documents.Count) > 0 else None
        probe.preflight_commands(operations)
    report["step"] = "create_document"
    app.Visible = True
    # 呼叫 Documents.Add 前先標記；即使 COM 回覆遺失，也不能當作未建立文件。
    report["drawing_started"] = True
    drawing = Drawing(app, family == "yoda_icad_2008")
    drawing.report = report
    report["owned_document_created"] = True
    drawing.preflight_commands(operations)
    report["step"] = "prepare_annotation_styles"
    drawing.prepare_linetypes([op["line_style"] for op in operations if "line_style" in op])
    drawing.prepare_styles(operations)
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
    if isinstance(exc, CadInstallationError):
        return dict(recovery="repair_installation", connection_lost=False,
                    advice="原廠 COM 型別庫有安裝問題；不重試繪圖，也不由模型自動修改 Windows 登錄。")
    if isinstance(exc, CommandCapabilityError) and report.get("completed_operations", 0) == 0:
        owned = report.get("owned_document_created") is True
        return dict(recovery="revised_new_document" if owned else "correct_and_retry",
                    connection_lost=False,
                    advice=("指令尚未執行，尚無幾何操作。請移除不支援指令後提交修正計畫；"
                            + ("本次空圖／副本保留在 CAD，不接續修改。" if owned else "未建立新圖或修改目前文件。")))
    info = getattr(exc, "excepinfo", None)
    code = info[5] if info and len(info) > 5 else getattr(exc, "hresult", None)
    before_drawing = report.get("drawing_started") is False or report.get("step") == "validate_edit"
    known_partial = (report.get("owned_document_created") is True
                     and code == -2147024809 and report.get("step") not in ("save", "reopen"))
    recovery = "correct_and_retry" if before_drawing else "revised_new_document" if known_partial else "manual_check"
    if before_drawing and (isinstance(exc, AttributeError) or code in (-2147467263, -2147352573)):
        return dict(recovery="unsupported_capability", connection_lost=False,
                    advice="此 CAD 缺少所需介面；相同操作重試不會改善，請保留診斷並改用副本或更新程式。")
    advice = {
        "correct_and_retry": "尚未建立圖面；補正參數或確認指定 CAD 已就緒後，可在本任務有限次重試。",
        "revised_new_document": "本次新圖面有未完成內容，未必已存檔。可提交修改後的計畫另建新圖面；不重播相同計畫、不修改或關閉部分圖面。",
        "manual_check": "CAD 結果未確認；停止後續繪圖，請人工檢查 CAD 與輸出目錄。",
    }[recovery]
    connection_lost = code in (-2147417848, -2147023174, -2147023170, -2147220995)
    return dict(recovery=recovery, advice=advice, connection_lost=connection_lost)


READ_PROGRESS_ENABLED = False
BULK_ENABLED = False
BULK_APP = None
BULK_LEGACY = False


def read_progress(completed, reported_count, phase):
    """固定數字進度走 stderr；stdout 保持單一結果 JSON，沒有圖元正文。"""
    if READ_PROGRESS_ENABLED:
        event = dict(completed=completed, reported_count=reported_count, phase=phase)
        sys.stderr.buffer.write(json.dumps(event, ensure_ascii=True).encode("ascii") + b"\n")
        sys.stderr.buffer.flush()


def main():
    global READ_PROGRESS_ENABLED, BULK_ENABLED
    report = {"status": "FAILED", "existing_documents_targeted": False}
    pythoncom.CoInitialize()
    try:
        request = json.loads(sys.stdin.buffer.read(1_000_001).decode("utf-8"))
        READ_PROGRESS_ENABLED = request.get("read_progress") is True
        BULK_ENABLED = True
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
