"""localhost 模型轉接：沿用桌面 agent 契約，每次推論使用獨立 Codex 對話。

僅供本機合成資料驗證。Codex 以原生動態工具選擇下一步，再轉成桌面
tool_calls；工具仍由 LM_AI 執行。這不是公司模型或原生 API 的效能等價模擬。
不讀取／複製 Codex 登入憑證，由官方 app-server 自行使用既有登入。
"""
import argparse
import concurrent.futures
import copy
import json
from pathlib import Path
import queue
import secrets
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]
MODEL = "gpt-6-luna"


class CodexTurn:
    """一個程序只推論一輪，避免 Codex 歷史或自動摘要補回桌面丟失的資訊。"""

    def __init__(self, executable, folder, trace_prefix):
        self.events = queue.Queue()
        # 留下協定階段與 stderr，才能區分啟動、網路、模型與工具端的等待。
        self.trace_path = trace_prefix.with_suffix(".events.jsonl")
        self.stderr = trace_prefix.with_suffix(".stderr.log").open("w", encoding="utf-8")
        self.process = subprocess.Popen(
            [str(executable), "app-server", "--stdio"], cwd=folder,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr,
            text=True, encoding="utf-8", creationflags=subprocess.CREATE_NO_WINDOW,
        )
        self.sequence = 0
        self.deadline = time.monotonic() + 240
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.process.stdout:
            try:
                event = json.loads(line)
                # 不記錄 token、完整模型文字或工具正文；只保存診斷所需的事件種類。
                params = event.get("params", {})
                with self.trace_path.open("a", encoding="utf-8") as log:
                    log.write(json.dumps({"time": time.time(), "method": event.get("method"),
                        "id": event.get("id"), "item_type": params.get("item", {}).get("type"),
                        "error": event.get("error")}, ensure_ascii=False) + "\n")
                self.events.put(event)
            except ValueError:
                continue
        self.events.put({"error": "Codex app-server 已結束"})

    def receive(self):
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("Codex 單輪超過 240 秒")
        try:
            event = self.events.get(timeout=remaining)
        except queue.Empty as error:
            raise TimeoutError("Codex 單輪超過 240 秒，未收到完成回覆") from error
        if "error" in event:
            raise RuntimeError(str(event["error"]))
        return event

    def call(self, method, params):
        self.sequence += 1
        self.process.stdin.write(json.dumps({"id": self.sequence, "method": method, "params": params}) + "\n")
        self.process.stdin.flush()
        while True:
            event = self.receive()
            if event.get("id") == self.sequence:
                return event["result"]

    def infer(self, request, folder):
        self.call("initialize", {"clientInfo": {"name": "lmai_local_luna", "version": "0.1"},
                                 "capabilities": {"experimentalApi": True}})
        catalog = self.call("model/list", {"includeHidden": True})
        model = next((item for item in catalog["data"] if item["model"] == MODEL), None)
        if not model or not any(item["reasoningEffort"] == "low" for item in model["supportedReasoningEfforts"]):
            raise RuntimeError("目前帳號未提供 GPT-6 Luna low；不自動替換模型")
        tools = request.get("tools", [])
        names = [tool["function"]["name"] for tool in tools]
        # 使用原生動態工具：模型呼叫後立即把參數交回桌面，Codex 不執行工具。
        dynamic_tools = [{"type": "function", "name": tool["function"]["name"],
            "description": tool["function"].get("description", ""),
            "inputSchema": tool["function"]["parameters"]} for tool in tools]
        system = "\n\n".join(message["content"] for message in request["messages"]
                             if message["role"] == "system")
        messages = [message for message in request["messages"] if message["role"] != "system"]
        instructions = (
            "你是 LM_AI 桌面代理的單輪模型端點。輸入是桌面當輪的原始對話快照，依序閱讀。"
            "只呼叫此次提供的動態工具；桌面會執行工具後在下一輪提供結果。"
            "不要使用 Codex 內建的 shell、檔案、MCP、搜尋或記憶工具，也不要自行操作 CAD。"
            "不要把工具呼叫寫成正文 JSON。若有提供工具，請選一項工具呼叫；完成也使用 finish。"
            "若沒有動態工具，直接回答使用者。\n\n" + system
        )
        thread = self.call("thread/start", {"model": MODEL, "cwd": str(folder), "ephemeral": True,
            "approvalPolicy": "never", "sandbox": "read-only", "baseInstructions": instructions,
            "dynamicTools": dynamic_tools, "allowProviderModelFallback": False,
            "config": {"features.shell_tool": False, "features.multi_agent": False,
                       "features.apps": False, "web_search": "disabled", "project_doc_max_bytes": 0}})
        self.call("turn/start", {"threadId": thread["thread"]["id"], "effort": "low",
            "input": [{"type": "text", "text": json.dumps(messages, ensure_ascii=False),
                       "text_elements": []}]})
        answer, usage = None, None
        while True:
            event = self.receive()
            method, params = event.get("method"), event.get("params", {})
            if method == "thread/tokenUsage/updated":
                usage = params.get("tokenUsage")
            if method == "item/tool/call":
                name, arguments = params["tool"], params["arguments"]
                if name not in names or not isinstance(arguments, dict):
                    raise ValueError("模型選擇未提供的工具或非 object 參數")
                return {"name": name, "content": ""}, arguments, usage
            if method == "item/completed" and params["item"]["type"] == "agentMessage":
                answer = params["item"]["text"]
            if method == "item/started" and params["item"]["type"] in (
                "commandExecution", "fileChange", "mcpToolCall", "webSearch"):
                raise RuntimeError("測試模型嘗試額外工具；停止以免外部上下文污染測試")
            if method == "turn/completed":
                if params["turn"]["status"] != "completed" or answer is None:
                    raise RuntimeError(str(params["turn"].get("error") or "模型未完成"))
                if tools:
                    raise RuntimeError("模型未呼叫桌面工具，只有文字回覆：" + answer[:500])
                return {"name": "text", "content": answer}, {}, usage

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=10)
        self.process.stdin.close()
        self.process.stdout.close()
        self.stderr.close()


