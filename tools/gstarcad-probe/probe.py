"""浩辰CAD獨立測試入口。父程序監控逾時，只終止自己的Python，不終止CAD。"""
import argparse
import datetime
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import uuid

sys.dont_write_bytecode = True
BASE = Path(__file__).resolve().parent


def write_json(path, data):
    temporary = path.with_suffix(".pending.json")
    temporary.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
    os.replace(temporary, path)


def worker(request_path):
    request = json.loads(Path(request_path).read_text(encoding="utf-8"))
    report_path = Path(request["report"])
    report = dict(tool_version="0.1.1", status="RUNNING", mode=request["mode"],
                  selected=request["selected"], python_bits=struct.calcsize("P") * 8,
                  python=sys.version, events=[], existing_documents_targeted=False)

    def update(step, **values):
        report.update(values)
        report["events"].append(dict(step=step, at=datetime.datetime.now().astimezone().isoformat()))
        write_json(report_path, report)
    try:
        import cad
        cad.run(request, update)
        return 0
    except Exception as error:
        update("failed", status="FAILED", error_type=type(error).__name__, error=str(error),
               hresult=getattr(error, "hresult", None),
               advice="不自動重試。若已建立測試文件，請先檢查；工具只針對新建圖面；測試圖面可能部分完成。")
        return 1


