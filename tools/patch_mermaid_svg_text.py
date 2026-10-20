"""核對／套用 Mermaid 12.2.0 的單字元 SVG 純文字相容修補。

上游 nonMarkdownToLines 的分詞規則會丟掉沒有配對標籤的「>」，
使 value >= 10 被畫成 value = 10。允許純文字單字保留 >；SVG 標籤
仍由原本分支辨識，安全層級、DOMPurify 與匯出淨化規則均不變。

不用重新下載或重建套件。只接受固定的上游／修補後 SHA256，且反向
核對必須還原上游位元組；套用請用 --apply，日常完整性檢查用 --check。
上游來源：packages/mermaid/src/rendering-util/handle-markdown-text.ts
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "ui/vendor/mermaid.min.js"
MANIFEST = ROOT / "ui/vendor/packages.json"
UPSTREAM_SHA256 = "a8a6a6f805b074a8261a2825c86f99c8054acb3b35f50bb7e24a4f9f2c3c3b9f"
PATCHED_SHA256 = "b8ec9fa298fc84a28dd06649e1b3fcf16c0c4ba85afbec51e5f2774879c2e470"
OLD = rb"<[^>]+>|[^\s<>]+"
NEW = rb"<[^>]+>|[^\s<]+"
PATCH_ID = "svg-plain-text-greater-than-v1"


def digest(data: bytes) -> str:
    """一律核對原始位元組，避免換行轉換改壞已固定的上游檔案。"""
    return hashlib.sha256(data).hexdigest()


def atomic_write(path: Path, data: bytes) -> None:
    """在同一目錄寫完並同步後才替換；不移動或刪除其他檔案。"""
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix="mermaid-patch-", delete=False) as file:
            temporary = Path(file.name)
            file.write(data)
            file.flush()
            os.fsync(file.fileno())
        os.replace(temporary, path)
    finally:
        if temporary is not None and temporary.exists():
            temporary.unlink()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--apply", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args()
    original = BUNDLE.read_bytes()
    current = digest(original)
    if current not in {UPSTREAM_SHA256, PATCHED_SHA256}:
        raise SystemExit("Mermaid 不是已核准的固定版本；停止，請先審閱新的上游來源。")
    patched = original
    if current == UPSTREAM_SHA256:
        if not args.apply:
            raise SystemExit("Mermaid 尚未套用 SVG 比較符號修補。")
        if original.count(OLD) != 1:
            raise SystemExit("分詞規則不再唯一；拒絕猜測修補位置。")
        patched = original.replace(OLD, NEW, 1)
    if digest(patched) != PATCHED_SHA256 or patched.count(NEW) != 1:
        raise SystemExit("修補後位元組不符預期。")
    if digest(patched.replace(NEW, OLD, 1)) != UPSTREAM_SHA256:
        raise SystemExit("反向核對無法還原上游；停止。")

    packages = json.loads(MANIFEST.read_text(encoding="utf-8-sig"))
    package = next(item for item in packages if item["name"] == "mermaid")
    if package["version"] != "12.2.0":
        raise SystemExit("套件版本已變更；不可套用舊版修補。")
    if args.apply:
        package["bundled_file_sha256"] = PATCHED_SHA256
        package["upstream_bundled_file_sha256"] = UPSTREAM_SHA256
        package["local_patches"] = [PATCH_ID]
        atomic_write(BUNDLE, patched)
        atomic_write(MANIFEST, (json.dumps(packages, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))
    elif (
        package.get("bundled_file_sha256") != PATCHED_SHA256
        or package.get("upstream_bundled_file_sha256") != UPSTREAM_SHA256
        or package.get("local_patches") != [PATCH_ID]
    ):
        raise SystemExit("Mermaid 修補與套件清單不一致。")
    print("PASS Mermaid SVG text patch: exact one-byte change and upstream reverse hash verified.")


if __name__ == "__main__":
    main()
