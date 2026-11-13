from __future__ import annotations

import json
import os
import shutil
from pathlib import Path

import pytest

from hermes_runtime import BINARY_PATH, IsolatedHermesHome, hermes_runtime_available, invoke_hook

pytestmark = pytest.mark.skipif(
    not hermes_runtime_available(), reason="Hermes source tree is unavailable"
)


@pytest.fixture
def live_foundation():
    """Load the installed Hermes manager and route callbacks to the real binary."""
    if not BINARY_PATH.is_file():
        pytest.fail("build foundation before running this suite")

    with IsolatedHermesHome(enable_foundation=True) as runtime:
        real_binary = runtime.bin / "foundation-real"
        shutil.copy2(BINARY_PATH, real_binary)
        real_binary.chmod(0o755)
        trace = runtime.root / "hook-invocations.jsonl"

        # Keep the production plugin/callback path intact while recording the
        # native command each real Hermes callback invokes.
        wrapper = runtime.bin / "foundation"
        wrapper.write_text(
            "#!/usr/bin/env python3\n"
            "import json, os, subprocess, sys\n"
            "payload = sys.stdin.read()\n"
            "with open(os.environ['FOUNDATION_HOOK_TRACE'], 'a', encoding='utf-8') as f:\n"
            "    f.write(json.dumps({'command': sys.argv[1], 'payload': json.loads(payload) if payload else {}}) + '\\n')\n"
            "result = subprocess.run([os.environ['FOUNDATION_REAL_BIN'], *sys.argv[1:]], input=payload, text=True)\n"
            "raise SystemExit(result.returncode)\n",
            encoding="utf-8",
        )
        wrapper.chmod(0o755)
        old_env = {
            key: os.environ.get(key)
            for key in ("FOUNDATION_REAL_BIN", "FOUNDATION_HOOK_TRACE")
        }
        os.environ["FOUNDATION_REAL_BIN"] = str(real_binary)
        os.environ["FOUNDATION_HOOK_TRACE"] = str(trace)
        try:
            from hermes_cli.plugins import get_plugin_manager

            get_plugin_manager().discover_and_load(force=True)
            yield runtime, trace
        finally:
            for key, value in old_env.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value


def _calls(trace: Path) -> list[dict]:
    return [json.loads(line) for line in trace.read_text(encoding="utf-8").splitlines()]


def test_real_hermes_prompt_hook_blocks_matching_prompt(live_foundation, tmp_path):
    _runtime, trace = live_foundation
    rules = tmp_path / ".claude"
    rules.mkdir()
    (rules / "hookify.no-secret.local.md").write_text(
        "---\nname: no-secret\nevent: prompt\naction: block\npattern: secret\n---\nDo not submit secrets.\n",
        encoding="utf-8",
    )

    results = invoke_hook(
        "pre_llm_call",
        user_message="send SECRET now",
        is_first_turn=False,
        session_id="runtime-prompt-test",
        cwd=str(tmp_path),
    )

    assert any(isinstance(result, dict) and result.get("action") == "block" for result in results), results
    commands = [call["command"] for call in _calls(trace)]
    assert "prompt-submit" in commands
    prompt_call = next(call for call in _calls(trace) if call["command"] == "prompt-submit")
    assert prompt_call["payload"]["hook_event_name"] == "pre_llm_call"


def test_real_hermes_session_start_dispatches_lockfile_detection(live_foundation, tmp_path):
    runtime, trace = live_foundation
    (tmp_path / "package-lock.json").write_text("{}\n", encoding="utf-8")

    invoke_hook(
        "on_session_start",
        session_id="runtime-session-test",
        cwd=str(tmp_path),
        platform="cli",
        reason="new_session",
    )

    calls = _calls(trace)
    session_call = next(call for call in calls if call["command"] == "session-hooks")
    assert session_call["payload"]["hook_event_name"] == "on_session_start"


def test_real_hermes_cost_hooks_persist_usage_and_session_total(live_foundation):
    runtime, trace = live_foundation

    invoke_hook(
        "post_api_request",
        session_id="runtime-cost-test",
        api_request_id="request-1",
        response_model="model-x",
        usage={"input_tokens": 10, "output_tokens": 4, "cache_creation_input_tokens": 2, "cache_read_input_tokens": 3},
    )
    invoke_hook(
        "post_llm_call",
        session_id="runtime-cost-test",
        model="model-x",
        assistant_response="must not be persisted",
    )

    calls = _calls(trace)
    assert sum(call["command"] == "cost-tracker" for call in calls) >= 2
    assert any(call["command"] == "desktop-notify" for call in calls)
    path = runtime.home / "plugin-data/foundation/cost_tracker.jsonl"
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    totals = [row for row in rows if row.get("kind") == "session_total"]
    assert len(totals) == 1
    assert totals[0]["session_id"] == "runtime-cost-test"
    assert totals[0]["input_tokens"] == 10
    assert totals[0]["output_tokens"] == 4
    assert totals[0]["cache_write_tokens"] == 2
    assert totals[0]["cache_read_tokens"] == 3
    assert totals[0]["cost"] == "not_calculated"
    assert "must not be persisted" not in path.read_text(encoding="utf-8")


def test_real_hermes_desktop_notify_hook_dispatches_on_linux(live_foundation):
    runtime, trace = live_foundation

    invoke_hook(
        "post_llm_call",
        session_id="runtime-notify-test",
        model="model-x",
        assistant_response="private response",
    )

    calls = _calls(trace)
    assert sum(call["command"] == "cost-tracker" for call in calls) >= 1
    notify_call = next(call for call in calls if call["command"] == "desktop-notify")
    assert notify_call["payload"]["hook_event_name"] == "post_llm_call"
    # Linux is intentionally a no-op for notification delivery.
    assert not (runtime.home / "plugin-data/foundation/desktop_notify.jsonl").exists()
