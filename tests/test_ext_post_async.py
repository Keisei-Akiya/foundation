from __future__ import annotations

import importlib.util
import shutil
import threading
import time
from pathlib import Path

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, invoke_hook


HOOK_PATH = Path(__file__).resolve().parents[1] / "adapters/hermes/plugin/hooks_ext/post_async.py"
SPEC = importlib.util.spec_from_file_location("foundation_post_async_hook", HOOK_PATH)
assert SPEC is not None and SPEC.loader is not None
post_async = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(post_async)


class Context:
    def __init__(self) -> None:
        self.callback = None

    def register_hook(self, event: str, callback) -> None:
        assert event == "post_tool_call"
        self.callback = callback


class Core:
    def __init__(self) -> None:
        self.started = threading.Event()
        self.release = threading.Event()
        self.received = None
        self.logger = type("Logger", (), {"warning": staticmethod(lambda *args: None)})()

    @staticmethod
    def payload(event, kwargs):
        return {"hook_event_name": event, **kwargs}

    def run_subcommand(self, subcommand, payload, *, strict, timeout):
        self.received = (subcommand, payload, strict, timeout)
        self.started.set()
        self.release.wait(timeout=2)


def test_post_tool_callback_dispatches_in_background():
    ctx = Context()
    core = Core()
    post_async.register(ctx, core)
    assert ctx.callback is not None

    start = time.monotonic()
    ctx.callback(tool_name="write_file", args={"path": "example.py"})
    elapsed = time.monotonic() - start

    assert elapsed < 0.2
    assert core.started.wait(timeout=1)
    assert core.received == (
        "post-async",
        {"hook_event_name": "post_tool_call", "tool_name": "write_file", "args": {"path": "example.py"}},
        False,
        5,
    )
    core.release.set()


def test_post_tool_hook_runs_through_hermes_invoke_hook():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        assert BINARY_PATH.is_file(), "build foundation before running this suite"
        shutil.copy2(BINARY_PATH, runtime.bin / "foundation")
        (runtime.bin / "foundation").chmod(0o755)

        results = invoke_hook(
            "post_tool_call",
            tool_name="write_file",
            args={"path": "example.py"},
            result={"private": "must not persist"},
        )
        assert results == []
        time.sleep(0.1)
        assert "must not persist" not in "".join(
            path.read_text(encoding="utf-8")
            for path in runtime.home.rglob("*.jsonl")
        )
