"""Opt-in, privacy-minimal lifecycle observers for Hermes hooks.

Only the skill-run telemetry subset of ECC #18 is portable here. Hermes's
``post_tool_call`` hook reports tool results, but this adapter does not pretend
to provide the MCP manager's health/reconnect API (#8/#17), Claude session
bootstrap (#13), or the Plan Canvas store/CLI (#14). The skill tracker records
only validated identifiers and outcome; it never persists tool arguments,
prompt text, results, paths, or environment values.

Enable persistence explicitly with ``ECC_HERMES_SKILL_TRACK=1``. State is
isolated beneath the active ``HERMES_HOME`` (or Hermes's resolved home).
"""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path
from typing import Any

MAX_STDIN_CHARS = 1_048_576
MAX_ID = 128
_FAILURE_MARKER_RE = re.compile(
    r'"(?:error|failed)"(?!\s*:\s*(?:null|false|0|""|\[\]|\{\})\s*(?:[,}\]]|$))',
    re.IGNORECASE,
)
MAX_VERSION = 64
_ID = re.compile(r"^[A-Za-z0-9._:@/-]+$")
_VERSION = re.compile(r"^[A-Za-z0-9._+-]+$")
_TOOL = re.compile(r"^[A-Za-z0-9_.:-]{1,128}$")


def _identifier(value: Any, pattern: re.Pattern[str], limit: int) -> str | None:
    if not isinstance(value, str):
        return None
    value = value.strip()
    if not value or len(value) > limit or not pattern.fullmatch(value):
        return None
    if pattern is _ID and any(part in {".", ".."} for part in value.split("/")):
        return None
    return value


def _hermes_home() -> Path:
    configured = os.environ.get("HERMES_HOME")
    if configured:
        return Path(configured).expanduser()
    from hermes_constants import get_hermes_home

    return Path(get_hermes_home()).expanduser()


def _outcome(payload: dict[str, Any], extra: dict[str, Any], event_extra: dict[str, Any]) -> str:
    """Classify only explicit Hermes result/status signals; opaque payloads stay unknown."""
    status = payload.get("status", extra.get("status", event_extra.get("status")))
    status_text = str(status or "").strip().lower()
    if status_text in {"error", "failed", "failure", "timeout", "timed_out", "timed out"}:
        return "failure"
    if any(bool(source.get(key)) for source in (extra, event_extra)
           for key in ("error_type", "timed_out", "timeout")):
        return "failure"
    result = payload.get("result", extra.get("result", event_extra.get("result")))
    if isinstance(result, str):
        if result.startswith("Error") or any(
            match.start() < 500 for match in _FAILURE_MARKER_RE.finditer(result[:600])
        ):
            return "failure"
        try:
            parsed = json.loads(result)
        except (TypeError, ValueError):
            parsed = None
        if isinstance(parsed, dict):
            result = parsed
    if isinstance(result, dict):
        result_status = str(result.get("status") or "").strip().lower()
        if (result.get("is_error") is True or result.get("isError") is True
                or result.get("success") is False or result.get("ok") is False
                or bool(result.get("error"))
                or result_status in {"error", "failed", "failure", "timeout", "timed_out", "timed out"}
                or result.get("timed_out") is True or result.get("timeout") is True):
            return "failure"
        exit_code = result.get("exit_code")
        if isinstance(exit_code, int) and not isinstance(exit_code, bool):
            return "success" if exit_code == 0 else "failure"
        if result.get("success") is True or result.get("ok") is True:
            return "success"
    if status_text in {"ok", "success", "succeeded", "completed"}:
        return "success"
    return "unknown"


def build_skill_record(payload: Any) -> dict[str, str] | None:
    """Extract an outcome-only skill record from Hermes post_tool_call payload."""
    if not isinstance(payload, dict) or payload.get("hook_event_name") != "post_tool_call":
        return None
    if str(payload.get("tool_name") or "").strip().lower() != "skill":
        return None
    args = payload.get("tool_input", payload.get("args"))
    if not isinstance(args, dict):
        args = {}
    extra = payload.get("extra")
    extra = extra if isinstance(extra, dict) else {}
    event_extra = extra.get("extra")
    if not isinstance(event_extra, dict):
        event_extra = {}
    if not args and isinstance(extra.get("tool_input"), dict):
        args = extra["tool_input"]
    skill_id = next((value for item in (
        args.get("skill_id"), args.get("skillId"), args.get("skill"),
        args.get("name"), args.get("command"),
    ) if (value := _identifier(item, _ID, MAX_ID))), None)
    if skill_id is None:
        return None
    version = next((value for item in (
        args.get("skill_version"), args.get("skillVersion"), args.get("version"),
    ) if (value := _identifier(item, _VERSION, MAX_VERSION))), "unknown")
    outcome = _outcome(payload, extra, event_extra)
    return {"skill_id": skill_id, "skill_version": version, "outcome": outcome}


