"""在全新空白 CAD 文件建立批次效能測試圖；保留文件並另存新路徑，不覆寫。

建立時間不併入匯出效能。所有幾何由固定 LISP 建立，沒有模型產生的程式。
"""
import argparse
import json
from pathlib import Path
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--lines", type=int, default=20000)
    args = parser.parse_args()
    if not 1 <= args.lines <= 50000:
        raise ValueError("合成直線數量需為 1–50000")
    sys.path.insert(0, (ROOT / ".build/cad-runtime-stage.txt").read_text().strip())
    import pythoncom
    import worker
    folder = ROOT / ".build" / ("cad-bulk-fixture-" + uuid.uuid4().hex)
    folder.mkdir()
    receipt = folder / "created.txt"
    output = folder / ("bulk-" + str(args.lines) + ".dwg")
    script = folder / "create.lsp"
    report = dict(status="RUNNING", version=args.version, lines=args.lines, path=str(output))
    pythoncom.CoInitialize()
    try:
        selected = next(v for v in worker.registry.discover()[0] if v["progid"] == args.version)
        app, _ = worker.connect_application(selected, dict(action="active_snapshot"), {})
        if int(app.Documents.Count) and int(app.ActiveDocument.GetVariable("CMDACTIVE")) != 0:
            raise ValueError("CAD 正在執行其他指令")
        doc = app.Documents.Add()
        report["owned_document"] = str(doc.Name)
        if int(doc.ModelSpace.Count) != 0:
            raise ValueError("新建範本並非空白，不執行測試")
        doc.SetVariable("INSUNITS", 4)
        source = (ROOT / "tools/cad_bulk/fixture.lsp").read_text(encoding="utf-8")
        source = source.replace("@@DOCUMENT@@", str(doc.GetVariable("DWGNAME"))).replace(
            "@@COUNT@@", str(args.lines)).replace("@@RECEIPT@@", receipt.as_posix())
        script.write_text(source, encoding="cp950")
        started = time.perf_counter()
        print(json.dumps(dict(folder=str(folder), step="create", document=str(doc.Name))), flush=True)
        if args.version == "Icad.Application.1":
            app.LoadLISP(str(script))
        else:
            doc.SendCommand('(load "' + script.as_posix() + '")\n')
        deadline = time.monotonic() + 180
        while not receipt.is_file() or b"COMPLETE" not in receipt.read_bytes():
            if time.monotonic() >= deadline:
                raise TimeoutError("合成圖未回傳完成標記；保留部分文件，不重畫")
            pythoncom.PumpWaitingMessages()
            time.sleep(.1)
        if app.ActiveDocument._oleobj_ != doc._oleobj_ or int(doc.ModelSpace.Count) != args.lines + 51:
            raise ValueError("建立後文件或數量不符")
        report["create_seconds"] = time.perf_counter() - started
        doc.SaveAs(str(output), 9) if args.version == "Icad.Application.1" else doc.SaveAs(str(output))
        if not output.is_file() or not output.stat().st_size:
            raise ValueError("合成 DWG 未產生")
        report.update(status="PASS", entity_count=int(doc.ModelSpace.Count), name=str(doc.Name))
        print(json.dumps(report, ensure_ascii=True), flush=True)
    except Exception as exc:
        report.update(status="FAILED", error=str(exc))
        raise
    finally:
        (folder / "fixture-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        pythoncom.CoUninitialize()


if __name__ == "__main__":
    main()
