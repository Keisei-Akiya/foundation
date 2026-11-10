#!/usr/bin/env python3
"""Compatibility bridge for upstream hooks that predate Hermes ``edit_file``."""
from __future__ import annotations

import importlib.util
import io
import json
import sys
from pathlib import Path

HOOKS = Path(__file__).resolve().parent
_ALLOWED = {"check-console-log.py"}


def main() -> int:
    if len(sys.argv) != 2 or sys.argv[1] not in _ALLOWED:
        sys.stderr.write("usage: hermes-tool-compat.py check-console-log.py\n")
        return 2
    try:
        payload = json.load(sys.stdin)
        if isinstance(payload, dict) and payload.get("tool_name") == "edit_file":
            payload["tool_name"] = "edit"
    except (ValueError, OSError) as exc:
        sys.stderr.write(f"[ecc-hook-compat] could not adapt hook payload ({exc}); original hook will fail open\n")
        payload = {}
    target = HOOKS / sys.argv[1]
    spec = importlib.util.spec_from_file_location("ecc_upstream_hook", target)
    if spec is None or spec.loader is None:
        sys.stderr.write(f"[ecc-hook-compat] could not load {target}\n")
        return 0
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    original_stdin, sys.stdin = sys.stdin, io.StringIO(json.dumps(payload))
    try:
        return module.main()
    finally:
        sys.stdin = original_stdin


if __name__ == "__main__":
    raise SystemExit(main())
