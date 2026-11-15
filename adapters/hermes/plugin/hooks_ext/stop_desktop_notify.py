"""Send opt-in long-turn completion notifications without private prompt data."""

import math
import threading
import time
from collections import OrderedDict


_MAX_TRACKED_TURNS = 4096
_START_TTL_SECONDS = 24 * 60 * 60


def _turn_key(kwargs):
    session_id = kwargs.get("session_id")
    turn_id = kwargs.get("turn_id") or kwargs.get("task_id")
    if not isinstance(session_id, str) or not session_id:
        return None
    if not isinstance(turn_id, str) or not turn_id:
        return None
    return session_id, turn_id


def register(ctx, core):
    started = OrderedDict()
    started_lock = threading.Lock()

    def notify_start(**kwargs):
        key = _turn_key(kwargs)
        if key is None:
            return
        now = time.monotonic()
        with started_lock:
            expired = [identity for identity, began in started.items()
                       if now - began > _START_TTL_SECONDS]
            for identity in expired:
                started.pop(identity, None)
            started[key] = now
            started.move_to_end(key)
            while len(started) > _MAX_TRACKED_TURNS:
                started.popitem(last=False)

    def notify(**kwargs):
        key = _turn_key(kwargs)
        if key is None:
            return None
        with started_lock:
            began = started.pop(key, None)
        if began is None:
            return None
        elapsed = max(0, math.ceil(time.monotonic() - began))
        try:
            core.run_subcommand(
                "desktop-notify",
                {"hook_event_name": "post_llm_call", "elapsed_seconds": elapsed},
                timeout=3,
            )
        except Exception:
            return None
        return None

    ctx.register_hook("pre_llm_call", notify_start)
    ctx.register_hook("post_llm_call", notify)