def build_tool_record(payload: Any) -> dict[str, str] | None:
    """Return privacy-minimal continuous-learning telemetry for one tool call."""
    if not isinstance(payload, dict) or payload.get("hook_event_name") != "post_tool_call":
        return None
    tool_name = payload.get("tool_name")
    if not isinstance(tool_name, str) or not _TOOL.fullmatch(tool_name):
        return None
    extra = payload.get("extra")
    extra = extra if isinstance(extra, dict) else {}
    nested = extra.get("extra")
    nested = nested if isinstance(nested, dict) else {}
    outcome = _outcome(payload, extra, nested)
    # Call ID permits deterministic duplicate suppression without retaining arguments/results.
    call_id = extra.get("tool_call_id", nested.get("tool_call_id"))
    call_id = _identifier(call_id, _ID, MAX_ID)
    if call_id is None:
        return None
    return {"tool_name": tool_name, "tool_call_id": call_id, "outcome": outcome}


def _append_record(root: Path, filename: str, record: dict[str, str]) -> bool:
    try:
        directory = root / "plugin-data" / "ecc-hooks"
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        target = directory / filename
        fd = os.open(target, os.O_RDWR | os.O_APPEND | os.O_CREAT, 0o600)
        with os.fdopen(fd, "r+", encoding="utf-8") as stream:
            try:
                import fcntl
                fcntl.flock(stream.fileno(), fcntl.LOCK_EX)
            except ImportError:
                pass
            if "tool_call_id" in record:
                stream.seek(0)
                for line in stream:
                    try:
                        if json.loads(line).get("tool_call_id") == record["tool_call_id"]:
                            return False
                    except (json.JSONDecodeError, AttributeError):
                        continue
                stream.seek(0, os.SEEK_END)
            stream.write(json.dumps(record, ensure_ascii=True, separators=(",", ":")) + "\n")
            stream.flush()
            return True
    except (OSError, RuntimeError, ImportError):
        # Telemetry is best-effort and must never interfere with tool execution.
        return False


def run_continuous_learning_once(payload: Any, *, home: str | Path | None = None,
                                 enabled: bool | None = None) -> bool:
    """Observe a Hermes post-tool event without recording any content."""
    if enabled is None:
        enabled = os.environ.get("ECC_HERMES_CONTINUOUS_LEARNING") == "1"
    if not enabled:
        return False
    record = build_tool_record(payload)
    if record is None:
        return False
    try:
        root = Path(home).expanduser() if home is not None else _hermes_home()
        return _append_record(root, "tool-observations.jsonl", record)
    except (OSError, RuntimeError, ImportError):
        return False


def run_once(payload: Any, *, home: str | Path | None = None,
             enabled: bool | None = None) -> bool:
    """Append one best-effort JSONL record; return whether it was written."""
    if enabled is None:
        enabled = os.environ.get("ECC_HERMES_SKILL_TRACK") == "1"
    if not enabled:
        return False
    record = build_skill_record(payload)
    if record is None:
        return False
    try:
        root = Path(home).expanduser() if home is not None else _hermes_home()
        directory = root / "plugin-data" / "ecc-hooks"
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        target = directory / "skill-runs.jsonl"
        fd = os.open(target, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
        with os.fdopen(fd, "a", encoding="utf-8") as stream:
            stream.write(json.dumps(record, ensure_ascii=True, separators=(",", ":")) + "\n")
        return True
    except (OSError, RuntimeError, ImportError):
        # Telemetry is best-effort and must never interfere with tool execution.
        return False


def main() -> int:
    """Consume exactly one Hermes shell-hook JSON event from stdin."""
    try:
        raw = sys.stdin.read(MAX_STDIN_CHARS + 1)
        if len(raw) > MAX_STDIN_CHARS:
            return 0
        run_continuous_learning_once(json.loads(raw))
        run_once(json.loads(raw))
    except (json.JSONDecodeError, OSError, ValueError):
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
