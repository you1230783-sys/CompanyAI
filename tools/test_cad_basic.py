"""基礎契約測試，不啟動 CAD，不以替身測試宣稱實機相容性。"""
import json
import math
from pathlib import Path
from types import SimpleNamespace
import unittest

import worker


class BasicTests(unittest.TestCase):
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
            def AddLeader(self, points, annotation, kind):
                calls.append(kind)
                return SimpleNamespace(Handle="1", Type=kind)
            def AddText(self, text, point, height):
                return SimpleNamespace(Handle="2", TextString=text, InsertionPoint=point, Height=height)
        drawing = worker.Drawing.__new__(worker.Drawing)
        drawing.legacy, drawing.model, drawing.expected = False, Model(), []
        drawing.point, drawing.points = lambda p: p, lambda p: p
        op = worker.prepare([dict(kind="leader", color=5, points=[[25,20],[50,45],[100,45]], text="此孔直徑10", height=2.5)])[0]
        drawing.draw(op)
        self.assertEqual(calls, [2])
        self.assertEqual(drawing.expected[0]["properties"]["Type"], 2)
        self.assertEqual(drawing.expected[0]["properties"]["Color"], 5)
        self.assertEqual(drawing.expected[1]["properties"]["InsertionPoint"], [52,48,0])

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
