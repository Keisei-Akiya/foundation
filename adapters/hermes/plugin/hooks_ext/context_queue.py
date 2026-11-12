"""Queue non-blocking hook messages and inject them on the next model turn."""
from __future__ import annotations

from typing import Any


def register(ctx: Any, core: Any) -> None:
    def on_pre_tool_call(**kwargs: Any) -> None:
        payload = core.payload("pre_tool_call", kwargs)
        core.run_subcommand("doc-warning", payload)
        core.run_subcommand("suggest-compact", payload)

    def on_pre_llm_call(**kwargs: Any) -> dict[str, Any] | None:
        payload = core.payload("pre_llm_call", kwargs)
        result = core.run_subcommand("doc-warning", payload)
        context = result.get("context") if result else None
        return {"context": context} if isinstance(context, str) and context else None

    ctx.register_hook("pre_tool_call", on_pre_tool_call)
    ctx.register_hook("pre_llm_call", on_pre_llm_call)
