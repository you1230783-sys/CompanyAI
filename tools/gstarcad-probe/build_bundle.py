"""用已下載且鎖定SHA256的官方輸入離線封裝，兩種位元數分開自檢。"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def build(inputs, output):
    source = Path(__file__).resolve().parent
    lock = json.loads((source / "dependencies.json").read_text(encoding="utf-8"))
    output.parent.mkdir(parents=True, exist_ok=True)
    # 每次另建目錄，不遞迴刪除既有工具、使用者報告或CompanyAI runtime。
    stage = Path(tempfile.mkdtemp(prefix="cad-probe-", dir=output.parent))
    for name in ["probe.py", "cad.py", "registry.py", "build_bundle.py", "dependencies.json", "README.md", "Start-x64.cmd", "Start-x86.cmd"]:
        shutil.copyfile(source / name, stage / name)
    archive_inputs = stage / "rebuild-inputs"
    archive_inputs.mkdir()
    for item in lock["inputs"]:
        archive = inputs / item["name"]
        if digest(archive) != item["sha256"]:
            raise ValueError("依賴雜湊不符：" + item["name"])
        shutil.copyfile(archive, archive_inputs / archive.name)
        runtime = stage / ("runtime-" + item["arch"])
        destination = runtime if item["kind"] == "python" else runtime / "Lib" / "site-packages"
        destination.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(archive) as package:
            for name in package.namelist():
                target = (destination / name).resolve()
                if not target.is_relative_to(destination.resolve()):
                    raise ValueError("依賴包含越界檔案")
            package.extractall(destination)
    checks = []
    for arch in ["x64", "x86"]:
        runtime = stage / ("runtime-" + arch)
        site = runtime / "Lib" / "site-packages"
        for name in ["pythoncom313.dll", "pywintypes313.dll"]:
            shutil.copyfile(site / "pywin32_system32" / name, runtime / name)
        (runtime / "python313._pth").write_text(
            "python313.zip\n.\n..\nLib/site-packages\nLib/site-packages/win32\nLib/site-packages/win32/lib\nLib/site-packages/Pythonwin\nimport site\n", encoding="ascii")
        process = subprocess.run([str(runtime / "python.exe"), "-I", str(stage / "probe.py"), "--self-check"],
                                 text=True, encoding="utf-8", capture_output=True, timeout=60,
                                 creationflags=subprocess.CREATE_NO_WINDOW)
        if process.returncode:
            raise RuntimeError(arch + " runtime自檢失敗：" + process.stderr + process.stdout)
        checks.append(json.loads(process.stdout))
    report = dict(result="PASS_LOCAL_SELF_CHECK", actual_cad_tested=False,
                  tool_version="0.1.1", runtime_checks=checks, binary_origin=lock["binary_origin"])
    (stage / "local-verification.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    files = {str(p.relative_to(stage)).replace("\\", "/"): digest(p) for p in stage.rglob("*") if p.is_file()}
    (stage / "files-sha256.json").write_text(json.dumps(files, indent=2), encoding="utf-8")
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as bundle:
        for path in stage.rglob("*"):
            if path.is_file():
                bundle.write(path, "GstarCAD-Probe/" + str(path.relative_to(stage)))
    print(json.dumps(dict(bundle=str(output), sha256=digest(output), stage=str(stage), **report), ensure_ascii=False))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    build(args.inputs.resolve(), args.output.resolve())
