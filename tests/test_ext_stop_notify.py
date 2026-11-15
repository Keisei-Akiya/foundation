from __future__ import annotations

import json
import shutil
import socket
import threading
import time

import pytest

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, hermes_runtime_available, invoke_hook

pytestmark = pytest.mark.skipif(not hermes_runtime_available(), reason="Hermes source is unavailable")


def test_real_hermes_long_turn_notification_uses_loopback_and_excludes_private_context(monkeypatch):
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen(1)
    received = {}

    def receive():
        connection, _ = listener.accept()
        with connection:
            data = b""
            while b"\r\n\r\n" not in data:
                data += connection.recv(4096)
            headers, body = data.split(b"\r\n\r\n", 1)
            length = next(int(line.split(b":", 1)[1]) for line in headers.split(b"\r\n") if line.lower().startswith(b"content-length:"))
            while len(body) < length:
                body += connection.recv(4096)
            received["body"] = json.loads(body[:length])
            connection.sendall(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        listener.close()

    server = threading.Thread(target=receive, daemon=True)
    server.start()
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        shutil.copy2(BINARY_PATH, runtime.bin / "foundation")
        monkeypatch.setenv("FOUNDATION_DESKTOP_NOTIFY", "1")
        monkeypatch.setenv("FOUNDATION_DESKTOP_NOTIFY_THRESHOLD_SECONDS", "1")
        monkeypatch.setenv("FOUNDATION_DESKTOP_NOTIFY_WEBHOOK", f"http://127.0.0.1:{listener.getsockname()[1]}/webhook/")
        invoke_hook("pre_llm_call", session_id="s52", turn_id="t52", command="private command", path="/private/path")
        time.sleep(1.05)
        invoke_hook("post_llm_call", session_id="s52", turn_id="t52", assistant_response="secret-token", model="private-model")
    server.join(timeout=5)
    assert not server.is_alive()
    message = received["body"]["content"]
    assert "Long task completed" in message
    assert "secret-token" not in message
    assert "private command" not in message
    assert "/private/path" not in message
    assert "private-model" not in message
