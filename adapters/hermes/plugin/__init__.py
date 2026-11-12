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


def run_subcommand(subcommand: str, payload: dict[str, Any]) -> Optional[dict[str, Any]]:
    """Run `foundation <subcommand>` with payload on stdin; return its stdout JSON.

    Returns None when the binary is missing, fails, times out, or prints
    nothing. Every failure mode is fail-open so a broken install never blocks
    the agent.
    """
    binary = resolve_binary()
    if binary is None:
        logger.warning("foundation: binary not found; skipping %s", subcommand)
        return None
    try:
        proc = subprocess.run(
            [binary, subcommand],
            input=json.dumps(payload, ensure_ascii=False, default=str),
            capture_output=True,
            text=True,
            timeout=_TIMEOUT_SECONDS,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as error:
        logger.warning("foundation %s failed: %s", subcommand, error)
        return None
    out = proc.stdout.strip()
    if not out:
        return None
    try:
        data = json.loads(out)
    except json.JSONDecodeError:
        logger.warning("foundation %s printed non-JSON stdout", subcommand)
        return None
    return data if isinstance(data, dict) else None


def _payload(event: str, kwargs: dict[str, Any]) -> dict[str, Any]:
    args = kwargs.get("args")
    return {
        "hook_event_name": event,
        "tool_name": kwargs.get("tool_name"),
        "tool_input": args if isinstance(args, dict) else None,
        "session_id": kwargs.get("session_id") or "",
        "cwd": os.getcwd(),
    }


def on_pre_tool_call(**kwargs: Any) -> Optional[dict[str, Any]]:
    """config-protection: veto edits to linter/formatter config files."""
    result = run_subcommand("config-protection", _payload("pre_tool_call", kwargs))
    if result and result.get("decision") == "block":
        return {"action": "block", "message": str(result.get("reason") or "blocked by foundation")}
    return None


def register(ctx: Any) -> None:
    ctx.register_hook("pre_tool_call", on_pre_tool_call)
