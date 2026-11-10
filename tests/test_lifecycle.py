"""Tests for the portable portion of the lifecycle adapter."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from adapters.hermes.hooks.lifecycle import (
    build_skill_record, build_tool_record, run_continuous_learning_once, run_once,
)


def _payload(**overrides):
    payload = {
        "hook_event_name": "post_tool_call",
        "tool_name": "skill",
        "tool_input": {"skill_id": "research/topic", "skill_version": "1.2.3"},
        "extra": {"result": "PRIVATE PROMPT AND TOOL RESULT", "status": "ok"},
        "session_id": "session-x",
    }
    payload.update(overrides)
    return payload


def test_records_only_bounded_skill_identifiers_and_status():
    assert build_skill_record(_payload()) == {
        "skill_id": "research/topic", "skill_version": "1.2.3", "outcome": "success"
    }
    status_record = build_skill_record(_payload(status="error", error_message="sensitive details"))
    result_record = build_skill_record(_payload(result={"is_error": True}))
    json_result_record = build_skill_record(_payload(
        extra={"status": "ok", "result": '{"error":"tool failed"}'},
    ))
    raw_error_record = build_skill_record(_payload(
        extra={"status": "ok", "result": "Error: command timed out"},
    ))
    timeout_record = build_skill_record(_payload(status="timeout"))
    unknown_record = build_skill_record(_payload(extra={"result": "opaque result"}))
    assert status_record is not None and status_record["outcome"] == "failure"
    assert result_record is not None and result_record["outcome"] == "failure"
    assert json_result_record is not None and json_result_record["outcome"] == "failure"
    assert raw_error_record is not None and raw_error_record["outcome"] == "failure"
    assert timeout_record is not None and timeout_record["outcome"] == "failure"
    assert unknown_record is not None and unknown_record["outcome"] == "unknown"
    assert build_skill_record(_payload(tool_name="terminal")) is None
    assert build_skill_record(_payload(tool_input={"skill_id": "invalid id"})) is None
    assert build_skill_record(_payload(tool_input={"skill_id": "../path"})) is None


def test_run_once_is_opt_in_profile_scoped_and_does_not_persist_content(tmp_path):
    assert run_once(_payload(), home=tmp_path, enabled=False) is False
    assert not (tmp_path / "plugin-data" / "ecc-hooks" / "skill-runs.jsonl").exists()
    assert run_once(_payload(), home=tmp_path, enabled=True) is True
    target = tmp_path / "plugin-data" / "ecc-hooks" / "skill-runs.jsonl"
    line = target.read_text(encoding="utf-8")
    assert json.loads(line) == {
        "skill_id": "research/topic", "skill_version": "1.2.3", "outcome": "success"
    }
    assert "PRIVATE PROMPT" not in line and "session-x" not in line
    assert target.stat().st_mode & 0o777 == 0o600


def test_hook_runner_consumes_one_realistic_hermes_event_in_isolated_home(tmp_path):
    home = tmp_path / "isolated-hermes-home"
    env = {**os.environ, "HERMES_HOME": str(home), "ECC_HERMES_SKILL_TRACK": "1"}
    result = subprocess.run(
        [sys.executable, "-m", "adapters.hermes.hooks.lifecycle"],
        input=json.dumps(_payload(status="error", error_type="ToolExecutionError")),
        text=True, capture_output=True, env=env, check=False,
    )
    assert result.returncode == 0
    assert result.stderr == ""
    target = home / "plugin-data" / "ecc-hooks" / "skill-runs.jsonl"
    assert [json.loads(line) for line in target.read_text().splitlines()] == [{
        "skill_id": "research/topic", "skill_version": "1.2.3", "outcome": "failure"
    }]


def test_continuous_learning_observer_logs_minimal_outcome_and_deduplicates(tmp_path):
    payload = _payload(extra={
        "tool_call_id": "call-123", "status": "error", "error_type": "ToolError",
        "result": "DO NOT SAVE THIS RESULT", "tool_input": {"token": "secret"},
    }, tool_name="terminal", tool_input={"command": "private command"})
    assert build_tool_record(payload) == {
        "tool_name": "terminal", "tool_call_id": "call-123", "outcome": "failure"
    }
    assert run_continuous_learning_once(payload, home=tmp_path, enabled=False) is False
    assert run_continuous_learning_once(payload, home=tmp_path, enabled=True) is True
    assert run_continuous_learning_once(payload, home=tmp_path, enabled=True) is False
    target = tmp_path / "plugin-data" / "ecc-hooks" / "tool-observations.jsonl"
    lines = target.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1
    assert json.loads(lines[0]) == {
        "tool_name": "terminal", "tool_call_id": "call-123", "outcome": "failure"
    }
    assert "secret" not in lines[0] and "private command" not in lines[0]
    assert "DO NOT SAVE" not in lines[0]
    assert target.stat().st_mode & 0o777 == 0o600


def test_continuous_learning_requires_call_id_for_duplicate_safety():
    assert build_tool_record(_payload(tool_name="terminal")) is None


def test_write_failure_is_non_blocking(tmp_path):
    blocker = tmp_path / "not-a-directory"
    blocker.write_text("x")
    assert run_once(_payload(), home=blocker, enabled=True) is False
