"""CAD 內部批次匯出效能原型：只讀已知合成圖面，輸出不交給模型。

執行範例（使用鎖定的 CAD Python）：
  python.exe -I -B tools/cad_bulk_probe.py --version Icad.Application.1 --name Drawing5
本工具尚未接入產品；完整原生屬性與寫入 revision 核對仍須另行整合。
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]


def check_fixture(rows, line_count):
    """逐一核對大型合成圖的全部幾何，驗證精度與數量，不透過慢速 COM 重讀。"""
    lines, circles, texts = set(), set(), []
    largest_error = 0.0

    def close(actual, expected):
        nonlocal largest_error
        error = abs(actual - expected)
        largest_error = max(largest_error, error)
        if not math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-10):
            raise ValueError("大型合成圖的幾何或數值精度不符")

    for pairs in rows.values():
        fields = dict(pairs)
        kind = fields[0]
        if kind == "LINE":
            x, y, z = fields[10]
            column, row = round(x / 2.5), round(y / 2.5)
            index = row * 200 + column
            if not 0 <= column < 200 or not 0 <= index < line_count or index in lines:
                raise ValueError("大型合成圖的直線索引重複或超出範圍")
            lines.add(index)
            for actual, expected in zip(fields[10], [column*2.5, row*2.5, 0.]):
                close(actual, expected)
            for actual, expected in zip(fields[11], [x+.1234567890123456, y+1.2345678901234567, 0.]):
                close(actual, expected)
        elif kind == "CIRCLE":
            index = round((fields[10][0] - 500.) / 20.)
            if not 0 <= index < 50 or index in circles:
                raise ValueError("大型合成圖的圓索引不符")
            circles.add(index)
            for actual, expected in zip(fields[10], [500.+index*20., 500., 0.]):
                close(actual, expected)
            close(fields[40], 1.23456789012345)
        elif kind == "TEXT":
            texts.append(fields[1])
        else:
            raise ValueError("大型合成圖包含非預期圖元")
    if len(lines) != line_count or len(circles) != 50 or texts != ["批次匯出測試 / BULK READ"]:
        raise ValueError("大型合成圖的完整數量或中文字不符")
    return dict(all_entities_verified=len(rows), lines=len(lines), circles=len(circles),
                chinese_text_verified=True, max_absolute_error=largest_error)


def check_sample(doc, rows, api):
    """以獨立 COM 詳讀抽樣確認數值、中文字及線型；不把抽樣稱為全圖屬性驗收。"""
    import cad_existing
    groups = {}
    for handle, pairs in rows.items():
        kind = next(value for code, value in pairs if code == 0)
        groups.setdefault(kind, []).append(handle)
    selected = []
    for kind, handles in groups.items():
        # 線段取前／中／末段，非線段每種最多三個，包含文字及註解類型。
        indices = {0, len(handles)//2, len(handles)-1}
        selected.extend(handles[i] for i in sorted(indices))
    checks, unavailable = [], []
    for handle in selected:
        entity = doc.HandleToObject(handle)
        actual = cad_existing.describe(entity, api)
        pairs = rows[handle]
        fields = dict(pairs)
        kind = fields[0]
        expected = dict(Layer=fields.get(8, "0"), Color=fields.get(62, 256),
                        Linetype=fields.get(6, "ByLayer"), LinetypeScale=fields.get(48, 1.0))
        if kind in ("LINE", "CIRCLE", "ARC", "TEXT", "MTEXT"):
            expected["Normal"] = fields.get(210, [0., 0., 1.])
        if kind == "LINE":
            expected.update(StartPoint=fields[10], EndPoint=fields[11])
        if kind in ("CIRCLE", "ARC"):
            # 原型尚未轉換 OCS；傾斜圖元不得冒充已完成 WCS 核對。
            if expected["Normal"] != [0., 0., 1.]:
                raise ValueError("抽樣包含傾斜 OCS 圓／弧；原型尚未支援轉換")
            expected.update(Center=fields[10], Radius=fields[40])
        if kind == "ARC":
            expected.update(StartAngle=fields[50], EndAngle=fields[51])
        if kind in ("TEXT", "MTEXT"):
            text = "".join(value for code, value in pairs if code == 3) + fields.get(1, "") if kind == "MTEXT" else fields.get(1, "")
            expected.update(TextString=text, Height=fields[40], StyleName=fields.get(7, "STANDARD"))
        if kind == "DIMENSION" and 42 in fields:
            expected["Measurement"] = fields[42]
        if kind == "HATCH":
            for key, code in (("PatternName", 2), ("PatternScale", 41)):
                value = cad_existing.optional(entity, key)
                if value is None:
                    unavailable.append(dict(handle=handle, field=key))
                elif not api.equal(value, fields[code]):
                    raise ValueError(f"剖面屬性不符：{handle}.{key}")
                else:
                    checks.append(dict(handle=handle, field=key))
        for key, value in expected.items():
            if key not in actual["properties"]:
                unavailable.append(dict(handle=handle, field=key))
                continue
            compare = api.text_equal if key == "TextString" else api.equal
            if not compare(actual["properties"][key], value):
                raise ValueError(f"批次資料與 COM 不符：{handle}.{key}")
            checks.append(dict(handle=handle, field=key))
    return dict(sampled_entities=len(selected), verified_properties=len(checks),
                unavailable_properties=unavailable, checks=checks)


def parse_export(path, nonce, encoding):
    """完整結尾、nonce、Handle 唯一性均符合才接受；不讀取部分結果。"""
    raw = path.read_bytes()
    if len(raw) > 128 * 1024 * 1024:
        raise ValueError("批次匯出超過原型上限 128 MiB")
    def reject_constant(value):
        raise ValueError("匯出含非有限數值：" + value)
    lines = [json.loads(line, parse_constant=reject_constant) for line in raw.decode(encoding).splitlines() if line]
    if not lines or not isinstance(lines[0], list) or len(lines[0]) != 7 or lines[0][:3] != ["HEADER", 1, nonce]:
        raise ValueError("批次匯出標頭不符")
    if not isinstance(lines[-1], list) or len(lines[-1]) != 4 or lines[-1][:2] != ["COMPLETE", nonce]:
        raise ValueError("缺少完整結尾；不接受部分結果")
    rows = lines[1:-1]
    if any(not isinstance(row, list) or len(row) != 2 or row[0] != "ENTITY"
           or not isinstance(row[1], list) for row in rows) or lines[-1][2] != len(rows):
        raise ValueError("批次匯出數量不符")
    handles = []
    for row in rows:
        if any(not isinstance(pair, list) or len(pair) != 2 or type(pair[0]) is not int for pair in row[1]):
            raise ValueError("批次匯出群組格式不符")
        values = [value for code, value in row[1] if code == 5]
        if len(values) != 1 or not isinstance(values[0], str) or not values[0] or any(
                c not in "0123456789ABCDEFabcdef" for c in values[0]):
            raise ValueError("批次匯出缺少唯一有效 Handle")
        handles.append(values[0].upper())
    if len(set(handles)) != len(handles):
        raise ValueError("批次匯出有重複 Handle")
    if lines[0][6] != lines[-1][3]:
        raise ValueError("匯出期間 DBMOD 改變")
    return lines[0], {handle: row[1] for handle, row in zip(handles, rows)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--name", required=True)
    parser.add_argument("--encoding", default="cp950")
    parser.add_argument("--compare-handles", action="store_true")
    parser.add_argument("--check-sample", action="store_true")
    parser.add_argument("--check-fixture-lines", type=int)
    parser.add_argument("--open-test-path", type=Path,
                        help="只允許 .build 內 DWG；以唯讀方式開啟同一合成檔比較不同 CAD。")
    args = parser.parse_args()
    stage = Path((ROOT / ".build/cad-runtime-stage.txt").read_text().strip())
    sys.path.insert(0, str(stage))
    import pythoncom
    import worker
    import cad_existing
    folder = ROOT / ".build" / ("cad-bulk-" + uuid.uuid4().hex)
    folder.mkdir()
    nonce = uuid.uuid4().hex
    output = folder / "entities.jsonl"
    script = folder / "export.lsp"
    source = (ROOT / "tools/cad_bulk/export.lsp").read_text(encoding="utf-8")
    # 原型路徑及 nonce 均由工具產生；不能從模型傳入程式、輸出位置或任意 LISP。
    source = source.replace("@@OUTPUT@@", output.as_posix()).replace("@@NONCE@@", nonce)
    script.write_text(source, encoding="cp950")
    report = dict(status="RUNNING", version=args.version, document=args.name,
                  folder=str(folder), source_sha256=hashlib.sha256(script.read_bytes()).hexdigest())
    report_path = folder / "report.json"

    def record(**values):
        report.update(values)
        report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    pythoncom.CoInitialize()
    try:
        selected = next(v for v in worker.registry.discover()[0] if v["progid"] == args.version)
        app, application = worker.connect_application(selected, dict(action="active_snapshot"), {})
        if args.open_test_path:
            test_path = args.open_test_path.resolve(strict=True)
            if not test_path.is_relative_to(ROOT / ".build") or test_path.suffix.lower() != ".dwg":
                raise ValueError("只允許開啟 .build 內合成 DWG")
            if test_path.name != args.name:
                raise ValueError("測試路徑與文件名稱不符")
            existing = [d for d in cad_existing.iter_entities(app.Documents)
                        if str(d.FullName or "") and Path(str(d.FullName)).resolve() == test_path]
            if not existing:
                app.Documents.Open(str(test_path), True)
        docs = [d for d in cad_existing.iter_entities(app.Documents)
                if str(d.Name) == args.name or Path(str(d.Name)).name == args.name]
        if len(docs) != 1:
            raise ValueError("指定合成文件不唯一或不存在")
        doc = docs[0]
        filename = str(doc.FullName or "")
        if not ((args.name == "Drawing5" and not filename)
                or Path(filename).resolve().is_relative_to(ROOT / ".build")):
            raise ValueError("原型只允許已知合成 Drawing5 或專案 .build 內圖檔")
        doc.Activate()
        if int(doc.GetVariable("CMDACTIVE")) != 0:
            raise ValueError("CAD 正在執行其他指令")
        before = dict(name=str(doc.Name), full_name=filename, count=int(doc.ModelSpace.Count),
                      units=doc.GetVariable("INSUNITS"), dbmod=doc.GetVariable("DBMOD"))
        drawing_hash = hashlib.sha256(Path(filename).read_bytes()).hexdigest() if filename else None
        record(step="export", before=before, application=application)
        print(json.dumps(dict(folder=str(folder), step="export")), flush=True)
        started = time.perf_counter()
        if args.version == "Icad.Application.1":
            app.LoadLISP(str(script))
        else:
            doc.SendCommand('(load "' + script.as_posix() + '")\n')
        call_seconds = time.perf_counter() - started
        deadline = time.monotonic() + 60
        while not output.is_file() or b'"COMPLETE"' not in output.read_bytes()[-200:]:
            if time.monotonic() >= deadline:
                raise TimeoutError("內部匯出沒有完成標記；不自動重送")
            pythoncom.PumpWaitingMessages()
            time.sleep(.05)
        export_seconds = time.perf_counter() - started
        parsing = time.perf_counter()
        header, rows = parse_export(output, nonce, args.encoding)
        parse_seconds = time.perf_counter() - parsing
        after = dict(name=str(doc.Name), full_name=str(doc.FullName or ""), count=int(doc.ModelSpace.Count),
                     units=doc.GetVariable("INSUNITS"), dbmod=doc.GetVariable("DBMOD"))
        if before != after or app.ActiveDocument._oleobj_ != doc._oleobj_:
            raise ValueError("讀取前後的文件／數量／單位／DBMOD 不符")
        if filename and (Path(header[4]) / header[3]).resolve() != Path(filename).resolve():
            raise ValueError("CAD 內部匯出的文件路徑不符")
        after_hash = hashlib.sha256(Path(filename).read_bytes()).hexdigest() if filename else None
        if drawing_hash != after_hash:
            raise ValueError("唯讀測試期間來源 DWG 檔案已變更")
        record(step="exported", export_seconds=export_seconds, call_seconds=call_seconds,
               parse_seconds=parse_seconds, exported_entities=len(rows), after=after,
               header=header, output_bytes=output.stat().st_size, drawing_sha256_before=drawing_hash,
               drawing_sha256_after=after_hash)
        if args.compare_handles:
            started = time.perf_counter()
            handles = [str(e.Handle).upper() for e in cad_existing.iter_entities(doc.ModelSpace)]
            record(com_handle_seconds=time.perf_counter()-started, com_handle_count=len(handles),
                   only_bulk=sorted(set(rows)-set(handles)), only_com=sorted(set(handles)-set(rows)))
            if len(set(handles)) != len(handles) or set(handles) != set(rows):
                raise ValueError("COM 與批次匯出的 Handle 集合不符")
        if args.check_sample:
            started = time.perf_counter()
            record(sample_verification=check_sample(doc, rows, worker),
                   sample_seconds=time.perf_counter()-started)
        if args.check_fixture_lines is not None:
            record(fixture_verification=check_fixture(rows, args.check_fixture_lines))
        record(status="PASS", step="complete")
        print(json.dumps(report, ensure_ascii=True), flush=True)
    except Exception as exc:
        record(status="FAILED", error=str(exc))
        raise
    finally:
        pythoncom.CoUninitialize()


if __name__ == "__main__":
    main()
