"""私聊契約 fixture，只使用臨時憑證與 loopback。參數為 cargo test --lib 產生的 EXE。"""
from base64 import b64encode
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import json
import os
import runpy
import ssl
import subprocess
import tempfile
import threading

PREFIX = "/lm_server/api/desktop/chat/"
DEVICE = "d9b0160f-dedb-4a44-8c31-5c6a6f9a3af2"


class ChatFixture(BaseHTTPRequestHandler):
    """以真實 HTTP 方法、裝置標頭和 JSON 重現伺服器手冊的好友流程。"""

    chat_id = "0123-ABCD"
    enabled = False
    replaced = False
    invited = False
    friends = []
    answers = {}

    def log_message(self, *_args):
        pass

    def send_json(self, status, result):
        data = json.dumps(result, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def route(self):
        fixture = type(self)
        if not self.path.startswith(PREFIX):
            return 404, {"error": "not_found"}
        if self.headers.get("Authorization") != "Bearer fixture-private-chat":
            return 401, {"error": "unauthorized"}
        if self.headers.get("X-LMAI-Device") != DEVICE:
            return 409, {"error": "device_replaced"}
        path = self.path[len(PREFIX):]
        length = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(length)) if length else None
        if self.command == "POST" and path == "devices/activate" and body == {"device_id": DEVICE}:
            fixture.enabled = True
            return 200, {}
        if not fixture.enabled or fixture.replaced:
            return 409, {"error": "device_replaced"}
        if self.command == "GET" and path == "me":
            return 200, {"chat_id": fixture.chat_id, "nickname": "本機測試", "employee_no": "100"}
        if self.command == "GET" and path == "lookup?chat_id=4567-EFGH":
            return 200, {"nickname": "邀請對象", "employee_no": "200"}
        if self.command == "POST" and path == "invites" and body == {"chat_id": "4567-EFGH"}:
            fixture.invited = True
            return 200, {}
        if self.command == "GET" and path == "invites" and fixture.invited:
            return 200, {"incoming": [
                {"invite_id": "accept-fixture", "nickname": "接受測試", "employee_no": "300"},
                {"invite_id": "reject-fixture", "nickname": "拒絕測試", "employee_no": "400"},
                {"invite_id": "block-fixture", "nickname": "封鎖測試", "employee_no": "500"},
            ], "outgoing": [{"invite_id": "sent-fixture", "nickname": "邀請對象", "employee_no": "200"}]}
        if self.command == "POST" and path.startswith("invites/"):
            _, invite_id, action = path.split("/")
            fixture.answers[invite_id] = action
            if (invite_id, action) == ("accept-fixture", "accept"):
                fixture.friends.append({"friend_id": "friend-fixture", "nickname": "接受測試",
                                        "employee_no": "300", "status": "active"})
            return 200, {}
        if self.command == "GET" and path == "friends":
            return 200, fixture.friends
        if self.command == "DELETE" and path == "friends/friend-fixture":
            fixture.friends.clear()
            return 200, {}
        if self.command == "POST" and path == "me/chat-id/regenerate":
            fixture.chat_id = "9876-WXYZ"
            return 200, {"chat_id": fixture.chat_id}
        if self.command == "GET" and path == "key-secret":
            return 200, {"version": 1, "secret": b64encode(bytes([17] * 32)).decode("ascii")}
        if self.command == "POST" and path == "key-secret/reset":
            fixture.replaced = True
            return 200, {"version": 2, "secret": b64encode(bytes([19] * 32)).decode("ascii")}
        return 404, {"error": "not_found"}

    def do_GET(self):
        self.send_json(*self.route())

    do_POST = do_GET
    do_DELETE = do_GET


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("test_exe", type=Path)
    parser.add_argument("--openssl", default=r"C:\Program Files\Git\usr\bin\openssl.exe")
    arguments = parser.parse_args()
    test_exe = arguments.test_exe.resolve(strict=True)
    certificate = runpy.run_path(str(Path(__file__).with_name("Test-HttpsPins.py")))["certificate"]
    build = Path(".build").resolve()
    build.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(dir=build, prefix="private-chat-tls-") as temporary:
        root = Path(temporary)
        primary = certificate(root, arguments.openssl, "primary", "localhost")
        backup = certificate(root, arguments.openssl, "backup", "localhost")
        tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        tls.minimum_version = ssl.TLSVersion.TLSv1_2
        tls.load_cert_chain(root / "primary.crt", root / "primary.key")
        with ThreadingHTTPServer(("127.0.0.1", 0), ChatFixture) as server:
            server.socket = tls.wrap_socket(server.socket, server_side=True)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            environment = dict(os.environ, LMAI_CHAT_FIXTURE_PRIMARY=primary, LMAI_CHAT_FIXTURE_BACKUP=backup,
                               LMAI_CHAT_FIXTURE_URL=f"https://localhost:{server.server_port}{PREFIX}")
            try:
                subprocess.run([test_exe, "private_chat::api::tests::tls_private_chat_fixture",
                                "--exact", "--ignored"], env=environment, timeout=120, check=True)
            finally:
                server.shutdown()
                thread.join(timeout=10)
        assert ChatFixture.answers == {"block-fixture": "reject", "accept-fixture": "accept",
                                       "reject-fixture": "reject"}
        print("PASS private chat: TLS, device header, friend lifecycle, blacklist, secret reset and device replacement")


if __name__ == "__main__":
    main()
