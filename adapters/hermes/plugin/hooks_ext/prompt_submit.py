"""Hermes pre-LLM prompt rules and first-turn summary restoration."""
from __future__ import annotations

from typing import Any


def on_pre_llm_call(core: Any, **kwargs: Any) -> dict[str, Any] | None:
    payload = core.payload("pre_llm_call", kwargs)
    payload["user_message"] = kwargs.get("user_message") or kwargs.get("prompt") or ""
    payload["is_first_turn"] = bool(kwargs.get("is_first_turn", False))
    if "turn_index" in kwargs:
        payload["turn_index"] = kwargs["turn_index"]
    return core.run_subcommand("prompt-submit", payload, strict=False, timeout=5)


def register(ctx: Any, core: Any) -> None:
    ctx.register_hook("pre_llm_call", lambda **kwargs: on_pre_llm_call(core, **kwargs))