def execute(selected, mode, launch, root):
    folder = root / (datetime.datetime.now().strftime("%Y%m%d_%H%M%S_") + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True, exist_ok=False)
    report_path = folder / "report.json"
    request = dict(selected=selected, mode=mode, launch=launch,
                   output=str(folder / "gstarcad_test.dwg"), report=str(report_path))
    request_path = folder / "request.json"
    write_json(request_path, request)
    with (folder / "worker.log").open("w", encoding="utf-8") as log:
        process = subprocess.Popen([sys.executable, "-I", str(BASE / "probe.py"), "--worker", str(request_path)],
            stdout=log, stderr=log, creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            process.wait(timeout=240)
        except (subprocess.TimeoutExpired, KeyboardInterrupt):
            process.kill()
            process.wait()
            current = json.loads(report_path.read_text(encoding="utf-8")) if report_path.exists() else {}
            current.update(status="UNKNOWN_INTERRUPTED", mode=mode, selected=selected,
                           advice="僅停止本工具Python程序；CAD操作可能仍在進行，請先人工檢查，不自動重送或關閉CAD。")
            write_json(report_path, current)
    if not report_path.exists():
        write_json(report_path, dict(status="FAILED_TO_START", selected=selected,
                                     advice="請一併提供worker.log，以確認runtime或COM載入問題。"))
    report = json.loads(report_path.read_text(encoding="utf-8"))
    print("結果：", report["status"])
    print("報告：", report_path)
    if report.get("error"):
        print("錯誤：", report["error"])
    return 0 if report["status"].startswith("PASS_") else 1


def self_check():
    """只用合成資料核對排序與幾何驗收，不連接CAD。"""
    import pythoncom
    import win32api
    import cad
    import registry
    assert pythoncom.IID_IDispatch
    assert win32api.GetCurrentProcessId() > 0
    rows = [dict(progid=f"GStarCAD.Application.{year - 2000}", year=year, registry_bits=64)
            for year in [2008, 2022, 2026, 2027]]
    assert [r["year"] for r in sorted(rows, key=registry.preference, reverse=True)] == [2027, 2026, 2022, 2008]
    assert registry.year_from("GStarCAD.Application.27") == 0
    assert registry.year_from(r"C:\Program Files\GstarCAD2022\gcad.exe") == 2022
    assert registry.executable('"C:\\CAD 2022\\gcad.exe" /automation') == r"C:\CAD 2022\gcad.exe"
    # 使用去識別的實機身份資料驗證判斷；不連接 CAD 或聲稱繪圖已成功。
    path = r"C:\CAD\佑達ICAD 2008 專業版\icad.exe"
    selected = dict(progid="Icad.Application.1", clsid="{8011C111-7E36-11D1-BC5A-0060089608A4}",
                    registry_bits=32, year=2008, server=path)
    identity = dict(Name="ICAD", Version="5.0.20.0", FullName=path)
    assert cad.identify_product(selected, identity) == "yoda_icad_2008"
    for field, value in [("Name", "Other CAD"), ("Version", "6.0"), ("FullName", "unavailable")]:
        assert cad.identify_product(selected, dict(identity, **{field: value})) == "unknown"
    for field, value in [("clsid", "other"), ("progid", "Icad.Application.2"), ("year", 2022)]:
        assert cad.identify_product(dict(selected, **{field: value}), identity) == "unknown"
    try:
        cad.identify_product(selected, dict(identity, FullName=r"C:\Other\icad.exe"))
    except RuntimeError:
        pass
    else:
        raise AssertionError("不同程式路徑未拒絕")
    assert cad.identify_product(dict(server=r"C:\CAD\gcad.exe"),
                                dict(Name="GstarCAD", FullName=r"C:\CAD\gcad.exe")) == "gstarcad"
    corners = [(0, 0, 0), (200, 0, 0), (200, 100, 0), (0, 100, 0)]
    shapes = [dict(kind="line", start=corners[i], end=corners[(i + 1) % 4]) for i in range(4)]
    shapes += [dict(kind="circle", center=(x, y, 0), radius=5) for x, y in [(15, 15), (185, 15), (185, 85), (15, 85)]]
    cad.verify_geometry(shapes, False)
    dims = [dict(kind="dimension", measurement=m) for m in [200, 100, 10]]
    cad.verify_geometry(shapes + dims, True)
    for invalid in [shapes[:-1], shapes + [dict(kind="unexpected")], shapes + dims[:-1]]:
        try:
            cad.verify_geometry(invalid, True)
        except AssertionError:
            pass
        else:
            raise AssertionError("異常幾何未拒絕")
    print(json.dumps(dict(result="PASS_LOCAL_SELF_CHECK", bits=struct.calcsize("P") * 8,
                         cad_tested=False, checks=["pywin32 imports", "newest-year ordering", "no inferred year from ProgID",
                                                  "quoted executable path", "geometry and dimension verification", "invalid geometry rejected", "reported ICAD identity accepted",
                                                  "unknown identities and mismatched paths rejected", "GstarCAD identity retained"])))
    return 0


def main():
    for stream in [sys.stdout, sys.stderr]:
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker", help=argparse.SUPPRESS)
    parser.add_argument("--self-check", action="store_true")
    parser.add_argument("--inventory", action="store_true", help="只輸出COM註冊清單，不連接CAD")
    args = parser.parse_args()
    if args.worker:
        return worker(args.worker)
    if args.self_check:
        return self_check()
    import registry
    candidates, errors = registry.discover()
    root = BASE / "results"
    root.mkdir(exist_ok=True)
    stamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S_") + uuid.uuid4().hex[:8]
    inventory = root / ("inventory_" + stamp + ".json")
    write_json(inventory, dict(tool_version="0.1.1", python_bits=struct.calcsize("P") * 8,
                              candidates=candidates, registry_errors=errors,
                              actual_cad_tested=False, default="newest confirmed year, then internal version"))
    print("CAD 相容性測試 0.1.1（浩辰／佑達 ICAD 2008；新版優先）")
    print("目前Python位元數：", struct.calcsize("P") * 8)
    print("註冊清單：", inventory)
    if args.inventory:
        return 0
    # 遺留的 COM 登錄仍保存在 inventory；預設清單只使用可確認存在的伺服器。
    candidates = [item for item in candidates if item["clsid"] and item["server_exists"]]
    if not candidates:
        print("找不到對應程式仍存在的候選COM註冊。請回傳inventory報告；此工具不自動修復登錄。")
        return 1
    for i, candidate in enumerate(candidates, 1):
        year = candidate["year"] or "年份未知"
        print(f"{i}. {year} | {candidate['progid']} | 登錄{candidate['registry_bits']}位元 | {candidate['server']}")
    answer = input("選擇版本編號（Enter 預設最新；q 離開）：").strip()
    if answer.lower() == "q":
        return 0
    index = int(answer or "1") - 1
    if not 0 <= index < len(candidates):
        raise ValueError("版本編號超出清單")
    selected = candidates[index]
    print("1. 唯讀連線診斷（不自動啟動CAD）")
    print("2. 新建測試DWG：200×100 mm矩形、四個直徑10 mm孔位圓")
    print("3. 新建相同測試DWG，增加200、100及直徑10的尺寸標註")
    mode = {"1": "diagnose", "2": "drawing", "3": "dimensions"}.get(input("模式（Enter 預設1）：").strip() or "1")
    if not mode:
        raise ValueError("模式無效")
    launch = False
    if mode != "diagnose":
        print("只新建測試圖面；成功後另存並重開。請讓所選CAD停在可操作狀態。")
        if input("確認新建測試文件？輸入 YES：").strip() != "YES":
            return 0
        launch = input("若未啟動，允許啟動此版本？輸入 YES，其餘為否：").strip() == "YES"
    return execute(selected, mode, launch, root)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (Exception, KeyboardInterrupt) as error:
        print("工具未完成：", str(error), file=sys.stderr)
        sys.exit(1)
