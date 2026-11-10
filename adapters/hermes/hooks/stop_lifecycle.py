"""Safe building blocks for mapping ECC stop hooks to Hermes lifecycle hooks.

This module deliberately performs no I/O, subprocess execution, notification,
network access, transcript inspection, or persistence. Callers must add explicit
opt-in and an independently reviewed storage/command adapter before enabling
any side effect.
"""

from __future__ import annotations

import os
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Mapping


# ECC Stop fires per response; it is closest to Hermes' run/turn boundary.
# Hermes session-finalize is the lifetime boundary and is not a substitute.
ECC_STOP_EVENT_MAP = {
    20: None,  # Plan Canvas queue/API + race-free acknowledgement are absent.
    21: "pre_verify",  # Only candidate detection is portable here.
    23: "on_session_end",
    24: "on_session_end",  # Evaluation needs explicit policy and user action.
    25: "post_api_request",  # Usage is observed per provider attempt, not at Stop.
    26: None,  # Desktop notification target and opt-in are environment-specific.
    27: "on_session_finalize",
}

_USAGE_FIELDS = (
    "input_tokens",
    "output_tokens",
    "cache_creation_input_tokens",
    "cache_read_input_tokens",
)
_JS_EXTENSIONS = {".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".mts", ".cts"}


def event_for_ecc_stop(issue: int) -> str | None:
    """Return the closest Hermes lifecycle event, or None if not portable."""
    return ECC_STOP_EVENT_MAP.get(issue)


def should_persist(event: str, policy: Mapping[str, Any] | None) -> bool:
    """Require an affirmative opt-in and a scope matching the event boundary."""
    if not isinstance(policy, Mapping) or policy.get("enabled") is not True:
        return False
    expected_scope = {"on_session_end": "turn", "on_session_finalize": "session"}.get(event)
    return expected_scope is not None and policy.get("scope") == expected_scope


def _safe_relative_path(value: Any, root: Path) -> str | None:
    if not isinstance(value, str) or not value or "\x00" in value:
        return None
    # Reject absolute and traversal paths before normalization. Hermes changed
    # paths are repo-relative; accepting otherwise could inspect outside files.
    posix = PurePosixPath(value.replace("\\", "/"))
    if posix.is_absolute() or any(part in ("", ".", "..") for part in posix.parts):
        return None
    candidate = root.joinpath(*posix.parts)
    try:
        candidate.resolve(strict=False).relative_to(root.resolve(strict=False))
    except (OSError, ValueError):
        return None
    return posix.as_posix()


def changed_javascript_paths(
    changed_paths: Iterable[str],
    repo_root: str | os.PathLike[str],
    deleted: Iterable[str] = (),
) -> tuple[str, ...]:
    """Select changed JS/TS paths only; never launches formatters/typecheckers."""
    root = Path(repo_root)
    deleted_set = {str(item) for item in deleted}
    selected: set[str] = set()
    for value in changed_paths:
        rel = _safe_relative_path(value, root)
        if rel is None or rel in deleted_set:
            continue
        if Path(rel).suffix.lower() in _JS_EXTENSIONS:
            selected.add(rel)
    return tuple(sorted(selected))


def session_usage_totals(usages: Iterable[Mapping[str, Any]]) -> dict[str, int]:
    """Aggregate Hermes usage fields without pricing assumptions or storage."""
    totals = {field: 0 for field in _USAGE_FIELDS}
    for usage in usages:
        if not isinstance(usage, Mapping):
            continue
        for field in _USAGE_FIELDS:
            value = usage.get(field)
            # Avoid bool (an int subclass), floats, numeric strings and negatives:
            # absent/ambiguous provider data is not silently converted.
            if isinstance(value, int) and not isinstance(value, bool) and value >= 0:
                totals[field] += value
    return totals