class Bridge:
    def __init__(self, args):
        self.args = args
        self.folder = args.output.resolve()
        self.folder.mkdir(parents=True, exist_ok=True)
        self.token = secrets.token_urlsafe(32)
        self.tasks, self.requests, self.fingerprints = {}, {}, {}
        self.lock = threading.RLock()
        self.executor = concurrent.futures.ThreadPoolExecutor(max_workers=1)
        self.count = 0

    def submit(self, body):
        request_id = body["client_request_id"]
        fingerprint = json.dumps(body, sort_keys=True)
        with self.lock:
            if request_id in self.requests:
                if self.fingerprints[request_id] != fingerprint:
                    raise ValueError("同一請求 ID 不可換內容")
                return copy.deepcopy(self.tasks[self.requests[request_id]])
            if self.count >= self.args.max_turns:
                raise ValueError("已達本機測試推論輪數上限")
            self.count += 1
            task_id = "luna_" + secrets.token_hex(8)
            task = {"contract_version": "desktop-agent-v1", "task_id": task_id,
                "client_request_id": request_id, "conversation_id": body["conversation_id"],
                "context": body["context"], "state": "queued", "result": None,
                "error": None, "error_message": ""}
            self.tasks[task_id] = task
            self.requests[request_id] = task_id
            self.fingerprints[request_id] = fingerprint
            accepted = copy.deepcopy(task)
            self.executor.submit(self.work, task_id, body, self.count)
            return accepted

    def work(self, task_id, request, sequence):
        started = time.monotonic()
        prefix = self.folder / f"turn-{sequence:03d}"
        prefix.with_suffix(".request.json").write_text(json.dumps(request, ensure_ascii=False, indent=2), encoding="utf-8")
        client = None
        try:
            with self.lock:
                if self.tasks[task_id]["state"] == "cancelled":
                    return
                self.tasks[task_id]["state"] = "running"
            client = CodexTurn(self.args.codex, self.folder, prefix)
            chosen, arguments, usage = client.infer(request, self.folder)
            message = {"role": "assistant", "content": chosen["content"]}
            if request.get("tools"):
                message["tool_calls"] = [{"id": "call_" + str(sequence), "type": "function", "function": {
                    "name": chosen["name"], "arguments": json.dumps(arguments, ensure_ascii=False)}}]
            completion = {"id": task_id, "object": "chat.completion", "created": int(time.time()),
                "model": request["model"], "choices": [{"index": 0, "message": message,
                "finish_reason": "tool_calls" if request.get("tools") else "stop"}]}
            trace = {"model": MODEL, "effort": "low", "elapsed_seconds": round(time.monotonic()-started, 3),
                     "tool": chosen["name"], "arguments": arguments, "codex_usage": usage,
                     "request_bytes": len(json.dumps(request).encode()), "completion": completion}
            prefix.with_suffix(".response.json").write_text(json.dumps(trace, ensure_ascii=False, indent=2), encoding="utf-8")
            with self.lock:
                if self.tasks[task_id]["state"] != "cancelled":
                    self.tasks[task_id].update(state="completed", result=completion)
            print(f"turn={sequence} tool={chosen['name']} offset={arguments.get('offset', '-')} seconds={trace['elapsed_seconds']}", flush=True)
        except Exception as error:
            with self.lock:
                if self.tasks[task_id]["state"] != "cancelled":
                    self.tasks[task_id].update(state="failed", error_message=str(error),
                        error={"code": "LOCAL_LUNA_FAILED", "message": str(error), "retryable": False})
            prefix.with_suffix(".error.txt").write_text(str(error), encoding="utf-8")
            print(f"turn={sequence} failed: {error}", flush=True)
        finally:
            if client:
                client.close()


