"""Hermes native plugin `foundation`.

Each Hermes lifecycle callback builds the same stdin JSON that shell hooks get,
runs the `foundation` binary as a subprocess and converts its stdout into the
return value Hermes expects. The binary stays the single source of hook logic.
"""

from __future__ import annotations

import json
import logging
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any, Optional

logger = logging.getLogger(__name__)

_TIMEOUT_SECONDS = 5
_CONFIG_PROTECTION_TIMEOUT_SECONDS = 10
_BASH_TOOLS = frozenset({"terminal", "bash", "Bash"})
_PREFLIGHT_TOOLS = _BASH_TOOLS | frozenset(
    {"powershell", "PowerShell", "powershell_tool", "pwsh"}
)
_EDIT_TOOLS = frozenset(
    {"patch", "write_file", "edit", "edit_file", "write", "apply_patch", "str_replace_editor"}
)
_HOOKIFY_TOOLS = _BASH_TOOLS | frozenset(
    {"read_file", "patch", "write_file", "edit", "edit_file", "write", "apply_patch", "str_replace_editor", "Edit", "Write", "MultiEdit"}
)


class HookFailure(Exception):
    """The binary ran but gave no usable answer (crash, timeout, non-zero exit, bad output)."""


def _hermes_home() -> Path:
    return Path(os.environ.get("HERMES_HOME") or Path.home() / ".hermes")


def resolve_binary() -> Optional[str]:
    """Find the foundation binary: $FOUNDATION_BIN, <HERMES_HOME>/bin, then PATH."""
    explicit = os.environ.get("FOUNDATION_BIN")
    if explicit:
        return explicit if os.access(explicit, os.X_OK) else None
    installed = _hermes_home() / "bin" / "foundation"
    if os.access(installed, os.X_OK):
        return str(installed)
    return shutil.which("foundation")


def run_subcommand(
    subcommand: str,
    payload: dict[str, Any],
    *,
    strict: bool = False,
    timeout: int = _TIMEOUT_SECONDS,
) -> Optional[dict[str, Any]]:
    """Run `foundation <subcommand>` with payload on stdin; return its stdout JSON.

    Returns None when the binary exits successfully without output, which is
    the allow response for config-protection. A missing binary, crash, timeout,
    non-zero exit or non-JSON output is a failure: strict=False returns None
    (fail-open), strict=True raises HookFailure so the caller can fail closed.
    """
    binary = resolve_binary()
    if binary is None:
        logger.warning("foundation: binary not found; skipping %s", subcommand)
        if strict:
            raise HookFailure(f"{subcommand}: binary not found")
        return None
    try:
        proc = subprocess.run(
            [binary, subcommand],
            input=json.dumps(payload, ensure_ascii=False, default=str),
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as error:
        logger.warning("foundation %s failed: %s", subcommand, error)
        if strict:
            raise HookFailure(f"{subcommand}: {error}") from error
        return None
    out = proc.stdout.strip()
    if proc.returncode != 0:
        if strict:
            raise HookFailure(f"{subcommand}: exit code {proc.returncode}")
        return None
    if not out:
        return None
    try:
        data = json.loads(out)
    except json.JSONDecodeError as error:
        logger.warning("foundation %s printed non-JSON stdout", subcommand)
        if strict:
            raise HookFailure(f"{subcommand}: non-JSON output") from error
        return None
    if not isinstance(data, dict):
        if strict:
            raise HookFailure(f"{subcommand}: response is not a JSON object")
        return None
    return data


def _payload(event: str, kwargs: dict[str, Any]) -> dict[str, Any]:
    args = kwargs.get("args")
    payload = {
        "hook_event_name": event,
        "tool_name": kwargs.get("tool_name"),
        "tool_input": args if isinstance(args, dict) else None,
        "session_id": kwargs.get("session_id") or "",
        "cwd": kwargs.get("cwd") or os.getcwd(),
    }
    for field in (
        "result", "tool_call_id", "task_id", "turn_id", "api_request_id",
        "conversation_id", "duration_ms", "status", "error_type", "error_message",
        "middleware_trace",
    ):
        if field in kwargs:
            payload[field] = kwargs[field]
    return payload


def on_pre_tool_call(**kwargs: Any) -> Optional[dict[str, Any]]:
    """Dispatch migrated preflight, hookify, and config-protection checks."""
    tool_name = kwargs.get("tool_name")
    if tool_name in _PREFLIGHT_TOOLS:
        result = run_subcommand("preflight", _payload("pre_tool_call", kwargs))
        if result and result.get("action") == "block":
            return {"action": "block", "message": str(result.get("message") or "blocked by foundation preflight")}

    if tool_name in _HOOKIFY_TOOLS:
        result = run_subcommand("hookify", _payload("pre_tool_call", kwargs))
        if result and (result.get("decision") == "block" or result.get("action") == "block"):
            message = result.get("message") or result.get("reason") or "blocked by foundation hookify"
            return {"action": "block", "message": str(message)}

    if tool_name not in _EDIT_TOOLS:
        return None
    try:
        result = run_subcommand(
            "config-protection",
            _payload("pre_tool_call", kwargs),
            strict=True,
            timeout=_CONFIG_PROTECTION_TIMEOUT_SECONDS,
        )
    except HookFailure as error:
        return {"action": "block", "message": f"foundation config-protection failed ({error}); edit blocked"}
    if result and (result.get("decision") == "block" or result.get("action") == "block"):
        message = result.get("message") or result.get("reason") or "blocked by foundation"
        return {"action": "block", "message": str(message)}
    return None


def on_post_tool_call(**kwargs: Any) -> None:
    """Dispatch Hermes post-tool events to independent best-effort observers."""
    payload = _payload("post_tool_call", kwargs)
    run_subcommand("post-observer", payload)
    run_subcommand("lifecycle", payload)


def register(ctx: Any) -> None:
    ctx.register_hook("pre_tool_call", on_pre_tool_call)
    ctx.register_hook("post_tool_call", on_post_tool_call)
