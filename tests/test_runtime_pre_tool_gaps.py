"""Regression coverage for hooks previously missing live Hermes firing proof."""
from __future__ import annotations

import json
import shutil

import pytest

from hermes_runtime import (
    BINARY_PATH,
    IsolatedHermesHome,
    hermes_runtime_available,
    invoke_hook,
)

pytest = pytest.importorskip("pytest")
if not hermes_runtime_available():
    pytest.skip("Hermes source tree unavailable", allow_module_level=True)


def _context(results: list[object]) -> str:
    return "\n".join(
        item["context"]
        for item in results
        if isinstance(item, dict) and isinstance(item.get("context"), str)
    )


def test_doc_warning_and_suggest_compact_reach_real_pre_llm_hook():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        shutil.copy2(BINARY_PATH, runtime.bin / "foundation")
        session = "runtime-doc-compact"

        invoke_hook(
            "pre_tool_call",
            tool_name="write_file",
            args={"path": "/tmp/NOTES.md"},
            session_id=session,
        )
        warning = _context(invoke_hook("pre_llm_call", session_id=session))
        assert "Ad-hoc documentation filename detected" in warning
        assert "/tmp/NOTES.md" in warning

        # Count tool calls through the actual Hermes dispatcher, not a direct
        # Rust CLI invocation or isolated callback call.
        for _ in range(49):
            invoke_hook(
                "pre_tool_call",
                tool_name="read_file",
                args={"path": "/tmp/ordinary.txt"},
                session_id=session,
            )
        suggestion = _context(invoke_hook("pre_llm_call", session_id=session))
        assert "50 tool calls reached" in suggestion
        assert "/compact" in suggestion


def test_gateguard_blocks_first_real_hermes_write_then_allows_retry():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        shutil.copy2(BINARY_PATH, runtime.bin / "foundation")
        kwargs = {
            "tool_name": "write_file",
            "args": {"path": "/tmp/runtime-gateguard-target"},
            "session_id": "runtime-gateguard",
        }
        first = invoke_hook("pre_tool_call", **kwargs)
        assert any(isinstance(item, dict) and item.get("action") == "block" for item in first)
        second = invoke_hook("pre_tool_call", **kwargs)
        assert not any(isinstance(item, dict) and item.get("action") == "block" for item in second)


def test_governance_capture_persists_sanitized_event_via_real_hermes_hook(monkeypatch):
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        shutil.copy2(BINARY_PATH, runtime.bin / "foundation")
        monkeypatch.setenv("ECC_GOVERNANCE_CAPTURE", "1")
        secret = "ghp_" + "A" * 36
        invoke_hook(
            "pre_tool_call",
            tool_name="terminal",
            args={"command": f'echo token="{secret}"'},
            session_id="runtime-governance",
        )
        events_path = runtime.home / "plugin-data/foundation/governance-capture.jsonl"
        assert events_path.is_file()
        events = [json.loads(line) for line in events_path.read_text(encoding="utf-8").splitlines()]
        assert len(events) == 1
        assert events[0]["eventType"] == "secret_detected"
        assert events[0]["payload"]["secretTypes"] == ["github_token"]
        assert secret not in events_path.read_text(encoding="utf-8")
