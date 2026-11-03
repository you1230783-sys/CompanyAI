"""從實際Git歷史重建一份已發行的舊CAD環境，只供隔離安裝測試使用。

不下載網路內容；不重造manifest或放寬雜湊，確保測的是正式清理入口。
"""
import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def create(destination):
    # 只准寫入腳本建立的隔離安裝位置，不能用這個測試工具寫正式安裝目錄。
    destination = Path(destination).resolve()
    if destination != Path(r"C:\largan\LM_AI_Installer_Test").resolve():
        raise ValueError("Only the isolated installer test directory is accepted")
    current = (ROOT / "assets/cad-runtime-sha256.txt").read_text().strip()
    current_root = ROOT / "dist" / f"cad-{current[:16]}"
    trusted = set(json.loads((ROOT / "assets/cad-runtime-history.json").read_text()))
    commits = git("log", "--format=%H", "--", "assets/cad-runtime-manifest.json").decode().splitlines()
    mappings = {"worker.py": "src/cad_worker.py", "cad.py": "tools/gstarcad-probe/cad.py", "registry.py": "tools/gstarcad-probe/registry.py"}
    for commit in commits:
        raw = git("show", f"{commit}:assets/cad-runtime-manifest.json")
        fingerprint = sha(raw)
        if fingerprint == current or fingerprint not in trusted:
            continue
        entries = json.loads(raw)
        contents = {}
        for name, expected in entries.items():
            path = PurePosixPath(name)
            if path.is_absolute() or ".." in path.parts or ":" in name or "\\" in name:
                raise ValueError("Unsafe historical manifest path")
            existing = current_root / name
            data = existing.read_bytes() if existing.is_file() else b""
            if sha(data) != expected:
                source = mappings.get(name, f"src/{name}" if len(path.parts) == 1 else None)
                if source is None:
                    break
                data = git("show", f"{commit}:{source}")
            if sha(data) != expected:
                break
            contents[name] = data
        else:
            output = destination / f"cad-{fingerprint[:16]}"
            output.mkdir()  # 不覆寫上次測試或使用者的任何既有內容。
            for name, data in contents.items():
                path = output / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            (output / "manifest.json").write_bytes(raw)
            print(json.dumps({"directory": str(output), "commit": commit, "sha256": fingerprint, "files": len(contents)}))
            return
    raise RuntimeError("No fully verifiable historical CAD runtime could be reconstructed")


if __name__ == "__main__":
    create(sys.argv[1])
