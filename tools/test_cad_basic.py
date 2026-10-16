"""基礎契約測試，不啟動 CAD，不以替身測試宣稱實機相容性。"""
import json
import math
from pathlib import Path
from types import SimpleNamespace
import unittest
import tempfile
import hashlib
import os

import worker
import cad_existing
import cad_active
from unittest.mock import patch


class BasicTests(unittest.TestCase):
    def test_command_snapshot_ignores_only_floating_point_roundoff(self):
        a = {"handle":"17B", "properties":{"StartPoint":[62., -53.58732180445816, 0.]}}
        b = {"handle":"17B", "properties":{"StartPoint":[62., -53.587321804458156, 0.]}}
        self.assertTrue(worker.command_snapshot_equal(a, b))
        b["properties"]["StartPoint"][0] += .001
        self.assertFalse(worker.command_snapshot_equal(a, b))
        self.assertFalse(worker.command_snapshot_equal(a, {**a, "text":"new"}))

    def test_legacy_zoom_records_first_dimension_text_but_rejects_changed_text(self):
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.legacy, drawing.report = True, {}
        entity = SimpleNamespace(Handle="A", EntityName="DimAligned", Color=7)
        drawing.doc = SimpleNamespace(ModelSpace=[entity], PaperSpace=[],
            GetVariable=lambda key:0, Activate=lambda:None,
            ActiveViewport=SimpleNamespace(ZoomExtents=lambda:setattr(entity, "TextString", "220.0000")))
        drawing.run_command("ZOOM EXTENTS")
        self.assertEqual(drawing.report["commands"][0]["dimension_text_first_readable"][0]["TextString"], "220.0000")
        drawing.doc.ActiveViewport.ZoomExtents = lambda:setattr(entity, "TextString", "999.0000")
        with self.assertRaisesRegex(ValueError, "圖元可讀內容不符"):
            drawing.run_command("ZOOM EXTENTS")
        self.assertIn("entity_differences", drawing.report["commands"][-1])

    def test_missing_typelib_is_installation_error_not_attribute_error(self):
        class Broken:
            def GetTypeInfo(self):
                raise worker.pythoncom.com_error(-2147319779, "not registered", None, None)
        with self.assertRaises(worker.CadInstallationError) as caught:
            worker.check_type_library(Broken())
        result = worker.failure_details(dict(drawing_started=False), caught.exception)
        self.assertEqual(result["recovery"], "repair_installation")

    def test_gstar_user_data_cache_standard_linetype(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            support = root / "UserDataCache" / "Support"
            support.mkdir(parents=True)
            source = support / "gcadiso.lin"
            source.write_text("*CENTER,Center\nA,12,-3,2,-3\n", encoding="ascii")
            class Table(list):
                def Load(self, name, filename):
                    self.append(SimpleNamespace(Name=name))
                    self.source = filename
                def Item(self, name): return self[0]
            table = Table()
            drawing = worker.Drawing.__new__(worker.Drawing)
            drawing.app = SimpleNamespace(FullName=str(root / "gcad.exe"))
            drawing.doc, drawing.report = SimpleNamespace(Linetypes=table), {}
            drawing.prepare_linetypes([dict(name="CENTER", scale=1)])
            self.assertEqual(table.source, str(source))

    def test_connect_retains_the_launched_application_object(self):
        selected = dict(clsid="{8011C111-7E36-11D1-BC5A-0060089608A4}", server=r"C:\CAD\icad.exe")
        app = SimpleNamespace(Name="ICAD", Version="5", FullName=selected["server"])
        missing = worker.pythoncom.com_error(-2147221021, "not running", None, None)
        report = {}
        with patch.object(worker.pythoncom, "GetActiveObject", side_effect=missing), \
             patch.object(worker.pythoncom, "CoCreateInstance", return_value=object()) as launch, \
             patch.object(worker.dynamic, "Dispatch", return_value=app):
            connected, identity = worker.connect_application(selected, dict(action="draw", launch_if_missing=True), report)
        self.assertIs(connected, app)
        self.assertTrue(app.Visible)
        self.assertEqual(identity["FullName"], selected["server"])
        launch.assert_called_once()
        self.assertTrue(report["connection"]["same_worker"])

    def test_connect_does_not_launch_without_user_permission(self):
        missing = worker.pythoncom.com_error(-2147221021, "not running", None, None)
        selected = dict(clsid="{8011C111-7E36-11D1-BC5A-0060089608A4}", server=r"C:\CAD\icad.exe")
        with patch.object(worker.pythoncom, "GetActiveObject", side_effect=missing), \
             patch.object(worker.pythoncom, "CoCreateInstance") as launch:
            with self.assertRaises(worker.pythoncom.com_error):
                worker.connect_application(selected, dict(action="draw"), {})
        launch.assert_not_called()

    def test_line_style_validation_and_legacy_defaults(self):
        op = dict(kind="line", color=7, start=[0,0], end=[100,0])
        self.assertNotIn("line_style", worker.prepare([op])[0])
        self.assertEqual(worker.prepare([dict(op, line_style=dict(name="dashed", scale=2))])[0]["line_style"],
                         dict(name="DASHED", scale=2.0))
        for style in (dict(name="*",scale=1),dict(name="CENTER",scale=0),
                      dict(name="CENTER",scale=True),dict(name="CENTER",scale=1,path="x.lin")):
            with self.assertRaises(ValueError): worker.prepare([dict(op,line_style=style)])

    def test_linetype_loads_only_missing_standard_definition(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "icadiso.lin").write_text("*DASHED,Dashed\nA,12,-6\n", encoding="ascii")
            class Table(list):
                def Load(self, name, filename):
                    self.append(SimpleNamespace(Name=name))
                    self.source = filename
                def Item(self, name):
                    return next(item for item in self if item.Name.upper() == name.upper())
            table = Table()
            drawing = worker.Drawing.__new__(worker.Drawing)
            drawing.app = SimpleNamespace(FullName=str(root / "icad.exe"))
            drawing.doc, drawing.report = SimpleNamespace(Linetypes=table), {}
            drawing.prepare_linetypes([dict(name="DASHED",scale=1)])
            self.assertEqual(table.source, str(root / "icadiso.lin"))
            drawing.prepare_linetypes([dict(name="DASHED",scale=2)])
            self.assertEqual(len(table), 1)
            entity = SimpleNamespace()
            properties = drawing.apply_line_style(entity,dict(name="DASHED",scale=2))
            self.assertEqual(properties,dict(Linetype="DASHED",LinetypeScale=2.0))
            with self.assertRaisesRegex(ValueError,"找不到標準線型"):
                drawing.prepare_linetypes([dict(name="CENTER",scale=1)])

    def test_linetype_readback_failure_is_not_success(self):
        class Entity:
            Linetype = "DASHED"
            @property
            def LinetypeScale(self): return 1.0
            @LinetypeScale.setter
            def LinetypeScale(self, value): pass
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.linetype_names = {"DASHED":"DASHED"}
        with self.assertRaisesRegex(ValueError,"比例讀回不符"):
            drawing.apply_line_style(Entity(),dict(name="DASHED",scale=2))

    def test_active_zoom_does_not_require_undo_or_save(self):
        app, doc, entity, calls = self.active_fixture()
        del doc.StartUndoMark
        del doc.EndUndoMark
        doc.PaperSpace = []
        doc.GetVariable = lambda key: 0
        doc.Activate = lambda: calls.append("activate")
        doc.ActiveViewport = SimpleNamespace(ZoomExtents=lambda:calls.append("zoom"))
        _, rows = cad_existing.snapshot(doc,worker)
        report = {"completed_operations":0}
        result = cad_existing.edit_document(app,doc,True,dict(edits=[],operations=[dict(kind="command",command="ZOOM E")]),
            report,worker,{"A":entity},rows,guard=lambda:None)
        self.assertTrue(result["view_only"])
        self.assertFalse(result["saved"])
        self.assertEqual(calls,["activate","zoom"])
        self.assertEqual(report["completed_operations"],1)

    def test_active_zoom_mixed_edit_still_requires_undo(self):
        app, doc, entity, calls = self.active_fixture()
        del doc.StartUndoMark
        doc.ActiveViewport = SimpleNamespace(ZoomExtents=lambda:self.fail("不應縮放"))
        _, rows = cad_existing.snapshot(doc,worker)
        with self.assertRaises(AttributeError):
            cad_existing.edit_document(app,doc,True,
                dict(edits=[dict(handle="A",kind="color",value=3)],operations=[dict(kind="command",command="ZOOM E")]),
                {"completed_operations":0},worker,{"A":entity},rows,guard=lambda:None)
        self.assertEqual(entity.Color,1)
        self.assertEqual(calls,[])

    def test_active_linetype_edit_preserves_unsaved_contract(self):
        app, doc, entity, calls = self.active_fixture()
        entity.Linetype, entity.LinetypeScale = "Continuous", 1.0
        doc.Linetypes = [SimpleNamespace(Name="Continuous"),SimpleNamespace(Name="DASHED")]
        app.FullName = r"C:\CAD\icad.exe"
        _, rows = cad_existing.snapshot(doc,worker)
        result = cad_existing.edit_document(app,doc,False,
            dict(edits=[dict(handle="A",kind="linetype",value=dict(name="DASHED",scale=2))],operations=[]),
            {"completed_operations":0},worker,{"A":entity},rows,guard=lambda:None)
        self.assertEqual(entity.Linetype,"DASHED")
        self.assertEqual(entity.LinetypeScale,2.0)
        self.assertFalse(result["saved"])
        self.assertEqual(calls,["begin","end","regen"])

    def test_process_identity_reads_current_windows_process(self):
        record = cad_active.process_record(os.getpid())
        self.assertEqual(record["pid"], os.getpid())
        self.assertTrue(record["created"])
        self.assertTrue(os.path.isabs(record["executable"]))

    def test_process_discovery_requires_exact_path_and_one_instance(self):
        app = SimpleNamespace(FullName=r"C:\CAD\icad.exe")
        record = dict(pid=11, created="start", executable=os.path.normcase(app.FullName))
        with patch.object(cad_active.win32process,"EnumProcesses",return_value=[11]), \
             patch.object(cad_active,"process_record",return_value=record), \
             patch.object(cad_active.win32gui,"EnumWindows",side_effect=lambda callback,arg:callback(123,arg)), \
             patch.object(cad_active.win32process,"GetWindowThreadProcessId",return_value=(1,11)):
            self.assertEqual(cad_active.discover_process(app),(record,None))
            app.FullName=r"C:\Other\icad.exe"
            self.assertIsNone(cad_active.discover_process(app)[0])
            app.FullName=r"C:\CAD\icad.exe"
            with patch.object(cad_active.win32process,"EnumProcesses",return_value=[11,22]):
                self.assertIsNone(cad_active.discover_process(app)[0])

    def test_process_without_desktop_window_is_readonly(self):
        app=SimpleNamespace(FullName=r"C:\CAD\icad.exe")
        with patch.object(cad_active.win32process,"EnumProcesses",return_value=[11]), \
             patch.object(cad_active,"process_record",return_value=dict(pid=11,created="start",executable=os.path.normcase(app.FullName))), \
             patch.object(cad_active.win32gui,"EnumWindows",return_value=None):
            self.assertIsNone(cad_active.discover_process(app)[0])

    def test_missing_com_hwnd_uses_process_and_unique_document(self):
        app,doc,_,_=self.active_fixture()
        app.Documents=[doc]
        record=dict(pid=11,created="start",executable=r"C:\CAD\icad.exe")
        with patch.object(cad_active,"discover_process",return_value=(record,None)):
            result=cad_active.identity(app,doc,worker)
            self.assertTrue(result["editable_session"])
            self.assertEqual(result["identity_scope"],"process_and_document_snapshot")
            app.Documents.append(SimpleNamespace(Name=doc.Name,FullName=doc.FullName))
            self.assertFalse(cad_active.identity(app,doc,worker)["editable_session"])

    def test_restarted_process_rejects_stale_active_edit(self):
        app,doc,_,calls=self.active_fixture()
        app.Documents=[doc]
        record=dict(pid=11,created="old",executable=r"C:\CAD\icad.exe")
        with patch.object(cad_active,"discover_process",return_value=(record,None)):
            before=cad_active.execute_active(app,True,dict(action="active_read",offset=0),{},worker)
            record["created"]="new"
            request=dict(action="active_edit",document=before["document"],revision=before["revision"],
                         edits=[dict(handle="A",kind="radius",value=8)],operations=[])
            with self.assertRaisesRegex(ValueError,"文件身分或內容已變更"):
                cad_active.execute_active(app,True,request,{},worker)
        self.assertEqual(calls,[])

    def test_command_parser_rejects_programs_and_unbounded_commands(self):
        for value in ('_.ZOOM _E', 'ZOOM A', 'REGEN', '_-PURGE _A * _N', 'AUDIT N'):
            self.assertEqual(worker.prepare([dict(kind="command",command=value)])[0]["kind"], "command")
        for value in ('AUDIT Y','SAVEAS x.dwg','EXPLODE ALL','SHELL','(load "x")',
                      'REGEN\nQUIT','ZOOM E;QUIT','PURGE *','SCRIPT x','ZOOM 2X'):
            with self.assertRaises(ValueError): worker.prepare([dict(kind="command",command=value)])
        with self.assertRaises(ValueError):
            worker.prepare([dict(kind="command",command="REGEN"),dict(kind="circle",color=1,center=[0,0],radius=2)])

    def test_commands_sync_no_save_and_readable_entities_preserved(self):
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.legacy, drawing.report = True, {}
        entity = SimpleNamespace(Handle="1",ObjectName="Circle",Radius=5.,Color=1)
        calls = []
        blocks = [SimpleNamespace(Name="unused")]
        drawing.app = SimpleNamespace(ZoomExtents=lambda:calls.append("zoom"),ZoomAll=lambda:calls.append("all"))
        drawing.doc = SimpleNamespace(ModelSpace=[entity],PaperSpace=[],Blocks=blocks,Layers=[],
            Linetypes=[],TextStyles=[],DimensionStyles=[],GetVariable=lambda key:0,
            ActiveViewport=SimpleNamespace(ZoomExtents=lambda:calls.append("viewport_zoom"),ZoomAll=lambda:calls.append("viewport_all")),
            Activate=lambda:calls.append("activate"),Regen=lambda:calls.append("regen"),
            PurgeAll=lambda:blocks.clear(),AuditInfo=lambda fix:calls.append(("audit",fix)))
        for command in ('ZOOM EXTENTS','ZOOM ALL','REGEN','PURGE','AUDIT N'):
            drawing.run_command(command)
        self.assertIn(("audit",False), calls)
        self.assertIn("viewport_zoom", calls)
        self.assertNotIn("zoom", calls)
        self.assertEqual(drawing.report["commands"][3]["removed_definitions"]["Blocks"],["unused"])
        self.assertTrue(drawing.report["commands"][-1]["audit_clean_not_verified"])
        drawing.doc.PurgeAll=lambda:setattr(entity,"Radius",10.)
        with self.assertRaisesRegex(ValueError,"圖元可讀內容不符"): drawing.run_command("PURGE")
        self.assertEqual(drawing.report["commands"][-1]["status"],"STARTED")

    def test_busy_cad_command_is_not_executed(self):
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.doc = SimpleNamespace(Regen=lambda:self.fail("不應執行"),GetVariable=lambda key:1)
        with self.assertRaisesRegex(ValueError,"正在執行其他指令"): drawing.run_command("REGEN")

    def test_modern_zoom_remains_on_application(self):
        drawing = worker.Drawing.__new__(worker.Drawing)
        calls = []
        drawing.legacy, drawing.doc = False, SimpleNamespace()
        drawing.app = SimpleNamespace(ZoomExtents=lambda:calls.append("modern"))
        drawing.command_method("ZOOM EXTENTS")()
        self.assertEqual(calls,["modern"])

    def test_unsupported_command_preflight_does_not_create_document(self):
        app = SimpleNamespace(Name="ICAD",Version="5.0.20.0",FullName=r"C:\CAD\icad.exe",
            Documents=SimpleNamespace(Count=1,Add=lambda:self.fail("不應建立文件")),
            ActiveDocument=SimpleNamespace(ActiveViewport=SimpleNamespace()))
        selected = dict(server_exists=True,progid="Icad.Application.1",server=app.FullName,
                        clsid="{8011C111-7E36-11D1-BC5A-0060089608A4}")
        raw = SimpleNamespace(QueryInterface=lambda iid:None)
        report = {}
        with patch.object(worker.registry,"discover",return_value=([selected],[])), \
             patch.object(worker.pythoncom,"GetActiveObject",return_value=raw), \
             patch.object(worker.dynamic,"Dispatch",return_value=app), \
             patch.object(worker,"identify_product",return_value="yoda_icad_2008"), \
             self.assertRaises(worker.CommandCapabilityError) as caught:
            worker.execute(dict(action="draw",version="latest",operations=[dict(kind="command",command="ZOOM E")]),report)
        self.assertFalse(report["drawing_started"])
        self.assertEqual(report["step"],"validate_commands")
        self.assertEqual(report["unsupported_commands"],["ZOOM EXTENTS"])
        self.assertFalse(report["command_capabilities"][0]["executed"])
        self.assertEqual(worker.failure_details(report,caught.exception)["recovery"],"correct_and_retry")
        report.update(owned_document_created=True,drawing_started=True)
        self.assertEqual(worker.failure_details(report,caught.exception)["recovery"],"revised_new_document")

    def test_unsupported_active_command_stops_before_undo(self):
        app,doc,entity,calls=self.active_fixture()
        request=self.active_request(app,doc,[])
        request["operations"]=[dict(kind="command",command="ZOOM E")]
        report={"completed_operations":0,"drawing_started":False}
        with patch.object(cad_active,"identity",return_value=request["document"]),self.assertRaises(worker.CommandCapabilityError):
            cad_active.execute_active(app,True,request,report,worker)
        self.assertEqual(calls,[])
        self.assertFalse(report["drawing_started"])

    def test_command_preflight_without_document_is_explicitly_deferred(self):
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.legacy,drawing.doc,drawing.report=True,None,{}
        drawing.preflight_commands([dict(kind="command",command="ZOOM EXTENTS")])
        self.assertEqual(drawing.report["command_capabilities"][0]["status"],"DEFERRED_NO_DOCUMENT")

    def test_missing_hwnd_still_reads_without_edit_permission(self):
        app, doc, _, calls = self.active_fixture()
        result = cad_active.execute_active(app, True, dict(action="active_read", offset=0), {}, worker)
        self.assertEqual(result["status"], "READ_ACTIVE")
        self.assertFalse(result["document"]["editable_session"])
        self.assertEqual(result["entity_count"], 1)
        self.assertEqual(calls, [])
        request = dict(action="active_edit", document=result["document"], revision=result["revision"],
                       edits=[dict(handle="A", kind="delete", value=None)], operations=[])
        with self.assertRaisesRegex(ValueError, "可以閱讀"):
            cad_active.execute_active(app, True, request, {}, worker)
        self.assertEqual(calls, [])

    def test_application_hwnd_fallback_and_duplicate_document(self):
        app, doc, _, _ = self.active_fixture()
        app.HWND, app.Documents = 123, [doc]
        process = SimpleNamespace(Close=lambda: None)
        with patch.object(cad_active.win32gui, "IsWindow", return_value=True), \
             patch.object(cad_active.win32process, "GetWindowThreadProcessId", return_value=(1, 2)), \
             patch.object(cad_active.win32api, "OpenProcess", return_value=process), \
             patch.object(cad_active.win32process, "GetProcessTimes", return_value={"CreationTime":"now"}):
            result = cad_active.identity(app, doc, worker)
            self.assertEqual(result["identity_scope"], "application_window_and_snapshot")
            self.assertTrue(result["editable_session"])
            app.Documents.append(SimpleNamespace(Name=doc.Name, FullName=doc.FullName))
            self.assertFalse(cad_active.identity(app, doc, worker)["editable_session"])

    def test_missing_interface_is_not_retryable(self):
        result = worker.failure_details(dict(drawing_started=False, step="read_active"), AttributeError("HWND"))
        self.assertEqual(result["recovery"], "unsupported_capability")
        self.assertFalse(result["connection_lost"])

    def test_annotation_style_preserves_defaults_and_bigfont(self):
        values = dict(DIMTXT=9, DIMASZ=9, DIMGAP=4, DIMTIH=1, DIMTOH=0,
                      DIMTAD=0, DIMTSZ=2, DIMSCALE=2, DIMTXSTY="User")
        original = values.copy()
        style, dimstyle = SimpleNamespace(), SimpleNamespace()
        captured = {}
        dimstyle.CopyFrom = lambda doc: captured.update(values)
        doc = SimpleNamespace(TextStyles=SimpleNamespace(Add=lambda name: style,
            Item=lambda name: SimpleNamespace(BigFontFile="chineset.shx")),
            ActiveTextStyle=SimpleNamespace(BigFontFile=""),
            DimStyles=SimpleNamespace(Add=lambda name: dimstyle),
            GetVariable=lambda name: values[name], SetVariable=lambda name, value: values.__setitem__(name,value))
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.app, drawing.doc, drawing.report = SimpleNamespace(), doc, {}
        drawing.legacy = False
        name = drawing.dimension_style()
        self.assertEqual(style.FontFile, "simplex.shx")
        self.assertEqual(style.BigFontFile, "chineset.shx")
        self.assertEqual(values, original)
        self.assertEqual([captured[k] for k in ("DIMTXT","DIMASZ","DIMGAP","DIMTIH","DIMTOH")], [3,3,1,0,1])
        self.assertEqual(captured["DIMTXSTY"], name)

    def test_legacy_dimension_styles_native_properties_without_alias_or_copy(self):
        class NativeStyle:
            __slots__ = ("DimTxt", "DimAsz", "DimGap", "DimTih", "DimToh", "DimTad",
                         "DimTsz", "DimScale", "DimTxSty")
        style = NativeStyle()
        calls = []
        class LegacyDocument:
            DimensionStyles = SimpleNamespace(Add=lambda name: calls.append(name) or style,
                                             Item=lambda name: style)
            @property
            def DimStyles(self):
                raise AssertionError("iCAD 2008 must not access DimStyles")
            def SetVariable(self, *args):
                raise AssertionError("native style must not change document defaults")
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.doc, drawing.legacy, drawing.report = LegacyDocument(), True, {}
        drawing.annotation_style = lambda: "LM_test"
        drawing.prepare_styles([dict(kind="line"), dict(kind="aligned_dimension"), dict(kind="leader")])
        self.assertEqual(calls, ["LM_test"])
        self.assertEqual([style.DimTxt, style.DimAsz, style.DimGap, style.DimTih, style.DimToh], [3,3,1,0,1])
        self.assertEqual(style.DimTxSty, "LM_test")
        drawing.verify_styles(drawing.doc)
        style.DimAsz = 9
        with self.assertRaisesRegex(ValueError, "DimAsz"):
            drawing.verify_styles(drawing.doc)

    def test_text_style_does_not_access_dimension_collections(self):
        style = SimpleNamespace()
        doc = SimpleNamespace(TextStyles=SimpleNamespace(Add=lambda name: style,
            Item=lambda name: SimpleNamespace(BigFontFile="chineset.shx")),
            ActiveTextStyle=SimpleNamespace(BigFontFile=""), GetVariable=lambda name:"User")
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.app, drawing.doc, drawing.report, drawing.legacy = SimpleNamespace(), doc, {}, True
        drawing.prepare_styles([dict(kind="text")])
        self.assertEqual(style.FontFile, "simplex.shx")
        self.assertFalse(hasattr(drawing, "dimension_style_name"))

    def test_style_preflight_failure_keeps_existing_entities_and_closes_undo(self):
        app, doc, entity, calls = self.active_fixture()
        request = self.active_request(app, doc, [dict(handle="A", kind="delete", value=None)])
        request["operations"] = [dict(kind="aligned_dimension", color=7,
                                      start=[0,0], end=[200,0], location=[100,-15])]
        report = {"completed_operations":0}
        with patch.object(cad_active,"identity",return_value=request["document"]), \
             patch.object(worker.Drawing,"prepare_styles",side_effect=AttributeError("DimensionStyles")), \
             self.assertRaises(AttributeError):
            cad_active.execute_active(app, True, request, report, worker)
        self.assertEqual(doc.ModelSpace, [entity])
        self.assertEqual(calls, ["begin","end"])
        self.assertEqual(report["completed_operations"], 0)
        self.assertEqual(report["step"], "prepare_annotation_styles")

    def test_legacy_native_leader_creates_text_annotation_once(self):
        class Model(list):
            @property
            def Count(self): return len(self)
            def AddLeader(self, points, annotation, kind):
                self.argument = annotation
                obj = SimpleNamespace(Handle="1", Type=kind, ObjectName="AcDbLeader")
                linked = SimpleNamespace(Handle="2", TextString=annotation, Height=0., Color=256,
                                         InsertionPoint=points[-1], ObjectName="AcDbMText")
                obj.Annotation = linked
                self.extend([obj, linked])
                return obj
            def AddText(self, text, point, height):
                obj = SimpleNamespace(Handle="3", TextString=text, InsertionPoint=point,
                                      Height=height, ObjectName="AcDbText")
                self.append(obj)
                return obj
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.legacy, drawing.model, drawing.expected, drawing.report = True, Model(), [], {}
        drawing.point, drawing.points = lambda p: p, lambda p: p
        drawing.annotation_style = lambda: "LM_test"
        drawing.dimension_style = lambda: "LM_test"
        drawing.draw(worker.prepare([dict(kind="leader", color=3, points=[[0,0],[10,10],[30,10]],
                                         text="%%c10", height=3)])[0])
        self.assertEqual(drawing.model.argument, "%%c10")
        self.assertEqual(len(drawing.expected), 2)
        self.assertEqual(drawing.model[1].InsertionPoint, [11.,11.,0.])
        self.assertEqual(drawing.model[1].Height, 3.)
        self.assertEqual(drawing.model[1].AttachmentPoint, 7)
        self.assertEqual(drawing.model[1].Rotation, 0)
        self.assertEqual(drawing.model[1].Color, 3)
        self.assertEqual(len(drawing.report["operation_entities"][0]["entities"]), 2)
        doc = SimpleNamespace(ModelSpace=drawing.model, GetVariable=lambda key:4)
        self.assertTrue(drawing.verify(doc)["geometry_and_colors_verified"])
        drawing.model.append(SimpleNamespace(Handle="X", ObjectName="AcDbPoint", Color=3))
        with self.assertRaisesRegex(ValueError, "圖元集合不符"):
            drawing.verify(doc)
        self.assertEqual(drawing.report["entity_audit"]["unexpected_entities"][0]["handle"], "X")

    def test_com_enumerator_is_reset_for_each_snapshot(self):
        class Enum:
            def __init__(self): self.resets = 0; self.offset = 0
            def QueryInterface(self, iid): return self
            def Reset(self): self.offset = 0; self.resets += 1
            def __iter__(self): return self
            def __next__(self):
                if self.offset == 2: raise StopIteration
                self.offset += 1
                return self.offset
        enum = Enum()
        space = SimpleNamespace(_oleobj_=SimpleNamespace(InvokeTypes=lambda *args:enum))
        from win32com.client import util
        with patch.object(util, "Iterator", side_effect=lambda obj: iter(obj)):
            self.assertEqual(list(cad_existing.iter_entities(space)), [1,2])
            self.assertEqual(list(cad_existing.iter_entities(space)), [1,2])
        self.assertEqual(enum.resets, 2)

    def test_legacy_hatch_cleanup_preserves_five_boundaries(self):
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.legacy, drawing.expected, drawing.report = True, [], {"operation_index":23}
        drawing.point = drawing.points = lambda p:p
        class Space(list):
            @property
            def Count(self): return len(self)
            def add(self, kind, **values):
                obj = SimpleNamespace(Handle=str(len(self)+1), ObjectName=kind, **values)
                self.append(obj)
                return obj
            def AddLine(self, a, b): return self.add("Line", StartPoint=a, EndPoint=b)
            def AddCircle(self, p, r): return self.add("Circle", Center=p, Radius=r)
            def AddHatch(self, *args): return self.add("Hatch", ApplyHatch=lambda *args:None)
        space = Space()
        class Selection(list):
            @property
            def Count(self): return len(self)
            def AddItems(self, items): self.extend(items)
            def Clear(self): self.clear()
            def Delete(self):
                for entity in self: space.remove(entity)
        selection = Selection()
        drawing.model = space
        drawing.doc = SimpleNamespace(SelectionSets=SimpleNamespace(Add=lambda name:selection),
                                      ModelSpace=space, GetVariable=lambda key:4)
        op = dict(kind="hatch", color=8, boundary=[[0,0,0],[10,0,0],[10,10,0],[0,10,0]],
                  holes=[dict(center=[5,5,0],radius=1)], scale=3, seed=[1,1,0])
        with patch.object(worker, "VARIANT", side_effect=lambda kind, items:items):
            drawing.draw(op)
        self.assertEqual(len(space), 6)
        self.assertTrue(drawing.verify(drawing.doc)["geometry_and_colors_verified"])
        self.assertEqual(drawing.report["operation_entities"][0]["removed_handles"], [])
        space.pop(0)
        with self.assertRaises(ValueError): drawing.verify(drawing.doc)
        self.assertEqual(drawing.last_audit["missing_expected_entities"][0]["operation_index"],23)

    def test_dimensions_are_delete_only(self):
        app, doc, entity, calls = self.active_fixture()
        entity.ObjectName = "AcDbAlignedDimension"
        row = cad_existing.describe(entity, worker)
        self.assertTrue(row["editable"])
        self.assertEqual(row["allowed_edits"], ["delete"])
        request = self.active_request(app, doc, [dict(handle="A",kind="color",value=3)])
        with patch.object(cad_active,"identity",return_value=request["document"]), self.assertRaisesRegex(ValueError,"刪除後重建"):
            cad_active.execute_active(app, True, request, {"completed_operations":0}, worker)
        self.assertEqual(calls, [])

    def active_fixture(self):
        entity = SimpleNamespace(Handle="A", ObjectName="AcDbCircle", Center=[0.,0.,0.], Radius=5., Layer="0", Color=1)
        calls=[]
        class Space(list):
            Count=1
        doc=SimpleNamespace(Name="Drawing1", FullName="", _oleobj_=object(), ModelSpace=Space([entity]),
            GetVariable=lambda key:4, Layers=SimpleNamespace(Item=lambda name:SimpleNamespace(Lock=False)),
            StartUndoMark=lambda:calls.append("begin"),EndUndoMark=lambda:calls.append("end"),
            Regen=lambda *args:calls.append("regen"), Save=lambda: self.fail("active must not save"),
            Close=lambda *args:self.fail("active must not close"))
        entity.Delete=lambda:doc.ModelSpace.remove(entity)
        app=SimpleNamespace(ActiveDocument=doc)
        return app,doc,entity,calls

    def active_request(self, app, doc, edits):
        _,rows=cad_existing.snapshot(doc,worker)
        return dict(action="active_edit",document={"name":"Drawing1"},revision=cad_active.revision(doc,rows),edits=edits,operations=[])

    def test_active_edit_unsaved_undo_and_radius(self):
        app,doc,entity,calls=self.active_fixture()
        request=self.active_request(app,doc,[dict(handle="A",kind="radius",value=10)])
        with patch.object(cad_active,"identity",return_value=request["document"]):
            result=cad_active.execute_active(app,False,request,{"completed_operations":0},worker)
        self.assertEqual(result["status"],"EDITED_UNSAVED")
        self.assertFalse(result["saved"])
        self.assertEqual(entity.Radius,10)
        self.assertEqual(calls,["begin","end","regen"])

    def test_arc_radius_recomputes_readonly_endpoints(self):
        app,doc,_,calls=self.active_fixture()
        class Arc:
            Handle="A"
            ObjectName="AcDbArc"
            Layer="0"
            Color=1
            Center=[10.,20.,0.]
            Radius=5.
            @property
            def StartPoint(self): return [10.+self.Radius,20.,0.]
            @property
            def EndPoint(self): return [10.,20.+self.Radius,0.]
        entity=Arc()
        doc.ModelSpace[:]=[entity]
        request=self.active_request(app,doc,[dict(handle="A",kind="radius",value=10)])
        with patch.object(cad_active,"identity",return_value=request["document"]):
            result=cad_active.execute_active(app,False,request,{"completed_operations":0},worker)
        self.assertEqual(result["status"],"EDITED_UNSAVED")
        self.assertEqual(entity.StartPoint,[20.,20.,0.])

    def test_active_rejects_stale_revision_before_undo(self):
        app,doc,entity,calls=self.active_fixture()
        request=self.active_request(app,doc,[dict(handle="A",kind="color",value=3)])
        entity.Radius=8
        with patch.object(cad_active,"identity",return_value=request["document"]),self.assertRaises(ValueError):
            cad_active.execute_active(app,False,request,{"completed_operations":0},worker)
        self.assertEqual(calls,[])
        self.assertEqual(entity.Color,1)

    def test_active_rejects_different_document(self):
        app,doc,entity,calls=self.active_fixture()
        request=self.active_request(app,doc,[dict(handle="A",kind="color",value=3)])
        with patch.object(cad_active,"identity",return_value={"name":"Drawing2"}),self.assertRaises(ValueError):
            cad_active.execute_active(app,False,request,{"completed_operations":0},worker)
        self.assertEqual(calls,[])

    def test_active_delete_verifies_remaining_handles(self):
        app,doc,entity,calls=self.active_fixture()
        request=self.active_request(app,doc,[dict(handle="A",kind="delete",value=None)])
        with patch.object(cad_active,"identity",return_value=request["document"]):
            result=cad_active.execute_active(app,False,request,{"completed_operations":0},worker)
        self.assertEqual(result["deleted_handles"],["A"])
        self.assertEqual(len(doc.ModelSpace),0)

    def test_active_invalid_plan_does_not_start_undo(self):
        app,doc,entity,calls=self.active_fixture()
        request=self.active_request(app,doc,[dict(handle="A",kind="radius",value=-2)])
        with patch.object(cad_active,"identity",return_value=request["document"]),self.assertRaises(ValueError):
            cad_active.execute_active(app,False,request,{"completed_operations":0},worker)
        self.assertEqual(calls,[])

    def test_active_requires_undo_support_before_mutation(self):
        app,doc,entity,calls=self.active_fixture()
        del doc.StartUndoMark
        request=self.active_request(app,doc,[dict(handle="A",kind="color",value=3)])
        report={"completed_operations":0,"drawing_started":False}
        with patch.object(cad_active,"identity",return_value=request["document"]),self.assertRaises(AttributeError):
            cad_active.execute_active(app,False,request,report,worker)
        self.assertFalse(report["drawing_started"])
        self.assertEqual(entity.Color,1)

    def test_active_failure_closes_undo_and_keeps_document(self):
        app,doc,entity,calls=self.active_fixture()
        def fail(): raise RuntimeError("COM failure")
        entity.Delete=fail
        report={"completed_operations":0}
        request=self.active_request(app,doc,[dict(handle="A",kind="delete",value=None)])
        with patch.object(cad_active,"identity",return_value=request["document"]),self.assertRaises(RuntimeError):
            cad_active.execute_active(app,False,request,report,worker)
        self.assertEqual(calls,["begin","end"])
        self.assertTrue(report["undo_group_closed"])
        self.assertEqual(len(doc.ModelSpace),1)

    def test_active_read_no_save_close_or_undo(self):
        app,doc,entity,calls=self.active_fixture()
        request=dict(action="active_read",offset=0)
        with patch.object(cad_active,"identity",return_value={"name":"Drawing1"}):
            result=cad_active.execute_active(app,False,request,{},worker)
        self.assertEqual(result["status"],"READ_ACTIVE")
        self.assertEqual(result["entities"][0]["handle"],"A")
        self.assertEqual(calls,[])

    def test_diameter_codes_preserve_other_text(self):
        self.assertEqual(worker.cad_text("孔 Ø10、Φ20、⌀5；Φ 是符號"), "孔 %%c10、%%c20、%%c5；Φ 是符號")
        self.assertTrue(worker.text_equal("%%C10", "%%c10"))
        self.assertFalse(worker.text_equal("%%c12", "%%c10"))

    def test_entity_audit_checks_handles_not_only_reported_count(self):
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.legacy = False
        drawing.expected = [{"handle":"A", "properties":{"Color":1}}]
        class Space(list):
            Count = 2
        space = Space([SimpleNamespace(Handle="A", Color=1)])
        doc = SimpleNamespace(ModelSpace=space, GetVariable=lambda key:4)
        result = drawing.verify(doc)
        self.assertEqual(result["entity_audit"]["reported_count"], 2)
        space.append(SimpleNamespace(Handle="B",Color=1))
        with self.assertRaises(ValueError): drawing.verify(doc)
        self.assertEqual(drawing.last_audit["unexpected_handles"], ["B"])

    def test_existing_copy_read_and_text_save_reopen(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[1]/".build") as folder:
            path = Path(folder)/"copy.dwg"
            path.write_bytes(b"synthetic DWG")
            entity = SimpleNamespace(Handle="AB", ObjectName="AcDbText", TextString="old", Color=7,
                                     Layer="0", InsertionPoint=[0.,0.,0.], Height=3.)
            class Space(list):
                Count = 1
            calls = []
            doc = SimpleNamespace(FullName=str(path), ModelSpace=Space([entity]), GetVariable=lambda key:4,
                                  Layers=SimpleNamespace(Item=lambda name:SimpleNamespace(Lock=False)),
                                  Regen=lambda *args:None, Close=lambda save:calls.append(("close",save)), Save=lambda:calls.append(("save",True)))
            class Documents(list):
                def Open(self, filename): return doc
            app = SimpleNamespace(Documents=Documents())
            report = {"completed_operations":0}
            read = cad_existing.execute_existing(app, False, dict(action="read",output=str(path),source_revision=hashlib.sha256(path.read_bytes()).hexdigest(),offset=0,limit=100),report,worker)
            self.assertEqual(read["entities"][0]["handle"], "AB")
            self.assertTrue(path.with_name("cad-index.json").is_file())
            request = dict(action="edit",output=str(path),source_revision=hashlib.sha256(path.read_bytes()).hexdigest(),edits=[dict(handle="AB",kind="text",value="Ø10")],operations=[])
            result = cad_existing.execute_existing(app,False,request,report,worker)
            self.assertEqual(entity.TextString,"%%c10")
            self.assertEqual(result["status"],"DATA_VERIFIED_VISUAL_PENDING")
            self.assertEqual(calls.count(("save",True)),1)
            request["edits"][0]["handle"]="UNKNOWN"
            with self.assertRaises(ValueError): cad_existing.execute_existing(app,False,request,report,worker)
            self.assertEqual(calls.count(("save",True)),1)

    def test_unknown_and_block_entities_are_not_editable(self):
        block = SimpleNamespace(Handle="F",ObjectName="AcDbBlockReference",Name="part",Layer="0",Color=256)
        row = cad_existing.describe(block, worker)
        self.assertFalse(row["editable"])
        self.assertEqual(row["properties"]["Name"],"part")
    def test_legacy_points_append_coordinates_without_indexing(self):
        class Points:
            def __init__(self):
                self.rows = []
            def Add(self, x, y, z):
                self.rows.append((x, y, z))
            def Item(self, index):
                raise AssertionError("不能假設舊版集合的索引基準")
        collection = Points()
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.legacy = True
        drawing.app = SimpleNamespace(Library=SimpleNamespace(CreatePoints=lambda: collection))
        values = [[25., 20., 0.], [50., 45., 0.], [100., 45., 0.]]
        self.assertIs(drawing.points(values), collection)
        self.assertEqual(collection.rows, [tuple(p) for p in values])

    def test_failure_recovery_requires_known_state(self):
        exc = ValueError("缺少欄位")
        self.assertEqual(worker.failure_details({"drawing_started": False}, exc)["recovery"], "correct_and_retry")
        self.assertEqual(worker.failure_details({}, exc)["recovery"], "manual_check")
        invalid = worker.pythoncom.com_error(-2147352567, "invalid", (0,None,None,None,0,-2147024809), None)
        report = dict(drawing_started=True, owned_document_created=True, step="leader")
        self.assertEqual(worker.failure_details(report, invalid)["recovery"], "revised_new_document")
        for step in ("save", "reopen"):
            self.assertEqual(worker.failure_details(dict(report, step=step), invalid)["recovery"], "manual_check")
        unknown = worker.pythoncom.com_error(-2147023174, "RPC", None, None)
        self.assertEqual(worker.failure_details(report, unknown)["recovery"], "manual_check")

    def test_icad_identity_does_not_require_brand_in_install_folder(self):
        selected = dict(server=r"C:\CAD\icad.exe", progid="Icad.Application.1",
                        clsid="{8011C111-7E36-11D1-BC5A-0060089608A4}", registry_bits=32, year=2008)
        identity = dict(FullName=selected["server"], Name="ICAD", Version="5.0.20.0")
        self.assertEqual(worker.identify_product(selected, identity), "yoda_icad_2008")
        self.assertEqual(worker.identify_product(dict(selected, clsid="unknown"), identity), "unknown")

    def test_formula_examples(self):
        self.assertAlmostEqual(worker.Formula("10*sin(2*pi*t/100)").at(25), 10)
        self.assertAlmostEqual(worker.Formula("(5+2*t)*cos(t)").at(0), 5)

    def test_formula_rejects_code(self):
        for source in ("__import__('os')", "t.real", "[1][0]", "(lambda:1)()", "2**1000"):
            with self.subTest(source=source), self.assertRaises(ValueError):
                worker.Formula(source).at(0)

    def test_formula_rejects_invalid_domains(self):
        for source in ("1/0", "sqrt(-1)", "exp(10000)"):
            with self.subTest(source=source), self.assertRaises((ValueError, ZeroDivisionError, OverflowError)):
                worker.Formula(source).at(0)

    def test_color_limits_and_fields(self):
        base = dict(kind="line", start=[0, 0], end=[1, 1], color=1)
        for color in (1, 7, 255):
            self.assertEqual(worker.prepare([dict(base, color=color)])[0]["color"], color)
        for color in (0, 256, -1, True, "red"):
            with self.assertRaises(ValueError):
                worker.prepare([dict(base, color=color)])
        with self.assertRaises(ValueError):
            worker.prepare([dict(base, command="anything")])

    def test_curve_count_and_coordinates(self):
        op = worker.prepare([dict(kind="curve", color=3, x="t", y="10*sin(2*pi*t/100)", t_start=0, t_end=200, segments=100)])[0]
        self.assertEqual(len(op["samples"]), 101)
        self.assertEqual(op["samples"][0], [0, 0, 0])
        self.assertAlmostEqual(op["samples"][-1][0], 200)

    def test_nonfinite_and_degenerate_geometry(self):
        for end in ([0, 0], [math.inf, 1], [math.nan, 1], [0, 1, 2]):
            with self.assertRaises(ValueError):
                worker.prepare([dict(kind="line", color=1, start=[0, 0], end=end)])

    def test_leader_requires_horizontal_landing(self):
        with self.assertRaises(ValueError):
            worker.prepare([dict(kind="leader", color=1, points=[[0,0],[10,10],[20,11]], text="中文", height=2.5)])

    def test_leader_type_and_color_are_read_back(self):
        calls = []
        class Model:
            def __iter__(self):
                return iter([])
            def AddLeader(self, points, annotation, kind):
                calls.append(kind)
                return SimpleNamespace(Handle="1", Type=kind)
            def AddText(self, text, point, height):
                return SimpleNamespace(Handle="2", TextString=text, InsertionPoint=point, Height=height)
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.legacy, drawing.model, drawing.expected = False, Model(), []
        drawing.point, drawing.points = lambda p: p, lambda p: p
        drawing.annotation_style = lambda: "LM_test"
        drawing.dimension_style = lambda: "LM_test"
        op = worker.prepare([dict(kind="leader", color=5, points=[[25,20],[50,45],[100,45]], text="此孔直徑10", height=2.5)])[0]
        drawing.draw(op)
        self.assertEqual(calls, [2])
        self.assertEqual(drawing.expected[0]["properties"]["Type"], 2)
        self.assertEqual(drawing.expected[0]["properties"]["Color"], 5)
        self.assertEqual(drawing.expected[1]["properties"]["InsertionPoint"], [51,46,0])
        self.assertEqual(drawing.expected[1]["properties"]["Height"], 3)

    def test_readback_detects_color_change(self):
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.legacy = False
        drawing.expected = [{"handle":"1", "properties":{"Color":3}}]
        class Space(list):
            Count = 1
        space = Space([SimpleNamespace(Handle="1", Color=3)])
        doc = SimpleNamespace(ModelSpace=space, GetVariable=lambda name:4)
        self.assertTrue(drawing.verify(doc)["geometry_and_colors_verified"])
        space[0].Color = 5
        with self.assertRaises(ValueError):
            drawing.verify(doc)

    def test_newest_priority(self):
        rows = [dict(progid=f"GStarCAD.Application.{year-2000}", year=year, registry_bits=64) for year in (2008, 2022, 2027, 2026)]
        self.assertEqual([r["year"] for r in sorted(rows, key=worker.registry.preference, reverse=True)], [2027,2026,2022,2008])


if __name__ == "__main__":
    unittest.main()
