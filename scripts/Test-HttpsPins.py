"""以臨時自簽憑證測試正式 Rust WinHTTP 路徑，只連本機 127.0.0.1。"""
from pathlib import Path
import argparse
import base64
import hashlib
import os
import socket
import ssl
import subprocess
import tempfile
import threading
import json
import shutil


def certificate(root, openssl, label, hostname):
    """公私鑰只留在本次測試目錄，回傳的是公鑰雜湊。"""
    run = lambda *args: subprocess.run([openssl, *args], cwd=root, check=True, capture_output=True)
    run("req", "-new", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
        "-noenc", "-keyout", f"{label}.key", "-out", f"{label}.crt", "-days", "2",
        "-subj", f"/CN={hostname}", "-addext", f"subjectAltName=DNS:{hostname}",
        "-addext", "extendedKeyUsage=serverAuth")
    run("x509", "-in", f"{label}.crt", "-pubkey", "-noout", "-out", f"{label}.pub")
    spki = run("pkey", "-pubin", "-in", f"{label}.pub", "-outform", "DER").stdout
    return base64.b64encode(hashlib.sha256(spki).digest()).decode("ascii")


def verify(root, test_exe, label, pins, expected):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(root / f"{label}.crt", root / f"{label}.key")
    observed = []
    errors = []
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(1)
        listener.settimeout(15)

        def serve():
            received = bytearray()
            try:
                connection, _ = listener.accept()
                with connection:
                    connection.settimeout(8)
                    with context.wrap_socket(connection, server_side=True) as stream:
                        while len(received) < 8192 and b"fixture-payload" not in received:
                            data = stream.recv(4096)
                            if not data:
                                break
                            received.extend(data)
                        if received:
                            stream.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            except (ConnectionResetError, ssl.SSLError) as error:
                if expected == "match":
                    errors.append(str(error))
            except OSError as error:
                errors.append(str(error))
            finally:
                # 驗證只輸出布林及長度，測試也不把 Header 或本文寫進日誌。
                observed.append((len(received), b"authorization:" in received.lower(),
                                 b"fixture-payload" in received))

        server = threading.Thread(target=serve, daemon=True)
        server.start()
        environment = os.environ.copy()
        environment.update({
            "LMAI_TLS_FIXTURE_URL": f"https://localhost:{listener.getsockname()[1]}/probe",
            "LMAI_TLS_FIXTURE_PRIMARY": pins[0],
            "LMAI_TLS_FIXTURE_BACKUP": pins[1],
            "LMAI_TLS_FIXTURE_EXPECTED": expected,
        })
        subprocess.run([test_exe, "transport::pins::tests::tls_pin_loopback_fixture", "--exact", "--ignored"],
                       env=environment, timeout=20, check=True)
        server.join(timeout=12)
    if server.is_alive() or errors or len(observed) != 1:
        raise RuntimeError(f"TLS fixture {label} failed: {errors}")
    size, auth, body = observed[0]
    if expected == "match" and not (auth and body):
        raise RuntimeError(f"TLS fixture {label} did not receive the verified request")
    if expected != "match" and (size != 0 or auth or body):
        raise RuntimeError(f"TLS fixture {label} received data from a rejected connection")
    print(f"PASS {label}: bytes={size}, authorization={auth}, body={body}", flush=True)


def verify_signed_update(root, test_exe, pins, fixture):
    """已簽署的舊版成品用於驗收下載與簽章，所有檔案留在 fixture 目錄。"""
    manifest = fixture / "update-manifest.json"
    document = json.loads(manifest.read_text(encoding="utf-8-sig"))
    if document["url"] != "/lm_server/desktop/releases/LM_AI_Setup.exe":
        raise RuntimeError("只接受既有相對路徑 NSIS 清單")
    package = fixture / "LM_AI_Setup.exe"
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(root / "primary.crt", root / "primary.key")
    requests = []
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(1)
        listener.settimeout(30)

        def serve():
            for route, path in (("/update-manifest.json", manifest), (document["url"], package)):
                connection, _ = listener.accept()
                with connection, context.wrap_socket(connection, server_side=True) as stream:
                    stream.settimeout(20)
                    request = bytearray()
                    while b"\r\n\r\n" not in request and len(request) < 8192:
                        chunk = stream.recv(4096)
                        if not chunk:
                            break
                        request.extend(chunk)
                    if not request.startswith(f"GET {route} ".encode("ascii")) or b"authorization:" in request.lower():
                        raise RuntimeError("更新路徑不符或帶入授權資料")
                    header = f"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {path.stat().st_size}\r\nConnection: close\r\n\r\n"
                    stream.sendall(header.encode("ascii"))
                    with path.open("rb") as source, stream.makefile("wb") as target:
                        shutil.copyfileobj(source, target, length=64 * 1024)
                    requests.append(route)

        server = threading.Thread(target=serve, daemon=True)
        server.start()
        environment = os.environ.copy()
        environment.update({
            "LMAI_TLS_FIXTURE_URL": f"https://localhost:{listener.getsockname()[1]}/update-manifest.json",
            "LMAI_TLS_FIXTURE_PRIMARY": pins[0],
            "LMAI_TLS_FIXTURE_BACKUP": pins[1],
            "LMAI_TLS_FIXTURE_EXPECTED": "signed_update",
            "LMAI_TLS_FIXTURE_OUTPUT": str(root),
        })
        subprocess.run([test_exe, "transport::pins::tests::tls_pin_loopback_fixture", "--exact", "--ignored"],
                       env=environment, timeout=90, check=True)
        server.join(timeout=15)
    if requests != ["/update-manifest.json", document["url"]]:
        raise RuntimeError("簽署更新未從同一個本機來源下載")
    print("PASS signed update: pinned same-origin download, RSA signature and payload SHA-256", flush=True)


def main():
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("test_exe", type=Path, help="cargo test --lib --no-run 的測試執行檔")
    args.add_argument("--openssl", default=r"C:\Program Files\Git\usr\bin\openssl.exe")
    args.add_argument("--signed-update", type=Path, help="含舊版 LM_AI_Setup.exe 和簽署清單的測試目錄")
    options = args.parse_args()
    build = Path(".build").resolve()
    build.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="tls-pin-", dir=build) as directory:
        root = Path(directory)
        keys = {label: certificate(root, options.openssl, label, host) for label, host in (
            ("primary", "localhost"), ("backup", "localhost"), ("unmatched", "localhost"),
            ("wrong-name", "invalid.example"),
        )}
        trusted = keys["primary"], keys["backup"]
        for label, pins, expected in (
            ("primary", trusted, "match"), ("backup", trusted, "match"),
            ("unmatched", trusted, "pin_error"),
            ("wrong-name", (keys["wrong-name"], keys["backup"]), "name_error"),
        ):
            verify(root, options.test_exe.resolve(strict=True), label, pins, expected)
        if options.signed_update:
            verify_signed_update(root, options.test_exe.resolve(strict=True), trusted,
                                 options.signed_update.resolve(strict=True))


if __name__ == "__main__":
    main()
