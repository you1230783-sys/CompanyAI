"""從既有發行提交整理CAD環境指紋，只記錄清單雜湊，不保存舊版程式碼。

本工具不連網、不編譯；CAD runtime更新後、封裝前執行一次。
清理器只信任編入EXE的這份名單，不信任任意cad-*資料夾自行聲稱的身分。
"""
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "assets/cad-runtime-history.json"


def git(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=ROOT)


def main():
    known = set()
    if DESTINATION.exists():
        known.update(json.loads(DESTINATION.read_text(encoding="utf-8-sig")))
    revisions = git("log", "--format=%H", "--", "assets/cad-runtime-manifest.json").decode().splitlines()
    for revision in revisions:
        raw = git("show", f"{revision}:assets/cad-runtime-manifest.json")
        expected = git("show", f"{revision}:assets/cad-runtime-sha256.txt").decode().strip()
        actual = hashlib.sha256(raw).hexdigest()
        if actual != expected:
            raise ValueError(f"Historical CAD manifest/hash mismatch: {revision}")
        known.add(actual)
    current = (ROOT / "assets/cad-runtime-manifest.json").read_bytes()
    expected = (ROOT / "assets/cad-runtime-sha256.txt").read_text().strip()
    if hashlib.sha256(current).hexdigest() != expected:
        raise ValueError("Current CAD manifest/hash mismatch")
    known.add(expected)
    if not all(len(value) == 64 and all(c in "0123456789abcdef" for c in value) for value in known):
        raise ValueError("Invalid CAD runtime fingerprint")
    DESTINATION.write_text(json.dumps(sorted(known), indent=2) + "\n", encoding="utf-8")
    print(f"Recorded {len(known)} trusted CAD runtime manifests.")


if __name__ == "__main__":
    main()
