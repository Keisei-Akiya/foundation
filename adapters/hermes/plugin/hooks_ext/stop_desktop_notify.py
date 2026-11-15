"""Send opt-in long-turn completion notifications without private prompt data."""

import math
import time


def register(ctx, core):
    started = {}

    def notify_start(**kwargs):
        key = (kwargs.get("session_id"), kwargs.get("turn_id") or kwargs.get("task_id"))
        if any(key):
            started[key] = time.monotonic()

    def notify(**kwargs):
        key = (kwargs.get("session_id"), kwargs.get("turn_id") or kwargs.get("task_id"))
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
