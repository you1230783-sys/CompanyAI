"""彙整 localhost 實測紀錄；不呼叫模型，也不讀取測試資料夾外的檔案。"""
import argparse
import json
from pathlib import Path


def summarize(folder):
    """工具耗時以相鄰請求間隔估算，包含桌面輪詢與整理，不能當精確 COM 計時。"""
    requests = sorted(folder.glob("turn-*.request.json"))
    records = [json.loads(path.read_text(encoding="utf-8")) for path in requests]
    rows = []
    for index, path in enumerate(requests):
        request = records[index]
        prefix = path.name.removesuffix(".request.json")
        response_path = folder / (prefix + ".response.json")
        error_path = folder / (prefix + ".error.txt")
        response = json.loads(response_path.read_text(encoding="utf-8")) if response_path.exists() else {}
        state = {}
        for message in request["messages"]:
            content = message.get("content", "")
            if isinstance(content, str) and content.startswith("本機續接資料：\n"):
                state = json.loads(content.split("\n", 1)[1])
        row = {
            "turn": prefix,
            "tool": response.get("tool"),
            "offset": response.get("arguments", {}).get("offset"),
            "model_seconds": response.get("elapsed_seconds"),
            "request_bytes": path.stat().st_size,
            "message_count": len(request["messages"]),
            "cad_paging": state.get("cad_paging", []),
            "error": error_path.read_text(encoding="utf-8") if error_path.exists() else None,
        }
        if response_path.exists() and index + 1 < len(requests):
            row["desktop_interval_seconds"] = round(
                requests[index + 1].stat().st_mtime - response_path.stat().st_mtime, 3)
            # 只有下一份真實桌面快照中相同 call_id 的結果，才算工具執行成功。
            # 模型選了 offset，不代表 CAD 已成功交付那一頁。
            calls = response["completion"]["choices"][0]["message"].get("tool_calls", [])
            if calls:
                for message in records[index + 1]["messages"]:
                    if message.get("role") == "tool" and message.get("tool_call_id") == calls[0]["id"]:
                        row["tool_ok"] = json.loads(message["content"]).get("ok")
        if response.get("tool") == "finish":
            row["answer"] = response["arguments"].get("message")
        rows.append(row)
    return {"folder": str(folder.resolve()), "turns": rows,
            "cad_offsets": [row["offset"] for row in rows if row["tool"] == "cad_active_read"],
            "successful_cad_offsets": [row["offset"] for row in rows
                                       if row["tool"] == "cad_active_read" and row.get("tool_ok") is True],
            "finish_called": bool(rows and rows[-1]["tool"] == "finish")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    args = parser.parse_args()
    result = summarize(args.folder)
    output = args.folder / "summary.json"
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"turns": len(result["turns"]), "cad_offsets": result["cad_offsets"],
                      "successful_cad_offsets": result["successful_cad_offsets"],
                      "finish_called": result["finish_called"], "summary": str(output)}, ensure_ascii=True))


if __name__ == "__main__":
    main()
