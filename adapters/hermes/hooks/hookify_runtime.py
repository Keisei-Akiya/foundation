"""Bounded Hookify subset for Hermes ``pre_tool_call`` shell hooks.

ECC's ``hookify-runtime.js`` owns rule discovery/normalization, supports YAML
conditions and tool_matcher plus events including ``all``/``prompt``/``stop``,
and matches regexes case-insensitively. This adapter intentionally supports only
flat scalar frontmatter with explicit ``event: bash`` or ``event: file`` and a
single ``pattern``; it does not implement conditions, tool_matcher, default
``event: all``, or warning injection. Only block actions can be enforced; warn
rules are not emulated. ECC requires a name and nonempty message,
whereas this adapter synthesizes a missing name from the filename and permits
an empty message. It checks Bash commands for terminal/Bash, and only a bounded
set of path/content arguments for known file tools. Rule files are flat scalar
YAML-like frontmatter (not full YAML); malformed/unsupported shapes fail open.
Regexes are capped at 256 characters (ECC allows 512), and matching input is
capped at 65,536 characters.

Regexes with backreferences, lookbehind, or quantified groups are rejected;
remaining searches run in a child process with a hard timeout because Python's
stdlib ``re`` engine has no in-process timeout. Oversized input/patterns fail
open. This is deliberately narrower than ECC's matcher, not a full port.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

MAX_RULE_FILES = 64
MAX_RULE_BYTES = 16_384
MAX_INPUT_CHARS = 65_536
MAX_PATTERN_CHARS = 256


def _frontmatter(text: str) -> dict[str, str]:
    """Parse the bounded, flat key/value subset used by ECC Hookify rules."""
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        raise ValueError("missing frontmatter opener")
    try:
        end = next(i for i in range(1, len(lines)) if lines[i].strip() == "---")
    except StopIteration as exc:
        raise ValueError("missing frontmatter closer") from exc
    result: dict[str, str] = {}
    for line in lines[1:end]:
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line[:1].isspace() or ":" not in line:
            raise ValueError("unsupported frontmatter shape")
        key, value = line.split(":", 1)
        key, value = key.strip(), value.strip()
        if not re.fullmatch(r"[a-z_]+", key) or key in result:
            raise ValueError("invalid or duplicate frontmatter key")
        if value.startswith(("[", "{", "|", ">")):
            raise ValueError("unsupported frontmatter value")
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
            value = value[1:-1]
        result[key] = value.split(" #", 1)[0].strip()
    return result


def _safe_pattern(pattern: str) -> bool:
    """Conservative preflight; remaining regex execution is process-time-limited."""
    if not pattern or len(pattern) > MAX_PATTERN_CHARS:
        return False
    if re.search(r"\\[1-9]|\(\?<|\)\s*(?:[+*]|\{)", pattern):
        return False
    try:
        re.compile(pattern)
    except re.error:
        return False
    return True


def load_rules(project_dir: str | Path) -> tuple[list[dict[str, str]], list[str]]:
    """Load project-local .claude/hookify.*.local.md rules without following escapes."""
    root = Path(project_dir).resolve()
    rule_dir = root / ".claude"
    rules: list[dict[str, str]] = []
    diagnostics: list[str] = []
    try:
        if rule_dir.is_symlink() or not rule_dir.is_dir():
            if rule_dir.exists() or rule_dir.is_symlink():
                diagnostics.append("ignored unsafe rule directory: .claude")
            return rules, diagnostics
        candidates = sorted(rule_dir.glob("hookify.*.local.md"))[:MAX_RULE_FILES]
    except OSError:
        return rules, diagnostics
    for path in candidates:
        try:
            if path.is_symlink() or not path.resolve().is_relative_to(rule_dir.resolve()):
                diagnostics.append(f"ignored unsafe rule path: {path.name}")
                continue
            if path.stat().st_size > MAX_RULE_BYTES:
                diagnostics.append(f"ignored oversized rule: {path.name}")
                continue
            rule = _frontmatter(path.read_text(encoding="utf-8"))
            if rule.get("enabled", "true").lower() not in ("true", "false"):
                raise ValueError("enabled must be true or false")
            if rule.get("enabled", "true").lower() == "false":
                continue
            if rule.get("event") not in ("bash", "file"):
                continue
            if not _safe_pattern(rule.get("pattern", "")):
                raise ValueError("unsafe regex pattern")
            if rule.get("action", "warn") not in ("warn", "block"):
                raise ValueError("action must be warn or block")
            rule.setdefault("name", path.name)
            rule.setdefault("action", "warn")
            rule["message"] = "\n".join(path.read_text(encoding="utf-8").split("---", 2)[-1].splitlines()).strip()
            rules.append(rule)
        except (OSError, UnicodeError, ValueError, re.error) as exc:
            diagnostics.append(f"ignored invalid rule {path.name}: {exc}")
    return rules, diagnostics


def _matches(pattern: str, value: str) -> bool:
    """Match inside a killable process; Python's stdlib regex has no timeout."""
    if len(value) > MAX_INPUT_CHARS or re.search(r"\\[1-9]|\(\?<|\)\s*(?:[+*]|\{)", pattern):
        return False
    if len(pattern) > MAX_PATTERN_CHARS:
        return False
    worker = "import json,re,sys; p,v=json.load(sys.stdin); print('1' if re.search(p,v,re.IGNORECASE) else '0')"
    try:
        result = subprocess.run(
            [sys.executable, "-c", worker],
            input=json.dumps([pattern, value]),
            text=True,
            capture_output=True,
            timeout=0.25,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return result.returncode == 0 and result.stdout.strip() == "1"


def evaluate(tool_name: str, args: dict[str, Any], project_dir: str | Path) -> dict[str, str] | None:
    """Return Hermes's documented block directive or ``None``; warnings are context."""
    if not isinstance(args, dict):
        return None
    rules, _ = load_rules(project_dir)
    chunks: list[str] = []
    for rule in rules:
        event = rule["event"]
        if event == "bash":
            values = [args.get("command", "")] if tool_name in ("terminal", "bash", "Bash") else []
        else:
            if tool_name not in ("read_file", "write_file", "patch", "edit_file", "apply_patch", "Edit", "Write", "MultiEdit"):
                continue
            values = [args.get(k, "") for k in ("path", "file_path", "old_string", "new_string", "content")]
        if not any(isinstance(value, str) and _matches(rule["pattern"], value) for value in values):
            continue
        line = f"Hookify rule {rule['name']}: {rule['message'] or 'Policy matched.'}"
        if rule["action"] == "block":
            return {"action": "block", "message": line}
        # Hermes pre_tool_call has no warning-only return contract; returning a
        # made-up context field would corrupt tool arguments, so warn-only rules
        # are intentionally not emulated here.
    return None


def run_payload(payload: Any) -> dict[str, str] | None:
    """Adapt Hermes shell-hook stdin to Hookify and its Claude-compatible reply."""
    if not isinstance(payload, dict):
        return None
    # Hermes supplies tool_name/tool_input/cwd but not hook_event_name; this adapter is registered only for pre_tool_call.
    cwd = payload.get("cwd")
    if not isinstance(cwd, str) or not cwd:
        return None
    result = evaluate(payload.get("tool_name", ""), payload.get("tool_input", {}), cwd)
    if result and result.get("action") == "block":
        return {"decision": "block", "reason": result["message"]}
    return None


def main() -> int:
    import json
    import sys

    try:
        raw = sys.stdin.read(MAX_INPUT_CHARS + 1)
        if len(raw) > MAX_INPUT_CHARS:
            return 0
        result = run_payload(json.loads(raw))
        if result:
            print(json.dumps(result, ensure_ascii=False))
    except (ValueError, OSError):
        # Policy hook is fail-open for malformed/missing project data.
        return 0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
