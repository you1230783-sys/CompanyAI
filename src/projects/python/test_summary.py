"""以封裝的 Python／NumPy 驗證 summary 契約，不繞過正式 analyze 的容器限制。"""
import importlib.util
import pathlib
import unittest

import numpy as np

spec = importlib.util.spec_from_file_location(
    "worker", pathlib.Path(__file__).resolve().parents[1] / "python_worker.py")
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class SummaryTests(unittest.TestCase):
    def test_nested_numpy_values(self):
        actual = worker.normalize_summary({"n": np.int64(121), "peaks": np.array([1., 2.]),
            "nested": [{"ok": np.bool_(True), "value": np.float32(1.5)}],
            "matrix": np.array([[1, 2], [3, 4]]), "scalar": np.array(7)})
        self.assertEqual(actual, {"n": 121, "peaks": [1., 2.],
            "nested": [{"ok": True, "value": 1.5}], "matrix": [[1, 2], [3, 4]], "scalar": 7})
        self.assertIs(type(actual["n"]), int)
        self.assertIs(type(actual["nested"][0]["ok"]), bool)

    def test_nonfinite_reports_exact_path(self):
        for value in (float("nan"), float("inf"), np.float32("-inf")):
            with self.assertRaisesRegex(ValueError, r"result\['outer'\]\[0\]\['x'\].*None.*缺值原因"):
                worker.normalize_summary({"outer": [{"x": value}]})

    def test_no_silent_string_coercion(self):
        for value in (object(), 1j, {1, 2}, np.array([1j])):
            with self.assertRaisesRegex(TypeError, "result.*不支援"):
                worker.normalize_summary({"x": value})

    def test_bounded_arrays_and_encoded_length(self):
        for value in (np.zeros(12001), {"x": "x" * 12000}, ["\\" * 6000]):
            with self.assertRaises(ValueError):
                worker.normalize_summary(value)
        self.assertEqual(worker.normalize_summary("x" * 11998), "x" * 11998)

    def test_cycles_and_depth(self):
        cycle = []; cycle.append(cycle)
        with self.assertRaisesRegex(ValueError, "循環引用"):
            worker.normalize_summary(cycle)
        deep = 0
        for _ in range(70):
            deep = [deep]
        with self.assertRaisesRegex(ValueError, "巢狀過深"):
            worker.normalize_summary(deep)

    def test_existing_json_and_shared_values(self):
        shared = [None, "繁體中文", True, 1.25]
        self.assertEqual(worker.normalize_summary({"a": shared, "b": shared}),
                         {"a": shared, "b": shared})
        self.assertEqual(worker.normalize_summary({1: (2, 3)}), {"1": [2, 3]})


if __name__ == "__main__":
    unittest.main()
