"""只操作本工具新建的圖面；COM 呼叫失敗不自動重播，避免新增重複圖元。"""
import math
import os
import re
import pythoncom
from win32com.client import VARIANT, dynamic


def point(x, y, z=0):
    return VARIANT(pythoncom.VT_ARRAY | pythoncom.VT_R8, (float(x), float(y), float(z)))


def close_enough(actual, expected):
    if isinstance(expected, tuple):
        return len(actual) == len(expected) and all(close_enough(a, b) for a, b in zip(actual, expected))
    return math.isclose(float(actual), expected, rel_tol=1e-7, abs_tol=1e-6)


def geometry(model):
    """從 CAD 讀回圖元幾何，不以呼叫未丟例外當作尺寸驗證通過。"""
    rows = []
    for i in range(int(model.Count)):
        entity = model.Item(i)
        kind = str(entity.ObjectName)
        if kind.lower().endswith("line"):
            rows.append(dict(kind="line", start=list(entity.StartPoint), end=list(entity.EndPoint)))
        elif kind.lower().endswith("circle"):
            rows.append(dict(kind="circle", center=list(entity.Center), radius=float(entity.Radius)))
        elif "dimension" in kind.lower():
            rows.append(dict(kind="dimension", measurement=float(entity.Measurement)))
        else:
            rows.append(dict(kind=kind))
    return rows


def verify_geometry(rows, dimensions):
    corners = [(0., 0., 0.), (200., 0., 0.), (200., 100., 0.), (0., 100., 0.)]
    expected = [(corners[i], corners[(i + 1) % 4]) for i in range(4)]
    lines = [r for r in rows if r["kind"] == "line"]
    circles = [r for r in rows if r["kind"] == "circle"]
    dims = [r["measurement"] for r in rows if r["kind"] == "dimension"]
    assert len(lines) == 4 and len(circles) == 4, "直線／圓數量不符"
    for start, end in expected:
        assert any(close_enough(r["start"], start) and close_enough(r["end"], end) for r in lines), "矩形端點不符"
    for x, y in [(15, 15), (185, 15), (185, 85), (15, 85)]:
        assert any(close_enough(r["center"], (x, y, 0)) and close_enough(r["radius"], 5) for r in circles), "孔位或半徑不符"
    expected_dims = [10., 100., 200.] if dimensions else []
    assert len(rows) == 8 + len(expected_dims), "存在非預期圖元"
    assert len(dims) == len(expected_dims) and all(close_enough(a, b) for a, b in zip(sorted(dims), expected_dims)), "標註量測值不符"


