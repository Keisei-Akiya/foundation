from __future__ import annotations

import json
import shutil
import time
from pathlib import Path
from typing import Any

import pytest

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, hermes_runtime_available, invoke_hook

pytestmark = pytest.mark.skipif(
    not hermes_runtime_available(), reason="Hermes source tree is unavailable"
)


def _install_logging_binary(runtime: IsolatedHermesHome) -> Path:
    assert BINARY_PATH.is_file(), "build foundation before running this test"
    real_binary = runtime.bin / "foundation-real"
    shutil.copy2(BINARY_PATH, real_binary)
    real_binary.chmod(0o755)
    log = runtime.root / "subcommands.jsonl"
    wrapper = runtime.bin / "foundation"
    wrapper.write_text(
        "#!/usr/bin/env python3\n"
        "import json, os, sys\n"
        f"with open({str(log)!r}, 'a', encoding='utf-8') as stream:\n"
        "    stream.write(json.dumps({'command': sys.argv[1:], 'payload': json.load(sys.stdin)}) + '\\n')\n"
        f"os.execv({str(real_binary)!r}, [{str(real_binary)!r}, *sys.argv[1:]])\n",
        encoding="utf-8",
    )
    wrapper.chmod(0o755)
    return log


def _commands(log: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines()]


def _reload_plugins() -> None:
    from hermes_cli.plugins import get_plugin_manager

    get_plugin_manager().discover_and_load(force=True)


def test_pre_mcp_health_check_runs_through_hermes_plugin_manager():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        log = _install_logging_binary(runtime)
        _reload_plugins()

        invoke_hook(
            "pre_tool_call",
            tool_name="mcp",
            args={"server_name": "runtime-mcp", "tool_name": "list_items"},
        )

        called = [entry["command"][0] for entry in _commands(log)]
        assert "mcp-health" in called, f"pre:mcp-health-check did not fire; commands={called!r}"


def test_post_mcp_health_check_runs_for_failed_mcp_tool_through_hermes():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        log = _install_logging_binary(runtime)
        _reload_plugins()

        invoke_hook(
            "post_tool_call",
            tool_name="mcp__runtime_mcp__list_items",
            args={"server_name": "runtime-mcp", "tool_name": "list_items"},
            status="error",
            error_type="MCPError",
            error_message="injected MCP failure",
        )

        called = [entry["command"][0] for entry in _commands(log)]
        assert "mcp-health" in called, f"post:mcp-health-check did not fire; commands={called!r}"
        event = next(entry["payload"] for entry in _commands(log) if entry["command"][0] == "mcp-health")
        assert event["status"] == "error"
        assert event["error_type"] == "MCPError"


def test_post_dispatcher_sync_dispatches_observer_and_lifecycle_via_hermes():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        log = _install_logging_binary(runtime)
        _reload_plugins()

        invoke_hook(
            "post_tool_call",
            tool_name="terminal",
            args={"command": "printf runtime"},
            result="runtime",
            status="success",
        )

        called = [entry["command"][0] for entry in _commands(log)]
        assert "post-observer" in called
        assert "lifecycle" in called


def test_skill_track_persists_failed_skill_run_via_hermes(monkeypatch: pytest.MonkeyPatch):
    monkeypatch.setenv("ECC_HERMES_SKILL_TRACK", "1")
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        _install_logging_binary(runtime)
        _reload_plugins()

        invoke_hook(
            "post_tool_call",
            tool_name="skill",
            args={"skill": "runtime-proof", "version": "1.2.3"},
            status="error",
            error_type="SkillError",
            error_message="injected skill failure",
        )

        runs = runtime.home / "plugin-data/ecc-hooks/skill-runs.jsonl"
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline and not runs.exists():
            time.sleep(0.05)
        assert runs.is_file(), "post:skill:track did not persist skill-runs.jsonl"
        records = [json.loads(line) for line in runs.read_text(encoding="utf-8").splitlines()]
        assert records == [
            {
                "skill_id": "runtime-proof",
                "skill_version": "1.2.3",
                "task_description": "Skill invocation: runtime-proof",
                "outcome": "failure",
            }
        ]


def test_async_post_tool_is_nonblocking_and_persists_skill_run(monkeypatch: pytest.MonkeyPatch):
    """Exercise the daemon observer through Hermes and verify its delayed CLI write."""
    assert BINARY_PATH.is_file(), "build foundation before running this test"

    with IsolatedHermesHome(enable_foundation=True) as runtime:
        real_binary = runtime.bin / "foundation-real"
        shutil.copy2(BINARY_PATH, real_binary)
        real_binary.chmod(0o755)

        wrapper = runtime.bin / "foundation"
        wrapper.write_text(
            "#!/usr/bin/env python3\n"
            "import sys, time\n"
            "if sys.argv[1:] == ['lifecycle']:\n"
            "    raise SystemExit(0)\n"
            "if sys.argv[1:] == ['post-async']:\n"
            "    time.sleep(1.0)\n"
            f"os.execv({str(real_binary)!r}, [{str(real_binary)!r}, *sys.argv[1:]])\n",
            encoding="utf-8",
        )
        wrapper.write_text(wrapper.read_text(encoding="utf-8").replace("import sys, time", "import os, sys, time"), encoding="utf-8")
        wrapper.chmod(0o755)
        monkeypatch.setenv("ECC_HERMES_SKILL_TRACK", "1")
        _reload_plugins()

        runs = runtime.home / "plugin-data/ecc-hooks/skill-runs.jsonl"
        started = time.monotonic()
        results = invoke_hook(
            "post_tool_call",
            tool_name="skill",
            args={"skill": "runtime-proof", "version": "1.2.3"},
            status="success",
        )
        elapsed = time.monotonic() - started

        assert results == []
        assert elapsed < 0.5, f"post_tool_call blocked for {elapsed:.3f}s"
        assert not runs.exists(), "the delayed detached worker should not have completed yet"

        deadline = time.monotonic() + 5
        while time.monotonic() < deadline and not runs.exists():
            time.sleep(0.05)
        assert runs.is_file(), "post-async did not persist skill-runs.jsonl"
        records = [json.loads(line) for line in runs.read_text(encoding="utf-8").splitlines()]
        assert records == [
            {
                "skill_id": "runtime-proof",
                "skill_version": "1.2.3",
                "task_description": "Skill invocation: runtime-proof",
                "outcome": "success",
            }
        ]
