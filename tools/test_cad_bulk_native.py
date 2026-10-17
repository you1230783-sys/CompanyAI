"""明確指定 CAD 後才執行的原生回歸：子圖元、OCS、舊 revision 與不儲存修改。

只建立本次新圖及新路徑，最後只關閉本工具新建的文件；不操作使用者原圖。
使用鎖定 CAD runtime 的 python.exe -I -B 執行此檔 --version <ProgID>。
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = r'''
(defun lm-ai-edge-fixture (/ f)
 (entmake '((0 . "LWPOLYLINE") (8 . "0") (90 . 4) (70 . 1)
            (10 0.0 0.0) (10 100.0 0.0) (10 100.0 50.0) (10 0.0 50.0)))
 (entmake '((0 . "POLYLINE") (8 . "0") (66 . 1) (70 . 1) (10 0.0 0.0 5.0)))
 (entmake '((0 . "VERTEX") (8 . "0") (10 200.0 0.0 0.0)))
 (entmake '((0 . "VERTEX") (8 . "0") (10 240.0 0.0 0.0)))
 (entmake '((0 . "VERTEX") (8 . "0") (10 240.0 30.0 0.0)))
 (entmake '((0 . "VERTEX") (8 . "0") (10 200.0 30.0 0.0)))
 (entmake '((0 . "SEQEND")))
 (entmake '((0 . "BLOCK") (2 . "LM_AI_EDGE_BLOCK") (70 . 2) (10 0.0 0.0 0.0)))
 (entmake '((0 . "LINE") (10 0.0 0.0 0.0) (11 5.0 0.0 0.0)))
 (entmake '((0 . "ATTDEF") (10 0.0 0.0 0.0) (40 . 2.5) (1 . "預設") (2 . "ID") (3 . "識別") (70 . 0)))
 (entmake '((0 . "ENDBLK")))
 (entmake '((0 . "INSERT") (2 . "LM_AI_EDGE_BLOCK") (10 150.0 100.0 0.0) (66 . 1)))
 (entmake '((0 . "ATTRIB") (10 150.0 100.0 0.0) (40 . 2.5) (1 . "中文-01") (2 . "ID") (70 . 0)))
 (entmake '((0 . "SEQEND")))
 (entmake '((0 . "CIRCLE") (10 20.0 30.0 0.0) (40 . 5.0) (210 0.0 1.0 0.0)))
 (entmake '((0 . "MTEXT") (10 0.0 120.0 0.0) (40 . 3.0) (41 . 100.0) (71 . 1)
            (1 . "測試 \"引號\" \\P第二行 \\\\路徑") (7 . "STANDARD")))
 (setq f (open "@@RECEIPT@@" "w")) (write-line "COMPLETE" f) (close f)
 (princ))
(lm-ai-edge-fixture)
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    sys.path.insert(0, (ROOT / ".build/cad-runtime-stage.txt").read_text().strip())
    import pythoncom
    import worker
    folder = ROOT / ".build" / ("cad-edge-069-" + uuid.uuid4().hex)
    folder.mkdir()
    script, receipt, path = folder/"fixture.lsp", folder/"created.txt", folder/"edge.dwg"
    report = dict(version=args.version, status="RUNNING", folder=str(folder))
    pythoncom.CoInitialize()
    doc = None
    worker.BULK_ENABLED = True
    try:
        selected = next(v for v in worker.registry.discover()[0] if v["progid"] == args.version)
        app, _ = worker.connect_application(selected, dict(action="active_snapshot"), {})
        if int(app.Documents.Count):
            assert int(app.ActiveDocument.GetVariable("CMDACTIVE")) == 0
        doc = app.Documents.Add()
        report["owned_document"] = str(doc.Name)
        assert int(doc.ModelSpace.Count) == 0
        doc.SetVariable("INSUNITS", 4)
        script.write_text(FIXTURE.replace("@@RECEIPT@@", receipt.as_posix()), encoding="cp950")
        if args.version == "Icad.Application.1":
            app.LoadLISP(str(script))
        else:
            doc.SendCommand('(load "' + script.as_posix() + '")\n')
        deadline = time.monotonic()+30
        while not receipt.is_file():
            if time.monotonic() > deadline:
                raise TimeoutError("合成圖未完成，保留現場，不重畫")
            pythoncom.PumpWaitingMessages()
            time.sleep(.1)
        assert int(doc.ModelSpace.Count) == (6 if args.version == "Icad.Application.1" else 5)
        doc.SaveAs(str(path), 9) if args.version == "Icad.Application.1" else doc.SaveAs(str(path))
        original_hash = hashlib.sha256(path.read_bytes()).hexdigest()
        state = {}
        state.update(worker.execute(dict(action="active_snapshot", version=args.version), state))
        import cad_bulk
        handles, no_rows = cad_bulk.snapshot(doc, worker, handles_only=True)
        assert len(handles) == int(doc.ModelSpace.Count) and not no_rows
        report["native_handle_audit_verified"] = True
        rows = state["entities"]
        assert len(rows) == (6 if args.version == "Icad.Application.1" else 5)
        polys = [r for r in rows if r["kind"] == "polyline"]
        assert len(polys) == 2 and all(r["geometry"]["closed"] for r in polys)
        assert sorted((len(r["geometry"]["vertices"]), r["geometry"]["vertices"][0][2]) for r in polys) == [(4,0.),(4,5.)]
        block = next(r for r in rows if r["kind"] == "blockreference")
        assert block["attributes"] == [dict(tag="ID", text="中文-01")]
        circle = next(r for r in rows if r["kind"] == "circle")
        entity = doc.HandleToObject(circle["handle"])
        assert circle["properties"]["Center"] == worker.coordinate(entity.Center)
        assert circle["properties"]["Normal"] == [0.,1.,0.]
        text = next(r for r in rows if r["kind"] == "mtext")["properties"]["TextString"]
        assert '"引號"' in text and "第二行" in text and "路徑" in text
        report.update(children_verified=True, tilted_ocs_com_fallback_verified=True, chinese_escapes_verified=True)
        entity.Radius = 6.0  # 在自己的圖面模擬同數量手動修改。
        rejection = {}
        try:
            worker.execute(dict(action="active_edit", version=args.version, document=state["document"],
                                revision=state["revision"], edits=[dict(handle=circle["handle"],kind="radius",value=7.0)], operations=[]), rejection)
        except ValueError as exc:
            assert "文件身分或內容已變更" in str(exc), exc
        else:
            raise AssertionError("舊 revision 未被拒絕")
        assert float(entity.Radius) == 6.0 and rejection["drawing_started"] is False
        report["stale_revision_rejected_without_edit"] = True
        if args.version != "Icad.Application.1":
            fresh = {}
            fresh.update(worker.execute(dict(action="active_snapshot", version=args.version), fresh))
            result = {}
            result.update(worker.execute(dict(action="active_edit", version=args.version, document=fresh["document"],
                revision=fresh["revision"], edits=[dict(handle=circle["handle"],kind="radius",value=7.0)],operations=[]),result))
            assert result["status"] == "EDITED_UNSAVED" and result["saved"] is False
            assert float(entity.Radius) == 7.0
            report["active_edit_unsaved_verified"] = True
        assert hashlib.sha256(path.read_bytes()).hexdigest() == original_hash
        doc.Close(False)
        doc = None
        report.update(status="PASS", source_file_unchanged=True)
        print(json.dumps(report, ensure_ascii=True))
    except Exception as exc:
        report.update(status="FAILED", error=str(exc))
        raise
    finally:
        # 失敗保留本次圖面供檢查；成功才關閉自己的文件。
        (folder/"report.json").write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding="utf-8")
        pythoncom.CoUninitialize()


if __name__ == "__main__":
    main()
