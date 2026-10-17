"""批次協定的資料完整性回歸；不啟動 CAD，不代替原生效能測試。"""
import json
from pathlib import Path
import tempfile
import unittest

import cad_bulk_probe as probe


class ExportProtocolTests(unittest.TestCase):
    def parse(self, lines):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "export.jsonl"
            path.write_text("\n".join(json.dumps(row, ensure_ascii=False) for row in lines), encoding="cp950")
            return probe.parse_export(path, "nonce", "cp950")

    def fixture(self):
        return [["HEADER", 1, "nonce", "test.dwg", "Y:/test/", 4, 0],
                ["ENTITY", [[0, "MTEXT"], [5, "AbC"], [3, "中文\"引號\""], [1, "\\P下一行"],
                            [10, [.1234567890123456, -1.2345678901234567, 0.]]]],
                ["COMPLETE", "nonce", 1, 0]]

    def test_preserves_chinese_escapes_precision_and_repeated_groups(self):
        _, rows = self.parse(self.fixture())
        self.assertEqual(rows["ABC"][2], [3, '中文"引號"'])
        self.assertEqual(rows["ABC"][3], [1, "\\P下一行"])
        self.assertEqual(rows["ABC"][-1][1][0], .1234567890123456)

    def test_rejects_truncated_export(self):
        with self.assertRaisesRegex(ValueError, "完整結尾"):
            self.parse(self.fixture()[:-1])

    def test_rejects_old_or_wrong_request(self):
        data = self.fixture()
        data[0][2] = "previous"
        with self.assertRaisesRegex(ValueError, "標頭"):
            self.parse(data)

    def test_rejects_duplicate_handles_and_incomplete_count(self):
        data = self.fixture()
        data.insert(2, data[1])
        data[-1][2] = 2
        with self.assertRaisesRegex(ValueError, "重複 Handle"):
            self.parse(data)
        data[-1][2] = 1
        with self.assertRaisesRegex(ValueError, "數量"):
            self.parse(data)

    def test_rejects_changed_document_and_nonfinite_numbers(self):
        data = self.fixture()
        data[-1][3] = 1
        with self.assertRaisesRegex(ValueError, "DBMOD"):
            self.parse(data)
        data = self.fixture()
        data[1][1][-1][1][0] = float("nan")
        with self.assertRaisesRegex(ValueError, "非有限"):
            self.parse(data)

    def test_rejects_missing_handle(self):
        data = self.fixture()
        data[1][1] = [[0, "LINE"]]
        with self.assertRaisesRegex(ValueError, "有效 Handle"):
            self.parse(data)


if __name__ == "__main__":
    unittest.main()
