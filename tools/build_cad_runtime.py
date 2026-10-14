"""從既有鎖定的官方下載建立 CAD 專用 runtime，不變動分析用 Python。"""
import hashlib
import json
from pathlib import Path
import shutil
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    lock = json.loads((ROOT / "tools/gstarcad-probe/dependencies.json").read_text(encoding="utf-8"))
    stage = ROOT / ".build" / ("cad-runtime-" + uuid.uuid4().hex)
    stage.mkdir()
    runtime = stage / "runtime"
    for item in lock["inputs"]:
        if item["arch"] != "x64":
            continue
        archive = ROOT / ".build/cad-probe-downloads" / item["name"]
        if not archive.is_file():
            # 原獨立工具 ZIP 已包含官方輸入，離線重建可直接沿用，不需 pip 或下載。
            archive.parent.mkdir(parents=True, exist_ok=True)
            with zipfile.ZipFile(ROOT / "offline/GstarCAD-Probe.zip") as bundle:
                archive.write_bytes(bundle.read("GstarCAD-Probe/rebuild-inputs/" + item["name"]))
        if digest(archive) != item["sha256"]:
            raise ValueError("CAD 輸入檔 SHA256 不符：" + item["name"])
        destination = runtime if item["kind"] == "python" else runtime / "Lib/site-packages"
        destination.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(archive) as package:
            for name in package.namelist():
                if not (destination / name).resolve().is_relative_to(destination.resolve()):
                    raise ValueError("壓縮檔路徑越界")
            package.extractall(destination)
    for name in ("pythoncom313.dll", "pywintypes313.dll"):
        shutil.copyfile(runtime / "Lib/site-packages/pywin32_system32" / name, runtime / name)
    (runtime / "python313._pth").write_text(
        "python313.zip\n.\n..\nLib/site-packages\nLib/site-packages/win32\nLib/site-packages/win32/lib\nLib/site-packages/Pythonwin\nimport site\n", encoding="ascii")
    shutil.copyfile(ROOT / "src/cad_worker.py", stage / "worker.py")
    shutil.copyfile(ROOT / "src/cad_existing.py", stage / "cad_existing.py")
    shutil.copyfile(ROOT / "src/cad_active.py", stage / "cad_active.py")
    for name in ("cad.py", "registry.py"):
        shutil.copyfile(ROOT / "tools/gstarcad-probe" / name, stage / name)
    files = {p.relative_to(stage).as_posix(): digest(p) for p in sorted(stage.rglob("*")) if p.is_file()}
    manifest = json.dumps(files, indent=2).encode("utf-8")
    fingerprint = hashlib.sha256(manifest).hexdigest()
    (stage / "manifest.json").write_bytes(manifest)
    (ROOT / "assets/cad-runtime-manifest.json").write_bytes(manifest)
    (ROOT / "assets/cad-runtime-sha256.txt").write_text(fingerprint + "\n", encoding="ascii")
    # 記錄本次 stage；後續腳本仍逐檔核對，不信任此路徑就跳過完整性檢查。
    (ROOT / ".build/cad-runtime-stage.txt").write_text(str(stage), encoding="utf-8")
    print(stage)


if __name__ == "__main__":
    main()