def handler(bridge):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def reply(self, status, value):
            body = json.dumps(value, ensure_ascii=False).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def dispatch(self):
            url = urlsplit(self.path)
            path = url.path
            if self.headers.get("Host", "").split(":")[0] not in ("127.0.0.1", "localhost"):
                return self.reply(403, {"error": "loopback only"})
            if self.command == "GET" and path in ("/", "/health"):
                return self.reply(200, {"service": "LM_AI localhost model bridge", "model": MODEL,
                    "effort": "low", "turns": bridge.count, "max_turns": bridge.args.max_turns})
            if self.headers.get("Authorization") != "Bearer " + bridge.token:
                return self.reply(401, {"error": "local test token required"})
            size = int(self.headers.get("Content-Length", "0"))
            if size > 2 * 1024 * 1024:
                return self.reply(413, {"error": "request too large", "task_accepted": False})
            body = json.loads(self.rfile.read(size)) if size else {}
            if path.endswith("/agent/capabilities"):
                caps = json.loads((ROOT / "src/projects/agent/test_capabilities.json").read_text())
                caps.update(model="quality", principal_id="local_luna", strict_tool_arguments=False,
                            capability_revision="local_luna_low_1")
                return self.reply(200, caps)
            if path.endswith("/capabilities"):
                return self.reply(200, {"contract_version": 1, "principal_id": "local_luna",
                    "execution_modes": ["background"], "attachments": {"enabled": False,
                    "max_count": 0, "max_file_bytes": 0, "max_total_bytes": 0,
                    "allowed_extensions": [], "allowed_mime_types": []}})
            if path.endswith("/models"):
                return self.reply(200, {"models": [{"id": "quality", "label": "GPT-6 Luna · low（本機測試）"}], "default_model": "quality"})
            if path.endswith("/version"):
                return self.reply(200, {"latest_version": bridge.args.app_version,
                    "minimum_version": bridge.args.app_version, "message": "本機模型測試"})
            if path.endswith("/conversations") and self.command == "POST":
                return self.reply(200, {"conversation_id": "local_" + body["client_conversation_id"]})
            if path.endswith("/agent/turns") and self.command == "POST":
                return self.reply(202, bridge.submit(body))
            if "/tasks/" in path:
                ident = path.rsplit("/", 1)[-1]
                cancel = ident == "cancel"
                if cancel:
                    ident = path.split("/")[-2]
                with bridge.lock:
                    ident = bridge.requests.get(ident, ident)
                    task = bridge.tasks.get(ident)
                    if task:
                        if cancel and self.command == "POST":
                            task.update(state="cancelled", result=None)
                        return self.reply(200, task)
            if "notifications" in path:
                return self.reply(200, {"items": [], "unread_count": 0, "next_cursor": None})
            return self.reply(404, {"error": "local test route not implemented"})

        def do_GET(self):
            try:
                self.dispatch()
            except (ValueError, KeyError) as error:
                self.reply(400, {"error": str(error), "task_accepted": False})

        do_POST = do_GET
    return Handler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / ".build/luna-local")
    parser.add_argument("--port", type=int, default=8766)
    parser.add_argument("--max-turns", type=int, default=30)
    parser.add_argument("--app-version", default="0.8.68")
    args = parser.parse_args()
    bridge = Bridge(args)
    server = ThreadingHTTPServer(("127.0.0.1", args.port), handler(bridge))
    (bridge.folder / "connection.json").write_text(json.dumps({"origin": f"http://127.0.0.1:{server.server_port}",
        "token": bridge.token, "model": "quality"}), encoding="utf-8")
    print(f"Listening http://localhost:{server.server_port} — {MODEL} low", flush=True)
    try:
        server.serve_forever()
    finally:
        server.server_close()
        bridge.executor.shutdown(wait=True, cancel_futures=True)


if __name__ == "__main__":
    main()
