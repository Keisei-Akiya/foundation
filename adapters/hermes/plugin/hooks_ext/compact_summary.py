"""Opt-in rolling work summary for post-compaction restoration."""
from __future__ import annotations

from typing import Any


def register(ctx: Any, core: Any) -> None:
    def dispatch(event: str, kwargs: dict[str, Any]) -> dict[str, Any] | None:
        payload = core.payload(event, kwargs)
        if event == "post_llm_call":
            payload["conversation_history"] = kwargs.get("conversation_history")
        try:
            result = core.run_subcommand("compact-summary", payload, strict=False, timeout=5)
        except Exception as error:  # noqa: BLE001 - compaction hooks must fail open
            core.logger.warning("foundation compact-summary failed: %s", error)
            return None
        if event == "pre_llm_call" and result:
            context = result.get("context")
            return {"context": context} if isinstance(context, str) and context else None
        return None

    ctx.register_hook("post_llm_call", lambda **kwargs: dispatch("post_llm_call", kwargs))
    ctx.register_hook("post_compaction", lambda **kwargs: dispatch("post_compaction", kwargs))
    ctx.register_hook("pre_llm_call", lambda **kwargs: dispatch("pre_llm_call", kwargs))