def run(request, update):
    """連接指定 CLSID，不用通用 ProgID 偷換版本；不接管目前作用中的圖面。"""
    pythoncom.CoInitialize()
    try:
        selected = request["selected"]
        update("connect", status="RUNNING")
        clsid = pythoncom.MakeIID(selected["clsid"])
        try:
            raw = pythoncom.GetActiveObject(clsid).QueryInterface(pythoncom.IID_IDispatch)
        except pythoncom.com_error:
            if request["mode"] == "diagnose" or not request.get("launch"):
                raise RuntimeError("所選版本沒有可連接的執行中COM物件。請先開啟該版本、完成啟動畫面，再試一次；不會自動改用舊版。")
            raw = pythoncom.CoCreateInstance(clsid, None, pythoncom.CLSCTX_LOCAL_SERVER, pythoncom.IID_IDispatch)
        app = dynamic.Dispatch(raw)
        identity = {}
        for field in ["Name", "Version", "FullName"]:
            try:
                identity[field] = str(getattr(app, field))
            except (pythoncom.com_error, AttributeError):
                identity[field] = "unavailable"
        update("identity", application=identity)
        # 舊版可能用Intellicad登錄；只有實際程式身份明確為浩辰才容許新建圖面。
        identity_text = " ".join(identity.values()).lower()
        confirmed = bool(re.search(r"gstar|浩辰", identity_text))
        actual_exe = identity.get("FullName", "")
        registered_exe = selected["server"]
        if actual_exe and actual_exe != "unavailable" and registered_exe:
            if os.path.normcase(os.path.abspath(actual_exe)) != os.path.normcase(os.path.abspath(registered_exe)):
                raise RuntimeError("連接程式路徑與所選COM登錄不同，拒絕操作；請分別開啟版本後診斷。")
        docs = app.Documents
        update("diagnose", document_count=int(docs.Count), gstar_identity_confirmed=confirmed,
               units="診斷不讀取圖面內容，不變更單位或視圖")
        if request["mode"] == "diagnose":
            update("complete", status="PASS_DIAGNOSTIC", cad_versions_tested=[identity])
            return
        if not confirmed:
            raise RuntimeError("可連接COM，但無法從實際應用名稱／路徑確認為浩辰；僅完成診斷，不新建圖面。")
        app.Visible = True
        update("create_document")
        doc = docs.Add()
        model = doc.ModelSpace
        # 文件來自Documents.Add，而不是ActiveDocument；空白範本若已有模型圖元便停止。
        if int(model.Count) != 0:
            raise RuntimeError("新文件範本不是空白模型空間，停止繪圖；新文件保留供檢查。")
        update("set_units", owned_document_created=True)
        doc.SetVariable("INSUNITS", 4)  # 插入單位：毫米；座標本身仍須依本工具規格核對。
        doc.SetVariable("LUNITS", 2)
        if int(doc.GetVariable("INSUNITS")) != 4:
            raise RuntimeError("無法確認新文件毫米單位。")
        update("rectangle")
        corners = [(0, 0), (200, 0), (200, 100), (0, 100)]
        for i in range(4):
            model.AddLine(point(*corners[i]), point(*corners[(i + 1) % 4]))
        update("holes")
        for x, y in [(15, 15), (185, 15), (185, 85), (15, 85)]:
            model.AddCircle(point(x, y), 5.)
        with_dimensions = request["mode"] == "dimensions"
        if with_dimensions:
            update("dimensions")
            for variable, value in [("DIMTXT", 2.5), ("DIMASZ", 2.5), ("DIMSCALE", 1.), ("DIMLFAC", 1.)]:
                doc.SetVariable(variable, value)
            model.AddDimAligned(point(0, 0), point(200, 0), point(100, -15))
            model.AddDimAligned(point(200, 0), point(200, 100), point(220, 50))
            model.AddDimDiametric(point(10, 15), point(20, 15), 10.)
        doc.Regen(1)
        rows = geometry(model)
        verify_geometry(rows, with_dimensions)
        update("geometry_verified", geometry=rows)
        # 輸出資料夾由父程序以隨機名稱新建；仍檢查不覆寫既有檔案。
        output = request["output"]
        if os.path.lexists(output):
            raise RuntimeError("輸出DWG已存在，拒絕覆寫。")
        update("save")
        doc.SaveAs(output)
        if not os.path.isfile(output) or os.path.getsize(output) == 0:
            raise RuntimeError("CAD未產生可核對的DWG檔案。")
        update("reopen", drawing_saved=output)
        doc.Close(False)  # 僅關閉本工具建立且已保存的文件。
        reopened = docs.Open(output)
        if os.path.normcase(os.path.abspath(str(reopened.FullName))) != os.path.normcase(os.path.abspath(output)):
            raise RuntimeError("重開文件不是本次輸出，停止核對。")
        rows = geometry(reopened.ModelSpace)
        verify_geometry(rows, with_dimensions)
        assert int(reopened.GetVariable("INSUNITS")) == 4, "重開後單位不符"
        update("complete", status="PASS_DRAWING", reopened_geometry=rows,
               drawing_saved=output, dimensions_verified=with_dimensions)
        # 留下重開的測試文件讓使用者目視核對，不結束CAD或其他文件。
    finally:
        pythoncom.CoUninitialize()
