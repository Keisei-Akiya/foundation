"""Stateless blocking preflight rules for Hermes shell hooks.

Hermes ``agent/shell_hooks.py`` consumes block/modify directives on
``pre_tool_call`` but ignores ``context`` there. This runner therefore emits
only verified blocking directives. Advisory warnings, session counters,
audit sinks, and stateful sensitive-file gates are intentionally omitted until
Hermes exposes a user-visible channel and a safe durable state contract.
"""
from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass
from typing import Any


@dataclass
class Result:
    action: str | None = None
    message: str = ""

    def directive(self) -> dict[str, str] | None:
        if self.action == "block" and self.message:
            return {"action": "block", "message": self.message}
        return None


def _command(tool_name: str, args: dict[str, Any]) -> str:
    if tool_name in {"terminal", "bash", "Bash", "powershell", "PowerShell", "powershell_tool", "pwsh"}:
        value = args.get("command", "")
        return value if isinstance(value, str) else ""
    return ""


def evaluate(event: dict[str, Any], *, state: dict[str, Any] | None = None,
             env: dict[str, str] | None = None, profile: str = "standard") -> Result:
    """Evaluate only stateless hard blocks; state/env/profile are ignored.

    Kept as optional arguments for callers using the earlier API. No policy
    relies on them: state is not persistent under Hermes' one-shot runner, and
    advisory/environment features have no verified delivery/sink contract.
    """
    result = Result()
    name = event.get("tool_name", "")
    args = event.get("tool_input")
    if not isinstance(args, dict):
        args = event.get("args")
    if not isinstance(name, str) or not isinstance(args, dict):
        return result
    cmd = _command(name, args)

    # Narrow lexical subset only. The ECC scanner also handles git -c hook redirects,
    # commit -n/prefixes and shell syntax; those are not claimed as ported here.
    if cmd and re.search(r"\bgit\s+commit\b[^\n]*--no-verify\b", cmd, re.I):
        result.action = "block"
        result.message = "Preflight block: detected literal git commit --no-verify; shell syntax and git hook-redirect variants are not covered."
        return result

    # Limited destructive PowerShell guard only; this is not full GateGuard equivalence.
    if name.casefold() in {"powershell", "powershell_tool", "pwsh"} and cmd:
        if re.search(r"\b(Remove-Item|Clear-Content|Stop-Process|Format-Volume|Remove-Partition)\b|\b(?:Remove-Item|del|erase)\b[^\n]*-Recurse[^\n]*-Force", cmd, re.I):
            result.action = "block"
            result.message = "Potentially destructive PowerShell operation; review target and rollback before retrying."
    return result


def main() -> int:
    raw = sys.stdin.read(1024 * 1024 + 1)
    if len(raw.encode("utf-8")) > 1024 * 1024:
        return 0  # Never inspect partial JSON; no action.
    try:
        event = json.loads(raw)
    except (json.JSONDecodeError, TypeError):
        return 0
    if not isinstance(event, dict):
        return 0
    directive = evaluate(event).directive()
    if directive:
        sys.stdout.write(json.dumps(directive, ensure_ascii=False) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
