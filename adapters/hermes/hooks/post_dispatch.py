"""Safe, synchronous subset of ECC PostToolUse child observers for Hermes.

This module intentionally does not recreate ECC's dispatcher. Hermes dispatches
shell hooks individually. The implemented checks are read-only and report via
stderr because post_tool_call shell-hook stdout has no context-injection contract.
Formatter/typecheck execution and telemetry writes are not enabled here.

ECC #15 children reviewed: Hookify rules (not ported: warning/context return
semantics differ), design-quality reminder (ported), edit accumulator (not
ported: session state writes), console warning (ported), governance capture,
activity tracker, metrics bridge, and context monitor (not ported: persistent
or sensitive telemetry/state). ECC #16 children reviewed: post-Bash command
log/cost, PR-created and build-complete observers (not ported: logging/state or
external process dependencies), quality gate (not ported: formatter/typecheck
execution is intentionally omitted; any future implementation must be explicit
opt-in, check tooling availability, and keep fix mode disabled by default), continuous-learning runner (not
ported: invokes a shell script), and skill tracker (not ported: persistent
learning-state writes). ECC sync/async dispatcher behavior is deliberately
left to Hermes; this module is a shell-hook observer, not a dispatcher.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

MAX_STDIN_CHARS = 1_048_576
MAX_FILE_BYTES = 2_000_000
EDIT_TOOLS = {"write_file", "patch", "Edit", "Write", "MultiEdit"}


def _payload_tool(payload: dict[str, Any]) -> tuple[str, dict[str, Any]]:
    name = payload.get("tool_name", "")
    args = payload.get("tool_input", {})
    return (name if isinstance(name, str) else "", args if isinstance(args, dict) else {})


def _path_for(args: dict[str, Any], cwd: str | Path) -> Path | None:
    raw = args.get("file_path", args.get("path"))
    if not isinstance(raw, str) or not raw or "\x00" in raw:
        return None
    candidate = Path(raw)
    if not candidate.is_absolute():
        candidate = Path(cwd) / candidate
    return candidate


def check_console_log(tool_name: str, args: dict[str, Any], cwd: str | Path) -> list[str]:
    """Read-only check for console logging in the file changed by a file tool."""
    if tool_name not in EDIT_TOOLS:
        return []
    target = _path_for(args, cwd)
    if target is None or target.suffix.lower() not in {".js", ".jsx", ".ts", ".tsx"}:
        return []
    try:
        if target.stat().st_size > MAX_FILE_BYTES:
            return []
        source = target.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return []
    if re.search(r"\bconsole\s*\.\s*(?:log|debug|info)\s*\(", source):
        return [f"Post-edit warning: console logging found in {target}"]
    return []


def check_design_quality(tool_name: str, args: dict[str, Any], cwd: str | Path) -> list[str]:
    """Conservative design reminder for edited front-end files; never modifies."""
    if tool_name not in EDIT_TOOLS:
        return []
    target = _path_for(args, cwd)
    if target is None or target.suffix.lower() not in {
        ".astro", ".css", ".html", ".jsx", ".scss", ".svelte", ".tsx", ".vue"
    }:
        return []
    try:
        if target.stat().st_size > MAX_FILE_BYTES:
            return []
        source = target.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return []
    signals = []
    if re.search(r"#(?:000|fff)(?:fff)?\b", source, re.I):
        signals.append("literal black/white color")
    if re.search(r"\b(?:Arial|Helvetica|sans-serif)\b", source, re.I):
        signals.append("generic font family")
    if signals:
        return [f"Design review reminder for {target}: {', '.join(signals)}"]
    return []


def evaluate(payload: Any, cwd: str | Path | None = None) -> list[str]:
    """Return safe read-only observer diagnostics for a Hermes post-tool payload."""
    if not isinstance(payload, dict) or payload.get("hook_event_name") != "post_tool_call":
        return []
    tool_name, args = _payload_tool(payload)
    workdir = cwd or payload.get("cwd")
    if not isinstance(workdir, (str, Path)) or not workdir:
        return []
    return check_design_quality(tool_name, args, workdir) + check_console_log(tool_name, args, workdir)


def main() -> int:
    """Read Hermes JSON from stdin. Malformed/oversized input fails open."""
    try:
        raw = sys.stdin.read(MAX_STDIN_CHARS + 1)
        if len(raw) > MAX_STDIN_CHARS:
            return 0
        messages = evaluate(json.loads(raw))
    except (json.JSONDecodeError, OSError, ValueError):
        return 0
    for message in messages:
        print(message, file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
