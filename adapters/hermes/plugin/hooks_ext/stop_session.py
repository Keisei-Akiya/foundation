"""Best-effort post-turn persistence and session review marker hooks."""

from __future__ import annotations

from typing import Any


def on_post_llm_call(core: Any, **kwargs: Any) -> None:
    payload = core.payload("post_llm_call", kwargs)
    payload["conversation_history"] = kwargs.get("conversation_history")
    payload["assistant_response"] = kwargs.get("assistant_response")
    payload["user_message"] = kwargs.get("user_message")
    payload["model"] = kwargs.get("model")
    payload["platform"] = kwargs.get("platform")
    core.run_subcommand("session-end", payload)
    core.run_subcommand("evaluate-session", payload)


def register(ctx: Any, core: Any) -> None:
    def callback(**kwargs: Any) -> None:
        on_post_llm_call(core, **kwargs)

    ctx.register_hook("post_llm_call", callback)
